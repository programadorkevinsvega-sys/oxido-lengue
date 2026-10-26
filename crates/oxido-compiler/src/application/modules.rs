use super::source_map::SourceMap;
use crate::{GeneratedProgram, generate_ox};
use oxido_language::{SourceFile, domain::syntax::LexOptions};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    ops::Range,
    path::{Path, PathBuf},
};
use syn::{Item, ext::IdentExt};

const MAX_MODULE_FILES: usize = 256;
const MAX_MODULE_DEPTH: usize = 64;
const MAX_SOURCE_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PROJECT_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct GeneratedModule {
    pub path: PathBuf,
    pub rust: String,
    pub source_map: Option<SourceMap>,
}

#[derive(Clone, Debug)]
pub struct ResolvedProjectModules {
    pub root_rust: String,
    pub modules: Vec<GeneratedModule>,
}

#[derive(Clone)]
struct Replacement {
    range: Range<usize>,
    value: String,
}

pub fn resolve_project_modules(
    root_source: &SourceFile,
    root_rust: &str,
    options: LexOptions,
    project_root: &Path,
) -> Result<ResolvedProjectModules, String> {
    let project_root = fs::canonicalize(project_root)
        .map_err(|error| format!("no se pudo resolver la raíz del proyecto: {error}"))?;
    let source_path = root_source
        .path()
        .ok_or_else(|| "la resolución de módulos necesita una ruta de archivo .ox".to_owned())?;
    let canonical_source = fs::canonicalize(source_path)
        .map_err(|error| format!("no se pudo resolver el archivo raíz: {error}"))?;
    if !canonical_source.starts_with(&project_root) {
        return Err("el archivo raíz sale del directorio del proyecto".into());
    }
    let source_root = canonical_source
        .parent()
        .ok_or_else(|| "el archivo raíz no tiene directorio padre".to_owned())?;
    let parsed = syn::parse_file(root_rust)
        .map_err(|error| format!("Rust raíz generado no válido: {error}"))?;
    let mut graph = ModuleGraph {
        project_root: &project_root,
        source_root,
        options,
        files: BTreeMap::new(),
        active_sources: BTreeSet::from([canonical_source.clone()]),
        total_bytes: root_source.text().len() as u64,
    };
    let mut root_replacements = Vec::new();
    graph.visit_items(
        &parsed.items,
        &ModulePaths::root(source_root),
        0,
        &mut root_replacements,
    )?;
    Ok(ResolvedProjectModules {
        root_rust: apply_replacements(root_rust, root_replacements)?,
        modules: graph.files.into_values().collect(),
    })
}

struct ModuleGraph<'a> {
    project_root: &'a Path,
    source_root: &'a Path,
    options: LexOptions,
    files: BTreeMap<PathBuf, GeneratedModule>,
    active_sources: BTreeSet<PathBuf>,
    total_bytes: u64,
}

struct ModulePaths {
    module_directory: PathBuf,
    attribute_directory: PathBuf,
    generated_attribute_directory: PathBuf,
}

impl ModulePaths {
    fn root(source_root: &Path) -> Self {
        Self {
            module_directory: source_root.to_owned(),
            attribute_directory: source_root.to_owned(),
            generated_attribute_directory: PathBuf::new(),
        }
    }

    fn child(&self, name: &str) -> Self {
        Self {
            module_directory: self.module_directory.join(name),
            attribute_directory: self.attribute_directory.join(name),
            generated_attribute_directory: self.generated_attribute_directory.join(name),
        }
    }
}

impl ModuleGraph<'_> {
    fn visit_items(
        &mut self,
        items: &[Item],
        paths: &ModulePaths,
        depth: usize,
        replacements: &mut Vec<Replacement>,
    ) -> Result<(), String> {
        if depth > MAX_MODULE_DEPTH {
            return Err(format!(
                "la jerarquía de módulos supera {MAX_MODULE_DEPTH} niveles"
            ));
        }
        for item in items {
            let Item::Mod(module) = item else {
                continue;
            };
            let name = module.ident.unraw().to_string();
            if let Some((_, child_items)) = &module.content {
                self.visit_items(child_items, &paths.child(&name), depth + 1, replacements)?;
                continue;
            }
            let explicit_path = path_attribute(module)?;
            let candidate = match &explicit_path {
                Some((path, _)) => resolve_explicit_candidate(
                    &paths.attribute_directory,
                    path,
                    self.project_root,
                    &name,
                )?,
                None => resolve_candidate(&paths.module_directory, &name, self.project_root)?.ok_or_else(
                    || {
                        format!(
                            "no se encontró el archivo del módulo `{name}` desde `{}`; se admite `<módulo>.ox` o `<módulo>/mod.ox` (y sus equivalentes `.rs`)",
                            relative_path(self.project_root, &paths.module_directory)
                        )
                    },
                )?,
            };
            let output_path = rust_relative_path(self.source_root, &candidate)?;
            if let Some((_, literal_span)) = &explicit_path {
                let generated_target =
                    relative_path_between(&paths.generated_attribute_directory, &output_path)?;
                replacements.push(Replacement {
                    range: literal_span.clone(),
                    value: rust_string_literal(&generated_target),
                });
            }
            self.load_external_module(candidate, output_path, explicit_path.is_some(), depth + 1)?;
        }
        Ok(())
    }

    fn load_external_module(
        &mut self,
        candidate: PathBuf,
        output_path: PathBuf,
        was_path_attribute: bool,
        depth: usize,
    ) -> Result<(), String> {
        if depth > MAX_MODULE_DEPTH {
            return Err(format!(
                "la jerarquía de módulos supera {MAX_MODULE_DEPTH} niveles"
            ));
        }
        let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
            format!(
                "no se pudo inspeccionar el módulo {}: {error}",
                candidate.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "el módulo {} debe ser un archivo regular, no un enlace simbólico",
                candidate.display()
            ));
        }
        if metadata.len() > MAX_SOURCE_FILE_BYTES {
            return Err(format!(
                "el archivo del módulo {} supera el límite de 4 MiB",
                candidate.display()
            ));
        }
        let canonical = fs::canonicalize(&candidate).map_err(|error| {
            format!(
                "no se pudo resolver el módulo {}: {error}",
                candidate.display()
            )
        })?;
        if !canonical.starts_with(self.project_root) {
            return Err(format!(
                "el módulo {} sale de la raíz del proyecto",
                candidate.display()
            ));
        }
        if self.active_sources.contains(&canonical) {
            return Err(format!(
                "ciclo detectado al resolver el módulo {}",
                candidate.display()
            ));
        }
        self.total_bytes = self.total_bytes.saturating_add(metadata.len());
        if self.total_bytes > MAX_PROJECT_SOURCE_BYTES {
            return Err("los archivos del proyecto superan el límite de 64 MiB".into());
        }
        if self.files.len() >= MAX_MODULE_FILES {
            return Err(format!(
                "el proyecto supera el límite de {MAX_MODULE_FILES} archivos de módulo"
            ));
        }
        if self.files.contains_key(&output_path) {
            return Err(format!(
                "más de una declaración resuelve al archivo Rust `{}`",
                output_path.display()
            ));
        }
        let source_text = fs::read_to_string(&canonical).map_err(|error| {
            format!("no se pudo leer el módulo {}: {error}", canonical.display())
        })?;
        let is_ox = canonical
            .extension()
            .is_some_and(|extension| extension == "ox");
        let (rust, source_map) = if is_ox {
            let source = SourceFile::with_path(&canonical, source_text);
            let generated = generate_ox(&source, self.options).map_err(|diagnostics| {
                format!(
                    "no se pudo analizar el módulo `{}`: {}",
                    relative_path(self.project_root, &canonical),
                    diagnostics
                        .iter()
                        .map(|diagnostic| format!("[{}] {}", diagnostic.code, diagnostic.message))
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            })?;
            let source_map =
                module_source_map(generated, self.project_root, &canonical, &output_path);
            (source_map.0, Some(source_map.1))
        } else {
            (source_text, None)
        };
        let parsed = syn::parse_file(&rust).map_err(|error| {
            format!(
                "no se pudo analizar Rust del módulo `{}`: {error}",
                relative_path(self.project_root, &canonical)
            )
        })?;
        let child_directory = if was_path_attribute {
            canonical
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        } else {
            module_child_directory(&canonical)
        };
        let generated_attribute_directory = output_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        self.active_sources.insert(canonical.clone());
        let mut replacements = Vec::new();
        let paths = ModulePaths {
            module_directory: child_directory,
            attribute_directory: canonical
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
            generated_attribute_directory,
        };
        let visited = self.visit_items(&parsed.items, &paths, depth, &mut replacements);
        self.active_sources.remove(&canonical);
        visited?;
        let rust = apply_replacements(&rust, replacements)?;
        self.files.insert(
            output_path.clone(),
            GeneratedModule {
                path: output_path,
                rust,
                source_map,
            },
        );
        Ok(())
    }
}

fn path_attribute(module: &syn::ItemMod) -> Result<Option<(String, Range<usize>)>, String> {
    let mut path = None;
    for attribute in &module.attrs {
        if !attribute.path().is_ident("path") {
            continue;
        }
        if path.is_some() {
            return Err(format!(
                "el módulo `{}` declara #[path] más de una vez",
                module.ident.unraw()
            ));
        }
        let syn::Meta::NameValue(name_value) = &attribute.meta else {
            return Err(format!(
                "#[path] del módulo `{}` debe tener la forma #[path = \"archivo.ox\"]",
                module.ident.unraw()
            ));
        };
        let syn::Expr::Lit(expression) = &name_value.value else {
            return Err(format!(
                "#[path] del módulo `{}` debe ser una cadena literal",
                module.ident.unraw()
            ));
        };
        let syn::Lit::Str(literal) = &expression.lit else {
            return Err(format!(
                "#[path] del módulo `{}` debe ser una cadena literal",
                module.ident.unraw()
            ));
        };
        path = Some((literal.value(), literal.span().byte_range()));
    }
    Ok(path)
}

fn resolve_explicit_candidate(
    base: &Path,
    relative: &str,
    project_root: &Path,
    module_name: &str,
) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            !matches!(
                component,
                std::path::Component::Normal(_)
                    | std::path::Component::CurDir
                    | std::path::Component::ParentDir
            )
        })
        || !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ox" | "rs")
        )
    {
        return Err(format!(
            "#[path] del módulo `{module_name}` debe ser una ruta relativa `.ox` o `.rs`"
        ));
    }
    let candidate = normalize_path(&base.join(path))?;
    let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
        format!("no se pudo resolver #[path] `{relative}` del módulo `{module_name}`: {error}")
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "#[path] `{relative}` del módulo `{module_name}` debe apuntar a un archivo regular"
        ));
    }
    let canonical = fs::canonicalize(&candidate)
        .map_err(|error| format!("no se pudo canonicalizar #[path] `{relative}`: {error}"))?;
    if !canonical.starts_with(project_root) {
        return Err(format!(
            "#[path] `{relative}` del módulo `{module_name}` sale de la raíz del proyecto"
        ));
    }
    Ok(canonical)
}

fn normalize_path(path: &Path) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err("#[path] intenta salir de la raíz del sistema de archivos".into());
                }
            }
        }
    }
    Ok(normalized)
}

fn apply_replacements(source: &str, mut replacements: Vec<Replacement>) -> Result<String, String> {
    replacements.sort_by_key(|replacement| replacement.range.start);
    if replacements
        .windows(2)
        .any(|pair| pair[0].range.end > pair[1].range.start)
    {
        return Err("los rangos de #[path] se superponen en el Rust generado".into());
    }
    let mut output = source.to_owned();
    for replacement in replacements.into_iter().rev() {
        if replacement.range.end > output.len()
            || !output.is_char_boundary(replacement.range.start)
            || !output.is_char_boundary(replacement.range.end)
        {
            return Err("el rango de #[path] no coincide con el Rust generado".into());
        }
        output.replace_range(replacement.range, &replacement.value);
    }
    Ok(output)
}

fn rust_string_literal(value: &str) -> String {
    let mut literal = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '"' => literal.push_str("\\\""),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            character if character.is_control() => {
                literal.push_str(&format!("\\u{{{:x}}}", u32::from(character)));
            }
            character => literal.push(character),
        }
    }
    literal.push('"');
    literal
}

fn relative_path_between(base: &Path, target: &Path) -> Result<String, String> {
    let base: Vec<_> = base.components().collect();
    let target: Vec<_> = target.components().collect();
    let shared = base
        .iter()
        .zip(&target)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in shared..base.len() {
        relative.push("..");
    }
    for component in &target[shared..] {
        let std::path::Component::Normal(value) = component else {
            return Err("la ruta generada de #[path] contiene componentes no válidos".into());
        };
        relative.push(value);
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn resolve_candidate(
    base: &Path,
    name: &str,
    project_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let candidates = [
        base.join(format!("{name}.ox")),
        base.join(name).join("mod.ox"),
        base.join(format!("{name}.rs")),
        base.join(name).join("mod.rs"),
    ];
    let mut existing = Vec::new();
    for candidate in candidates {
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "el módulo `{name}` no puede resolverse mediante enlaces simbólicos"
                    ));
                }
                if !metadata.is_file() {
                    return Err(format!(
                        "la ruta del módulo `{name}` no es un archivo regular"
                    ));
                }
                let canonical = fs::canonicalize(&candidate)
                    .map_err(|error| format!("no se pudo resolver el módulo `{name}`: {error}"))?;
                if !canonical.starts_with(project_root) {
                    return Err(format!(
                        "la ruta del módulo `{name}` sale de la raíz del proyecto"
                    ));
                }
                existing.push(canonical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "no se pudo inspeccionar el módulo `{name}`: {error}"
                ));
            }
        }
    }
    match existing.len() {
        0 => Ok(None),
        1 => Ok(existing.pop()),
        _ => Err(format!(
            "el módulo `{name}` es ambiguo; existe más de una ruta convencional"
        )),
    }
}

fn rust_relative_path(source_root: &Path, source: &Path) -> Result<PathBuf, String> {
    let relative = source
        .strip_prefix(source_root)
        .map_err(|_| "la ruta del módulo no es relativa al directorio de la raíz".to_owned())?;
    let mut output = relative.to_path_buf();
    output.set_extension("rs");
    if output
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("la ruta generada del módulo debe permanecer dentro de src".into());
    }
    Ok(output)
}

fn module_child_directory(source: &Path) -> PathBuf {
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    if source.file_stem().is_some_and(|stem| stem == "mod") {
        parent.to_path_buf()
    } else {
        parent.join(source.file_stem().unwrap_or_default())
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn module_source_map(
    generated: GeneratedProgram,
    project_root: &Path,
    source: &Path,
    output: &Path,
) -> (String, SourceMap) {
    let mut source_map = generated.source_map;
    source_map.set_source_relative_path(relative_path(project_root, source));
    source_map.set_generated_file(Some(output));
    (generated.rust, source_map)
}
