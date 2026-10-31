//! Índice incremental del workspace `.ox` con caché en disco.
//!
//! El índice contiene solo declaraciones y rangos léxicos; no infiere tipos ni
//! ejecuta código. La caché vive fuera del workspace, se versiona por esquema y
//! se invalida por fecha de modificación, tamaño y hash del contenido. Las
//! escrituras usan un archivo temporal en el mismo directorio y un `rename`
//! atómico. Los errores se registran sin incluir rutas absolutas.

use crate::{WorkspaceSymbol, file_uri, file_uri_path, workspace_symbols_from_documents};
use oxido_language::domain::syntax::SyntaxMode;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant, UNIX_EPOCH},
};

pub(crate) const INDEX_CACHE_VERSION: u32 = 1;
const MAX_INDEXED_FILES: usize = 256;
const MAX_INDEX_SCAN_ENTRIES: usize = 2048;
const MAX_INDEXED_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_INDEX_CACHE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const STALE_AFTER: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct FileStamp {
    mtime_seconds: i64,
    mtime_nanos: u32,
    size: u64,
    hash: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CacheEntry {
    uri: String,
    stamp: FileStamp,
    symbols: Vec<WorkspaceSymbol>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CacheFile {
    version: u32,
    mode: Option<String>,
    files: Vec<CacheEntry>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct IndexStats {
    pub(crate) files: usize,
    pub(crate) reused: usize,
    pub(crate) reindexed: usize,
    pub(crate) removed: usize,
    pub(crate) cache_written: bool,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug)]
struct IndexEntry {
    stamp: FileStamp,
    symbols: Vec<WorkspaceSymbol>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WorkspaceIndex {
    root: Option<PathBuf>,
    mode: Option<SyntaxMode>,
    entries: BTreeMap<String, IndexEntry>,
    cache_path: Option<PathBuf>,
    last_refresh: Option<Instant>,
    stats: IndexStats,
}

impl WorkspaceIndex {
    #[must_use]
    pub(crate) fn load(
        root: Option<PathBuf>,
        mode: Option<SyntaxMode>,
        cache_dir: Option<PathBuf>,
    ) -> Self {
        let Some(root) = root else {
            return Self::default();
        };
        let cache_path = cache_dir
            .filter(|directory| !directory.starts_with(&root))
            .map(|directory| cache_path(&directory, &root));
        let cached = cache_path
            .as_deref()
            .and_then(|path| read_cache(path, mode))
            .unwrap_or_default();
        let mut index = Self {
            root: Some(root.clone()),
            mode,
            cache_path,
            last_refresh: Some(Instant::now()),
            ..Self::default()
        };
        index.rebuild(cached);
        index
    }

    #[must_use]
    pub(crate) fn stats(&self) -> IndexStats {
        self.stats
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Símbolos del workspace cerrado con los documentos abiertos superpuestos.
    #[must_use]
    pub(crate) fn symbols(
        &self,
        open: &[(String, String)],
        root: Option<&Path>,
        mode: Option<SyntaxMode>,
    ) -> Vec<WorkspaceSymbol> {
        let mut by_uri = self
            .entries
            .iter()
            .map(|(uri, entry)| (uri.clone(), entry.symbols.clone()))
            .collect::<BTreeMap<_, _>>();
        for (uri, text) in open {
            by_uri.insert(
                uri.clone(),
                workspace_symbols_from_documents(
                    &[(uri.clone(), text.clone(), None, uri.clone())],
                    root,
                    mode,
                ),
            );
        }
        by_uri.into_values().flatten().collect()
    }

    /// Relee el workspace si pasó el intervalo mínimo. Es incremental: reutiliza
    /// las entradas cuya fecha y tamaño no cambiaron y solo recalcula las demás.
    pub(crate) fn refresh_if_stale(
        &mut self,
        root: Option<&Path>,
        mode: Option<SyntaxMode>,
        min_interval: Duration,
    ) {
        if self
            .last_refresh
            .is_some_and(|last| last.elapsed() < min_interval)
        {
            return;
        }
        let Some(root) = root else {
            self.last_refresh = Some(Instant::now());
            return;
        };
        if self.root.as_deref() != Some(root) {
            return;
        }
        self.mode = mode;
        self.last_refresh = Some(Instant::now());
        let cached = self.entries.clone().into_iter().collect::<Vec<_>>();
        self.rebuild(cached);
    }

    /// Actualiza una sola entrada tras cerrar un documento.
    pub(crate) fn refresh_uri(&mut self, uri: &str, mode: Option<SyntaxMode>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let Some(path) = file_uri_path(uri).map(PathBuf::from) else {
            return;
        };
        let canonical = path
            .canonicalize()
            .ok()
            .filter(|canonical| canonical.starts_with(&root));
        let Some(canonical) = canonical else {
            self.entries.remove(uri);
            self.stats.files = self.entries.len();
            self.persist();
            return;
        };
        self.mode = mode;
        match read_entry(&canonical, &root, mode) {
            Some(entry) => {
                self.stats.reindexed = self.stats.reindexed.saturating_add(1);
                self.entries.insert(uri.to_owned(), entry);
            }
            None => {
                self.entries.remove(uri);
            }
        }
        self.stats.files = self.entries.len();
        self.persist();
    }

    fn rebuild(&mut self, cached: Vec<(String, IndexEntry)>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let mut previous = cached.into_iter().collect::<BTreeMap<_, _>>();
        let mut entries = BTreeMap::new();
        let mut stats = IndexStats {
            generation: self.stats.generation.saturating_add(1),
            ..IndexStats::default()
        };
        let mut changed = false;
        for path in scan_workspace(&root) {
            let Ok(canonical) = path.canonicalize() else {
                continue;
            };
            if !canonical.starts_with(&root) {
                continue;
            }
            let uri = file_uri(&canonical);
            let Some(stamp) = file_stamp(&canonical) else {
                continue;
            };
            if let Some(entry) = previous.remove(&uri)
                && entry.stamp.size == stamp.size
                && entry.stamp.mtime_seconds == stamp.mtime_seconds
                && entry.stamp.mtime_nanos == stamp.mtime_nanos
            {
                stats.reused = stats.reused.saturating_add(1);
                entries.insert(uri, entry);
                continue;
            }
            let Some(entry) = read_entry_with_stamp(&canonical, &root, self.mode, stamp) else {
                continue;
            };
            if let Some(previous) = previous.get(&uri)
                && previous.stamp.hash == entry.stamp.hash
            {
                stats.reused = stats.reused.saturating_add(1);
                entries.insert(uri, entry);
                changed = true;
                continue;
            }
            stats.reindexed = stats.reindexed.saturating_add(1);
            entries.insert(uri, entry);
            changed = true;
        }
        stats.removed = previous.len();
        if stats.removed > 0 {
            changed = true;
        }
        stats.files = entries.len();
        self.entries = entries;
        self.stats = stats;
        if changed {
            self.persist();
        }
    }

    fn persist(&mut self) {
        let Some(path) = self.cache_path.clone() else {
            return;
        };
        let cache = CacheFile {
            version: INDEX_CACHE_VERSION,
            mode: mode_label(self.mode).map(str::to_owned),
            files: self
                .entries
                .iter()
                .map(|(uri, entry)| CacheEntry {
                    uri: uri.clone(),
                    stamp: entry.stamp,
                    symbols: entry.symbols.clone(),
                })
                .collect(),
        };
        let Ok(encoded) = serde_json::to_vec(&cache) else {
            return;
        };
        if encoded.len() > MAX_INDEX_CACHE_BYTES {
            return;
        }
        if let Some(parent) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                eprintln!("no se pudo preparar la caché del índice: {error}");
                return;
            }
            if let Err(error) = restrict_permissions(parent, 0o700) {
                eprintln!(
                    "no se pudieron restringir los permisos del directorio de caché: {error}"
                );
            }
        }
        let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
        if let Err(error) = std::fs::write(&temporary, encoded) {
            eprintln!("no se pudo escribir la caché del índice: {error}");
            return;
        }
        if let Err(error) = restrict_permissions(&temporary, 0o600) {
            eprintln!("no se pudieron restringir los permisos de la caché del índice: {error}");
            if let Err(cleanup) = std::fs::remove_file(&temporary) {
                eprintln!("no se pudo limpiar la caché temporal del índice: {cleanup}");
            }
            return;
        }
        if let Err(error) = std::fs::rename(&temporary, &path) {
            if std::fs::remove_file(&path).is_ok() && std::fs::rename(&temporary, &path).is_ok() {
                self.stats.cache_written = true;
                return;
            }
            if let Err(cleanup) = std::fs::remove_file(&temporary) {
                eprintln!("no se pudo limpiar la caché temporal del índice: {cleanup}");
            }
            eprintln!("no se pudo publicar la caché del índice: {error}");
            return;
        }
        self.stats.cache_written = true;
    }
}

/// Restringe permisos de la caché al usuario actual en plataformas Unix.
///
/// En otros sistemas es un no-op: los permisos dependen de las ACL del sistema.
#[cfg(unix)]
fn restrict_permissions(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path, _mode: u32) -> std::io::Result<()> {
    Ok(())
}

/// Directorio base de caché del usuario. Nunca se usa la raíz del workspace.
#[must_use]
pub(crate) fn default_cache_dir() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("OXIDO_CACHE_DIR")
        && !directory.is_empty()
    {
        return Some(PathBuf::from(directory).join("oxido"));
    }
    if let Some(directory) = std::env::var_os("XDG_CACHE_HOME")
        && !directory.is_empty()
    {
        return Some(PathBuf::from(directory).join("oxido"));
    }
    if cfg!(windows)
        && let Some(directory) = std::env::var_os("LOCALAPPDATA")
        && !directory.is_empty()
    {
        return Some(PathBuf::from(directory).join("oxido").join("cache"));
    }
    std::env::var_os("HOME")
        .filter(|directory| !directory.is_empty())
        .map(|directory| PathBuf::from(directory).join(".cache").join("oxido"))
}

fn cache_path(directory: &Path, root: &Path) -> PathBuf {
    directory.join(format!(
        "workspace-index-v{INDEX_CACHE_VERSION}-{:016x}.json",
        fnv1a(root.to_string_lossy().as_bytes())
    ))
}

fn read_cache(path: &Path, mode: Option<SyntaxMode>) -> Option<Vec<(String, IndexEntry)>> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_INDEX_CACHE_BYTES as u64 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let cache = serde_json::from_slice::<CacheFile>(&bytes).ok()?;
    if cache.version != INDEX_CACHE_VERSION || cache.mode.as_deref() != mode_label(mode) {
        return None;
    }
    Some(
        cache
            .files
            .into_iter()
            .map(|entry| {
                (
                    entry.uri,
                    IndexEntry {
                        stamp: entry.stamp,
                        symbols: entry.symbols,
                    },
                )
            })
            .collect(),
    )
}

fn mode_label(mode: Option<SyntaxMode>) -> Option<&'static str> {
    match mode {
        Some(SyntaxMode::Indentation) => Some("indentation"),
        Some(SyntaxMode::Braces) => Some("braces"),
        None => None,
    }
}

fn scan_workspace(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    let mut scanned = 0_usize;
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        let mut entries = entries.flatten().collect::<Vec<_>>();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            scanned = scanned.saturating_add(1);
            if scanned > MAX_INDEX_SCAN_ENTRIES || files.len() >= MAX_INDEXED_FILES {
                files.sort();
                return files;
            }
            let path = entry.path();
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if path.file_name().is_some_and(|name| {
                    matches!(name.to_str(), Some("target" | ".git" | "node_modules"))
                }) {
                    continue;
                }
                pending.push(path);
                continue;
            }
            if !metadata.is_file()
                || path.extension().and_then(|extension| extension.to_str()) != Some("ox")
                || metadata.len() > MAX_INDEXED_FILE_BYTES
            {
                continue;
            }
            files.push(path);
        }
    }
    files.sort();
    files
}

fn read_entry(path: &Path, root: &Path, mode: Option<SyntaxMode>) -> Option<IndexEntry> {
    let stamp = file_stamp(path)?;
    read_entry_with_stamp(path, root, mode, stamp)
}

fn read_entry_with_stamp(
    path: &Path,
    root: &Path,
    mode: Option<SyntaxMode>,
    stamp: FileStamp,
) -> Option<IndexEntry> {
    let text = std::fs::read_to_string(path).ok()?;
    if text.len() as u64 > MAX_INDEXED_FILE_BYTES {
        return None;
    }
    let uri = file_uri(path);
    let symbols = workspace_symbols_from_documents(
        &[(uri, text.clone(), None, file_uri(path))],
        Some(root),
        mode,
    );
    Some(IndexEntry {
        stamp: FileStamp {
            hash: fnv1a(text.as_bytes()),
            ..stamp
        },
        symbols,
    })
}

fn file_stamp(path: &Path) -> Option<FileStamp> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .unwrap_or_default();
    Some(FileStamp {
        mtime_seconds: i64::try_from(modified.as_secs()).unwrap_or(i64::MAX),
        mtime_nanos: modified.subsec_nanos(),
        size: metadata.len(),
        hash: 0,
    })
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_workspace(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "oxido-index-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("temporary workspace");
        root
    }

    fn temporary_cache(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "oxido-index-cache-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("temporary cache");
        directory
    }

    fn cleanup(paths: &[&Path]) {
        for path in paths {
            if let Err(error) = std::fs::remove_dir_all(path)
                && path.exists()
            {
                eprintln!("no se pudo limpiar el directorio temporal de prueba: {error}");
            }
        }
    }

    #[test]
    fn persists_and_reuses_symbols_within_the_cache_directory() {
        let root = temporary_workspace("persist");
        std::fs::write(
            root.join("modelo.ox"),
            "sintaxis indentacion\nestructura Alumno:\n    valor: i32\n",
        )
        .expect("fixture");
        let cache = temporary_cache("persist");
        let index = WorkspaceIndex::load(
            Some(root.clone()),
            Some(SyntaxMode::Indentation),
            Some(cache.clone()),
        );
        let symbols = index.symbols(&[], Some(&root), Some(SyntaxMode::Indentation));
        assert!(symbols.iter().any(|symbol| symbol.name == "Alumno"));
        let cache_files = std::fs::read_dir(&cache)
            .expect("cache readable")
            .flatten()
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            })
            .count();
        assert_eq!(cache_files, 1, "la caché se escribe fuera del workspace");
        let second = WorkspaceIndex::load(
            Some(root.clone()),
            Some(SyntaxMode::Indentation),
            Some(cache.clone()),
        );
        assert!(
            second.stats().reused >= 1,
            "la segunda carga reutiliza la caché"
        );
        cleanup(&[&root, &cache]);
    }

    #[test]
    fn invalidates_an_entry_when_content_size_or_mtime_changes() {
        let root = temporary_workspace("invalid");
        let file = root.join("modelo.ox");
        std::fs::write(&file, "sintaxis indentacion\nfuncion vieja():\n    pass\n")
            .expect("fixture");
        let cache = temporary_cache("invalid");
        let first = WorkspaceIndex::load(
            Some(root.clone()),
            Some(SyntaxMode::Indentation),
            Some(cache.clone()),
        );
        assert!(
            first
                .symbols(&[], Some(&root), Some(SyntaxMode::Indentation))
                .iter()
                .any(|symbol| symbol.name == "vieja")
        );
        std::fs::write(&file, "sintaxis indentacion\nfuncion nueva():\n    pass\n")
            .expect("updated fixture");
        let second = WorkspaceIndex::load(
            Some(root.clone()),
            Some(SyntaxMode::Indentation),
            Some(cache.clone()),
        );
        let symbols = second.symbols(&[], Some(&root), Some(SyntaxMode::Indentation));
        assert!(symbols.iter().any(|symbol| symbol.name == "nueva"));
        assert!(!symbols.iter().any(|symbol| symbol.name == "vieja"));
        cleanup(&[&root, &cache]);
    }

    #[test]
    fn never_writes_cache_inside_the_workspace_root() {
        let root = temporary_workspace("inside");
        std::fs::write(
            root.join("main.ox"),
            "sintaxis indentacion\nfuncion principal():\n    pass\n",
        )
        .expect("fixture");
        let index = WorkspaceIndex::load(
            Some(root.clone()),
            Some(SyntaxMode::Indentation),
            Some(root.join("cache")),
        );
        assert!(index.root().is_some());
        assert!(
            !root.join("cache").exists(),
            "la caché no se escribe dentro del workspace"
        );
        cleanup(&[&root]);
    }

    #[test]
    fn ignores_a_cache_with_a_different_schema_version() {
        let root = temporary_workspace("schema");
        std::fs::write(
            root.join("main.ox"),
            "sintaxis indentacion\nfuncion principal():\n    pass\n",
        )
        .expect("fixture");
        let cache = temporary_cache("schema");
        let path = cache_path(&cache, &root);
        let stale = serde_json::json!({
            "version": INDEX_CACHE_VERSION + 1,
            "mode": "indentation",
            "files": []
        });
        std::fs::write(&path, serde_json::to_vec(&stale).expect("json")).expect("stale cache");
        let index = WorkspaceIndex::load(
            Some(root.clone()),
            Some(SyntaxMode::Indentation),
            Some(cache.clone()),
        );
        assert!(index.stats().reindexed >= 1);
        cleanup(&[&root, &cache]);
    }

    #[cfg(unix)]
    #[test]
    fn restricts_cache_permissions_to_the_current_user() {
        use std::os::unix::fs::PermissionsExt;

        let root = temporary_workspace("permissions");
        std::fs::write(
            root.join("main.ox"),
            "sintaxis indentacion\nfuncion principal():\n    pass\n",
        )
        .expect("fixture");
        let cache = temporary_cache("permissions");
        let _index = WorkspaceIndex::load(
            Some(root.clone()),
            Some(SyntaxMode::Indentation),
            Some(cache.clone()),
        );
        let path = cache_path(&cache, &root);
        let file_mode = std::fs::metadata(&path)
            .expect("caché escrita")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600, "la caché solo la lee el usuario");
        let directory_mode = std::fs::metadata(&cache)
            .expect("directorio de caché")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(directory_mode, 0o700, "el directorio es privado");
        cleanup(&[&root, &cache]);
    }
}
