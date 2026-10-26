use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct TestDirectory(std::path::PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("oxido-cli-test-{nonce}"));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "no se pudo limpiar el directorio temporal `{}`: {error}",
                self.0.display()
            );
        }
    }
}

#[test]
fn new_creates_a_runnable_project_and_rejects_an_existing_directory() {
    let temp = TestDirectory::new();
    let project = temp.0.join("proyecto");

    let created = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&temp.0)
        .args(["nuevo", "proyecto"])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(project.join("oxido.toml").is_file());
    let source_path = project.join("src/main.ox");
    let original_source = fs::read_to_string(&source_path).unwrap();
    assert!(original_source.contains("funcion principal():"));

    let executed = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&temp.0)
        .args(["ejecutar", "proyecto/src/main.ox"])
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert!(String::from_utf8_lossy(&executed.stdout).contains("Hola, Óxido."));

    let rejected = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&temp.0)
        .args(["nuevo", "proyecto"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("el directorio ya existe"));
    assert_eq!(fs::read_to_string(source_path).unwrap(), original_source);
}

#[test]
fn test_command_runs_a_real_rust_test_and_rejects_release_mode() {
    let temp = TestDirectory::new();
    let source_path = temp.0.join("suite.ox");
    fs::write(
        &source_path,
        "sintaxis llaves\n#[test]\nfn cli_ejecuta_una_prueba_real() { assert_eq!(2 + 2, 4); }\nfn main() {}\n",
    )
    .unwrap();

    let rejected = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&temp.0)
        .args(["probar", "suite.ox", "--release"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("probar no admite --release"));

    let tested = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&temp.0)
        .args(["probar", "suite.ox"])
        .output()
        .unwrap();
    assert!(
        tested.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&tested.stdout),
        String::from_utf8_lossy(&tested.stderr)
    );
    assert!(
        String::from_utf8_lossy(&tested.stdout).contains("1 passed"),
        "{}",
        String::from_utf8_lossy(&tested.stdout)
    );

    fs::write(
        &source_path,
        "sintaxis llaves\n#[test]\nfn cli_propaga_una_prueba_fallida() { assert_eq!(2 + 2, 5); }\nfn main() {}\n",
    )
    .unwrap();
    let failed_test = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&temp.0)
        .args(["probar", "suite.ox"])
        .output()
        .unwrap();
    assert!(!failed_test.status.success());
    let failure_output = format!(
        "{}{}",
        String::from_utf8_lossy(&failed_test.stdout),
        String::from_utf8_lossy(&failed_test.stderr)
    );
    assert!(
        failure_output.contains("cli_propaga_una_prueba_fallida"),
        "salida de la CLI:\n{failure_output}"
    );
    assert!(
        failure_output.contains("FAILED"),
        "salida de la CLI:\n{failure_output}"
    );
}

#[test]
fn formats_braced_source_idempotently_without_losing_comments() {
    let temp = TestDirectory::new();
    let work = temp.0.join("project");
    fs::create_dir(&work).unwrap();
    let source_path = work.join("comentarios.ox");
    let original = "sintaxis llaves\n// comentario antes del item\n/// documentación del programa\nfuncion main() {\n    let texto = \"cadena // intacta\"; // comentario al final de línea\n    /* comentario de bloque */\n    println!(\"{}\", texto);\n}\n// comentario al final del archivo\n";
    fs::write(&source_path, original).unwrap();

    let check_before = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["formato", "comentarios.ox", "--comprobar"])
        .output()
        .unwrap();
    assert!(!check_before.status.success());
    assert_eq!(fs::read_to_string(&source_path).unwrap(), original);

    let formatted = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["formato", "comentarios.ox"])
        .output()
        .unwrap();
    assert!(
        formatted.status.success(),
        "{}",
        String::from_utf8_lossy(&formatted.stderr)
    );
    let first_result = fs::read_to_string(&source_path).unwrap();
    assert!(first_result.starts_with("sintaxis llaves\n"));
    for comment in [
        "// comentario antes del item",
        "documentación del programa",
        "// comentario al final de línea",
        "/* comentario de bloque */",
        "// comentario al final del archivo",
        "cadena // intacta",
    ] {
        assert!(
            first_result.contains(comment),
            "falta {comment:?}:\n{first_result}"
        );
    }
    assert!(
        first_result.find("// comentario antes del item")
            < first_result.find("documentación del programa")
    );
    assert!(first_result.find("documentación del programa") < first_result.find("funcion main"));
    assert!(
        first_result.contains("let texto = \"cadena // intacta\"; // comentario al final de línea")
    );
    assert!(
        first_result.find("// comentario al final del archivo") > first_result.find("funcion main")
    );

    let check_after = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["formato", "comentarios.ox", "--comprobar"])
        .output()
        .unwrap();
    assert!(
        check_after.status.success(),
        "{}",
        String::from_utf8_lossy(&check_after.stderr)
    );

    let formatted_again = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["formato", "comentarios.ox"])
        .output()
        .unwrap();
    assert!(
        formatted_again.status.success(),
        "{}",
        String::from_utf8_lossy(&formatted_again.stderr)
    );
    assert_eq!(fs::read_to_string(&source_path).unwrap(), first_result);
}

#[test]
fn exports_compiles_and_runs_the_hello_world_fixture() {
    let temp = TestDirectory::new();
    let work = temp.0.join("project");
    fs::create_dir(&work).unwrap();
    fs::write(
        work.join("hola.ox"),
        include_str!("../../../examples/hola_mundo.ox"),
    )
    .unwrap();

    let exported = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["exportar", "hola.ox", "--salida", "hola.rs"])
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    assert!(work.join("hola.rs").is_file());
    assert!(work.join("hola.oxido-map.json").is_file());
    let exported_rust = fs::read_to_string(work.join("hola.rs")).unwrap();
    assert!(exported_rust.contains("Documentación del crate del ejemplo."));
    assert!(exported_rust.contains("Punto de entrada del programa de ejemplo."));
    assert!(exported_rust.contains("// Este comentario ordinario se pierde al importar"));
    assert!(exported_rust.contains("// Comentario ordinario dentro del cuerpo"));
    let map = fs::read_to_string(work.join("hola.oxido-map.json")).unwrap();
    assert!(map.contains("\"sourceFile\": \"hola.ox\""));
    assert!(map.contains("\"generatedFile\": \"hola.rs\""));
    assert!(!map.contains(work.to_str().unwrap()));

    let imported = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["importar", "hola.rs", "--salida", "hola-vuelta.ox"])
        .output()
        .unwrap();
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let imported_ox = fs::read_to_string(work.join("hola-vuelta.ox")).unwrap();
    assert!(imported_ox.contains("//!Documentación del crate del ejemplo."));
    assert!(imported_ox.contains("///Punto de entrada del programa de ejemplo."));
    assert!(imported_ox.contains("// Este comentario ordinario se pierde al importar desde Rust."));
    assert!(
        imported_ox
            .contains("// Comentario ordinario dentro del cuerpo; no forma parte del round trip.")
    );
    let exported_again = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["exportar", "hola-vuelta.ox", "--salida", "hola-vuelta.rs"])
        .output()
        .unwrap();
    assert!(
        exported_again.status.success(),
        "{}",
        String::from_utf8_lossy(&exported_again.stderr)
    );
    assert_eq!(
        fs::read_to_string(work.join("hola.rs")).unwrap(),
        fs::read_to_string(work.join("hola-vuelta.rs")).unwrap()
    );

    let compiled = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["compilar", "hola.ox"])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let executed = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["ejecutar", "hola.ox"])
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&executed.stdout).trim(),
        "Hola, mundo."
    );
}

#[test]
fn compiles_runs_and_recursively_exports_a_hierarchical_ox_project() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let fixture = repository.join("prueba/modulos_jerarquicos");
    let root_source = fixture.join("fuente/main.ox");
    let temp_path = repository.join("target").join(format!(
        "oxido-cli-modules-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_path).unwrap();
    let temp = TestDirectory(temp_path);

    let compiled = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&repository)
        .args(["compilar", root_source.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let executed = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&repository)
        .args(["ejecutar", root_source.to_str().unwrap(), "--release"])
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    let expected = "Ana aprobó: true\n";
    assert_eq!(String::from_utf8_lossy(&executed.stdout), expected);

    let output_source = temp.0.join("rust/src/main.rs");
    fs::create_dir_all(output_source.parent().unwrap()).unwrap();
    let exported = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&repository)
        .args([
            "exportar",
            root_source.to_str().unwrap(),
            "--salida",
            output_source.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    for generated in [
        "rust/src/main.rs",
        "rust/src/main.oxido-map.json",
        "rust/src/aplicacion.rs",
        "rust/src/aplicacion.oxido-map.json",
        "rust/src/aplicacion/servicios.rs",
        "rust/src/aplicacion/servicios.oxido-map.json",
        "rust/src/dominio.rs",
        "rust/src/dominio.oxido-map.json",
        "rust/src/dominio/modelos.rs",
        "rust/src/dominio/modelos.oxido-map.json",
    ] {
        assert!(temp.0.join(generated).is_file(), "falta {generated}");
    }
    let generated_map =
        fs::read_to_string(temp.0.join("rust/src/dominio/modelos.oxido-map.json")).unwrap();
    assert!(generated_map.contains("fuente/dominio/modelos.ox"));
    assert!(!generated_map.contains(repository.to_str().unwrap()));

    let second_source = temp.0.join("rust-second/src/main.rs");
    fs::create_dir_all(second_source.parent().unwrap()).unwrap();
    let exported_again = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&repository)
        .args([
            "exportar",
            root_source.to_str().unwrap(),
            "--salida",
            second_source.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        exported_again.status.success(),
        "{}",
        String::from_utf8_lossy(&exported_again.stderr)
    );
    for relative in [
        "src/main.rs",
        "src/main.oxido-map.json",
        "src/aplicacion.rs",
        "src/aplicacion.oxido-map.json",
        "src/aplicacion/servicios.rs",
        "src/aplicacion/servicios.oxido-map.json",
        "src/dominio.rs",
        "src/dominio.oxido-map.json",
        "src/dominio/modelos.rs",
        "src/dominio/modelos.oxido-map.json",
    ] {
        assert_eq!(
            fs::read(temp.0.join("rust").join(relative)).unwrap(),
            fs::read(temp.0.join("rust-second").join(relative)).unwrap(),
            "la exportación no fue reproducible para {relative}"
        );
    }
    let second_attempt = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&repository)
        .args([
            "exportar",
            root_source.to_str().unwrap(),
            "--salida",
            output_source.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!second_attempt.status.success());
    assert_eq!(
        fs::read(&output_source).unwrap(),
        fs::read(&second_source).unwrap()
    );

    let manifest = temp.0.join("rust/Cargo.toml");
    fs::write(
        &manifest,
        "[package]\nname = \"module-export-check\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
    )
    .unwrap();
    let target_dir = temp.0.join("cargo-target");
    let generate_lock = Command::new("cargo")
        .current_dir(&repository)
        .env("CARGO_TARGET_DIR", &target_dir)
        .args([
            "generate-lockfile",
            "--offline",
            "--manifest-path",
            manifest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        generate_lock.status.success(),
        "{}",
        String::from_utf8_lossy(&generate_lock.stderr)
    );
    let built = Command::new("cargo")
        .current_dir(&repository)
        .env("CARGO_TARGET_DIR", &target_dir)
        .args([
            "build",
            "--release",
            "--locked",
            "--offline",
            "--manifest-path",
            manifest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let binary = target_dir.join("release").join(format!(
        "module-export-check{}",
        std::env::consts::EXE_SUFFIX
    ));
    let exported_run = Command::new(binary).output().unwrap();
    assert!(exported_run.status.success());
    assert_eq!(String::from_utf8_lossy(&exported_run.stdout), expected);

    // The checked-in `rust/` tree is the file-by-file export route. Keep it
    // equivalent to recursive export and compile/run both trees independently.
    for relative in [
        "src/main.rs",
        "src/aplicacion.rs",
        "src/aplicacion/servicios.rs",
        "src/dominio.rs",
        "src/dominio/modelos.rs",
    ] {
        assert_eq!(
            fs::read(fixture.join("rust").join(relative)).unwrap(),
            fs::read(temp.0.join("rust").join(relative)).unwrap(),
            "la exportación archivo por archivo difiere en {relative}"
        );
    }
    let manual_manifest = fixture.join("rust/Cargo.toml");
    let manual_target = temp.0.join("cargo-target-file-by-file");
    let manual_build = Command::new("cargo")
        .current_dir(&repository)
        .env("CARGO_TARGET_DIR", &manual_target)
        .args([
            "build",
            "--release",
            "--locked",
            "--offline",
            "--manifest-path",
            manual_manifest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        manual_build.status.success(),
        "{}",
        String::from_utf8_lossy(&manual_build.stderr)
    );
    let manual_binary = manual_target.join("release").join(format!(
        "modulos-jerarquicos{}",
        std::env::consts::EXE_SUFFIX
    ));
    let manual_run = Command::new(manual_binary).output().unwrap();
    assert!(manual_run.status.success());
    assert_eq!(String::from_utf8_lossy(&manual_run.stdout), expected);
}

#[test]
fn resolves_module_ambiguity_and_maps_rustc_errors_to_the_module_ox_file() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let test_path = repository.join("target").join(format!(
        "oxido-cli-module-diagnostics-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&test_path).unwrap();
    let temp = TestDirectory(test_path);
    let main = write_module_fixture(&temp.0.join("mapped"), Some("alumno.nota >= \"seis\""));

    let compile = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&repository)
        .args(["compilar", main.to_str().unwrap()])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(!compile.status.success());
    assert!(
        stderr.contains("fuente/aplicacion/servicios.ox:5:5"),
        "{stderr}"
    );

    let ambiguous_root = temp.0.join("ambiguous");
    let ambiguous_main = write_module_fixture(&ambiguous_root, Some("alumno.nota >= 6.0"));
    fs::write(
        ambiguous_root.join("fuente/aplicacion/mod.ox"),
        "sintaxis llaves\n",
    )
    .unwrap();
    let ambiguous = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&repository)
        .args(["compilar", ambiguous_main.to_str().unwrap()])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&ambiguous.stderr);
    assert!(!ambiguous.status.success());
    assert!(stderr.contains("OX8001"), "{stderr}");
    assert!(stderr.contains("ambig"), "{stderr}");
}

#[test]
fn resolves_path_modules_and_exports_their_rewritten_rust_paths() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let root = repository.join("target").join(format!(
        "oxido-cli-path-modules-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("fuente/modulos/custom")).unwrap();
    fs::write(
        root.join("oxido.toml"),
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("fuente/main.ox"),
        "#[path = \"modulos/custom.ox\"] mod app;\nfn main() { println!(\"{}\", app::mensaje()); }\n",
    )
    .unwrap();
    fs::write(
        root.join("fuente/modulos/custom.ox"),
        "#[path = \"../shared.ox\"] mod shared;\npub fn mensaje() -> &'static str { shared::mensaje() }\n",
    )
    .unwrap();
    fs::write(
        root.join("fuente/shared.ox"),
        "pub fn mensaje() -> &'static str { \"#[path] funciona\" }\n",
    )
    .unwrap();
    let temp = TestDirectory(root.clone());
    let root_source = root.join("fuente/main.ox");

    let executed = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&repository)
        .args(["ejecutar", root_source.to_str().unwrap(), "--release"])
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&executed.stdout),
        "#[path] funciona\n"
    );

    let output_main = temp.0.join("export/src/main.rs");
    fs::create_dir_all(output_main.parent().unwrap()).unwrap();
    let exported = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&repository)
        .args([
            "exportar",
            root_source.to_str().unwrap(),
            "--salida",
            output_main.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let exported_root = fs::read_to_string(&output_main).unwrap();
    let exported_nested = fs::read_to_string(temp.0.join("export/src/modulos/custom.rs")).unwrap();
    assert!(exported_root.contains("#[path = \"modulos/custom.rs\"]"));
    assert!(exported_nested.contains("#[path = \"../shared.rs\"]"));
    assert!(temp.0.join("export/src/shared.rs").is_file());

    let manifest = temp.0.join("export/Cargo.toml");
    fs::write(
        &manifest,
        "[package]\nname = \"path-module-export\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
    )
    .unwrap();
    let target = temp.0.join("export-target");
    let lock = Command::new("cargo")
        .env("CARGO_TARGET_DIR", &target)
        .args([
            "generate-lockfile",
            "--offline",
            "--manifest-path",
            manifest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let built = Command::new("cargo")
        .env("CARGO_TARGET_DIR", &target)
        .args([
            "build",
            "--release",
            "--locked",
            "--offline",
            "--manifest-path",
            manifest.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let binary = target.join("release").join(format!(
        "path-module-export{}",
        std::env::consts::EXE_SUFFIX
    ));
    let exported_run = Command::new(binary).output().unwrap();
    assert!(exported_run.status.success());
    assert_eq!(
        String::from_utf8_lossy(&exported_run.stdout),
        "#[path] funciona\n"
    );
}

#[test]
fn rejects_path_modules_that_escape_the_project_root() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let temp = TestDirectory::new();
    let project = temp.0.join("project");
    fs::create_dir_all(project.join("fuente")).unwrap();
    fs::write(
        project.join("oxido.toml"),
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        project.join("fuente/main.ox"),
        "#[path = \"../../outside.ox\"] mod modulo_fuera;\nfn main() {}\n",
    )
    .unwrap();
    fs::write(temp.0.join("outside.ox"), "pub fn dato() {}\n").unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&repository)
        .args(["compilar", project.join("fuente/main.ox").to_str().unwrap()])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success());
    assert!(stderr.contains("OX8001"), "{stderr}");
    assert!(stderr.contains("sale de la raíz del proyecto"), "{stderr}");
}

fn write_module_fixture(root: &std::path::Path, service_body: Option<&str>) -> std::path::PathBuf {
    fs::create_dir_all(root.join("fuente/aplicacion")).unwrap();
    fs::create_dir_all(root.join("fuente/dominio")).unwrap();
    fs::write(
        root.join("oxido.toml"),
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("fuente/main.ox"),
        include_str!("../../../prueba/modulos_jerarquicos/fuente/main.ox"),
    )
    .unwrap();
    fs::write(
        root.join("fuente/aplicacion.ox"),
        include_str!("../../../prueba/modulos_jerarquicos/fuente/aplicacion.ox"),
    )
    .unwrap();
    let services =
        include_str!("../../../prueba/modulos_jerarquicos/fuente/aplicacion/servicios.ox");
    let services = service_body.map_or_else(
        || services.to_owned(),
        |body| services.replace("alumno.nota >= 6.0", body),
    );
    fs::write(root.join("fuente/aplicacion/servicios.ox"), services).unwrap();
    fs::write(
        root.join("fuente/dominio.ox"),
        include_str!("../../../prueba/modulos_jerarquicos/fuente/dominio.ox"),
    )
    .unwrap();
    fs::write(
        root.join("fuente/dominio/modelos.ox"),
        include_str!("../../../prueba/modulos_jerarquicos/fuente/dominio/modelos.ox"),
    )
    .unwrap();
    root.join("fuente/main.ox")
}

#[test]
fn reads_project_mode_from_declarative_oxido_toml() {
    let temp = TestDirectory::new();
    let work = temp.0.join("project");
    fs::create_dir(&work).unwrap();
    fs::write(
        work.join("oxido.toml"),
        "[language]\nsyntax = \"indentation\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        work.join("hola.ox"),
        "funcion main():\n    println!(\"Hola, mundo.\");\n",
    )
    .unwrap();

    let compiled = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["compilar", "hola.ox"])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
}

#[test]
fn compiles_and_runs_with_a_declared_local_cargo_dependency_offline() {
    let temp = TestDirectory::new();
    let work = temp.0.join("project");
    let dependency = work.join("vendor/saludos_local");
    fs::create_dir_all(dependency.join("src")).unwrap();
    fs::write(
        dependency.join("Cargo.toml"),
        "[package]\nname = \"saludos_local\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        dependency.join("src/lib.rs"),
        "#[macro_export]\nmacro_rules! macro_externa { ($valor:expr) => { println!(\"macro={}:{}\", $valor, $valor); }; }\npub fn saludo() -> &'static str { \"desde un crate local\" }\n",
    )
    .unwrap();
    fs::create_dir_all(&work).unwrap();
    fs::write(
        work.join("oxido.toml"),
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n\n[dependencies.saludos_local]\npath = \"vendor/saludos_local\"\n",
    )
    .unwrap();
    fs::write(
        work.join("principal.ox"),
        "sintaxis llaves\nfn main() { saludos_local::macro_externa!(\"sin modificar\"); println!(\"{}\", saludos_local::saludo()); }\n",
    )
    .unwrap();

    let fetched = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["descargar-dependencias", "principal.ox"])
        .output()
        .unwrap();
    assert!(
        !fetched.status.success(),
        "la fuente path requiere política explícita"
    );
    assert!(String::from_utf8_lossy(&fetched.stderr).contains("--permitir-rutas-locales"));

    let without_lock = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["compilar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(!without_lock.status.success());
    assert!(String::from_utf8_lossy(&without_lock.stderr).contains("[OX7001]"));
    assert!(!work.join("Cargo.lock").exists());

    let preview = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args([
            "descargar-dependencias",
            "principal.ox",
            "--permitir-rutas-locales",
        ])
        .output()
        .unwrap();
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert!(!work.join("Cargo.lock").exists());

    let fetched = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args([
            "descargar-dependencias",
            "principal.ox",
            "--permitir-rutas-locales",
            "--confirmar",
        ])
        .output()
        .unwrap();
    assert!(
        fetched.status.success(),
        "{}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    assert!(work.join("Cargo.lock").is_file());

    let compiled = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["compilar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let executed = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["ejecutar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&executed.stdout).trim(),
        "macro=sin modificar:sin modificar\ndesde un crate local"
    );
}

#[test]
fn dependency_fetch_never_runs_build_scripts_or_proc_macros_without_explicit_trust() {
    let temp = TestDirectory::new();
    let project = temp.0.join("project");
    let build_crate = project.join("vendor/build_fixture");
    let macro_crate = project.join("vendor/macro_fixture");
    let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/cargo-execution");
    copy_directory(&fixture_root.join("build-script"), &build_crate);
    copy_directory(&fixture_root.join("proc-macro"), &macro_crate);
    fs::write(
        project.join("oxido.toml"),
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n\n[dependencies.build_fixture]\npath = \"vendor/build_fixture\"\n\n[dependencies.macro_fixture]\npath = \"vendor/macro_fixture\"\n",
    )
    .unwrap();
    fs::write(
        project.join("principal.ox"),
        "sintaxis llaves\n#[macro_fixture::identity]\nfn main() { build_fixture::marker(); println!(\"{}\", macro_fixture::token_text!(si tipo verdadero)); }\n",
    )
    .unwrap();
    let build_marker = temp.0.join("build-marker");
    let macro_marker = temp.0.join("macro-marker");

    let untrusted = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&project)
        .args(["compilar", "principal.ox"])
        .output()
        .unwrap();
    assert!(!untrusted.status.success());
    assert!(String::from_utf8_lossy(&untrusted.stderr).contains("[OX7002]"));
    assert!(String::from_utf8_lossy(&untrusted.stderr).contains("--confiar-dependencias"));
    assert!(!build_marker.exists());
    assert!(!macro_marker.exists());

    let preview = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&project)
        .args([
            "descargar-dependencias",
            "principal.ox",
            "--permitir-rutas-locales",
        ])
        .output()
        .unwrap();
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert!(!project.join("Cargo.lock").exists());
    assert!(!build_marker.exists());
    assert!(!macro_marker.exists());

    let fetched = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&project)
        .args([
            "descargar-dependencias",
            "principal.ox",
            "--permitir-rutas-locales",
            "--confirmar",
        ])
        .output()
        .unwrap();
    assert!(
        fetched.status.success(),
        "{}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    assert!(!build_marker.exists());
    assert!(!macro_marker.exists());

    let trusted = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&project)
        .env("OXIDO_BUILD_MARKER", &build_marker)
        .env("OXIDO_PROC_MARKER", &macro_marker)
        .args(["ejecutar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(
        trusted.status.success(),
        "{}",
        String::from_utf8_lossy(&trusted.stderr)
    );
    assert_eq!(fs::read_to_string(build_marker).unwrap(), "build");
    assert_eq!(fs::read_to_string(macro_marker).unwrap(), "proc-macro");
    assert_eq!(
        String::from_utf8_lossy(&trusted.stdout).trim(),
        "si tipo verdadero"
    );
}

#[test]
fn compiles_a_registry_crate_offline_and_persists_the_project_lockfile() {
    let temp = TestDirectory::new();
    let work = temp.0.join("project");
    fs::create_dir(&work).unwrap();
    let cargo_home = temp.0.join("cargo-home");
    fs::create_dir(&cargo_home).unwrap();
    let vendor = fs::canonicalize(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/cargo-registry/vendor"),
    )
    .unwrap();
    let vendor_text = vendor.to_str().unwrap();
    fs::write(
        cargo_home.join("config.toml"),
        format!(
            "[source.crates-io]\nreplace-with = \"fixture\"\n\n[source.fixture]\ndirectory = {vendor_text:?}\n"
        ),
    )
    .unwrap();
    fs::write(
        work.join("oxido.toml"),
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n\n[dependencies.itoa]\nversion = \"1.0\"\n\n[dependencies.futures-executor]\nversion = \"0.3.32\"\n\n[dependencies.paste]\nversion = \"1\"\n\n[dependencies.serde]\nversion = \"1\"\nfeatures = [\"derive\"]\n\n[dependencies.serde_json]\nversion = \"1\"\n\n[dependencies.tokio]\nversion = \"1\"\nfeatures = [\"macros\", \"rt-multi-thread\"]\n",
    )
    .unwrap();
    fs::write(
        work.join("principal.ox"),
        "sintaxis llaves\nasync fn resultado() -> i32 { 42 }\nfn main() { let mut buffer = itoa::Buffer::new(); let resultado = futures_executor::block_on(resultado()); println!(\"{}:{}\", buffer.format(42), resultado); }\n",
    )
    .unwrap();

    let expected_lock = include_str!("../../../tests/fixtures/cargo-registry/Cargo.lock");
    let second_work = temp.0.join("other-working-directory");
    fs::create_dir(&second_work).unwrap();
    fs::copy(work.join("oxido.toml"), second_work.join("oxido.toml")).unwrap();
    fs::copy(work.join("principal.ox"), second_work.join("principal.ox")).unwrap();
    let first_export = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .args(["exportar", "principal.ox", "--salida", "primera.rs"])
        .output()
        .unwrap();
    assert!(first_export.status.success());
    let second_export = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&second_work)
        .args(["exportar", "principal.ox", "--salida", "segunda.rs"])
        .output()
        .unwrap();
    assert!(second_export.status.success());
    assert_eq!(
        fs::read_to_string(work.join("primera.rs")).unwrap(),
        fs::read_to_string(second_work.join("segunda.rs")).unwrap()
    );
    let generated_rust = fs::read_to_string(work.join("primera.rs")).unwrap();
    assert!(!generated_rust.contains(work.to_str().unwrap()));
    assert!(!generated_rust.contains(second_work.to_str().unwrap()));

    let without_lock = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .env("CARGO_HOME", &cargo_home)
        .args(["compilar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(!without_lock.status.success());
    assert!(
        String::from_utf8_lossy(&without_lock.stderr).contains("[OX7001]"),
        "{}",
        String::from_utf8_lossy(&without_lock.stderr)
    );
    assert!(String::from_utf8_lossy(&without_lock.stderr).contains("descargar-dependencias"));
    assert!(!work.join("Cargo.lock").exists());

    let plan = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["descargar-dependencias", "principal.ox"])
        .output()
        .unwrap();
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    assert!(String::from_utf8_lossy(&plan.stdout).contains("futures-executor@0.3.34"));
    assert!(String::from_utf8_lossy(&plan.stdout).contains("Cambios en Cargo.lock"));
    assert!(String::from_utf8_lossy(&plan.stdout).contains("checksum="));
    assert!(
        !work.join("Cargo.lock").exists(),
        "la vista previa no debe escribir Cargo.lock"
    );

    let fetched = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["descargar-dependencias", "principal.ox", "--confirmar"])
        .output()
        .unwrap();
    assert!(
        fetched.status.success(),
        "{}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    assert_eq!(
        fs::read_to_string(work.join("Cargo.lock")).unwrap(),
        expected_lock
    );
    let update_preview = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["descargar-dependencias", "principal.ox", "--actualizar"])
        .output()
        .unwrap();
    assert!(
        update_preview.status.success(),
        "{}",
        String::from_utf8_lossy(&update_preview.stderr)
    );
    assert!(String::from_utf8_lossy(&update_preview.stdout).contains("Cargo.lock: sin cambios"));
    assert_eq!(
        fs::read_to_string(work.join("Cargo.lock")).unwrap(),
        expected_lock
    );

    let compiled = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .env("CARGO_HOME", &cargo_home)
        .args(["compilar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let lockfile = work.join("Cargo.lock");
    assert!(
        lockfile.is_file(),
        "Cargo.lock debe persistir en el proyecto"
    );
    let lock = fs::read_to_string(&lockfile).unwrap();
    assert!(lock.contains("name = \"itoa\""));
    assert_eq!(lock, expected_lock, "--locked no debe cambiar el lockfile");
    assert!(!lock.contains(work.to_str().unwrap()));
    let lock_data: toml::Value = toml::from_str(&lock).unwrap();
    let packages = lock_data["package"].as_array().unwrap();
    for package in packages {
        let Some(name) = package.get("name").and_then(toml::Value::as_str) else {
            continue;
        };
        let Some(version) = package.get("version").and_then(toml::Value::as_str) else {
            continue;
        };
        let Ok(checksum_file) = fs::read_to_string(vendor.join(name).join(".cargo-checksum.json"))
        else {
            continue;
        };
        let checksum_data: serde_json::Value = serde_json::from_str(&checksum_file).unwrap();
        let package_checksum = checksum_data["package"].as_str().unwrap();
        assert!(
            lock.contains(&format!("name = \"{name}\"\nversion = \"{version}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{package_checksum}\"")),
            "el checksum de {name} {version} debe coincidir entre vendor y Cargo.lock"
        );
    }

    let executed = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&work)
        .env("CARGO_HOME", &cargo_home)
        .args(["ejecutar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&executed.stdout).trim(), "42:42");
}

#[test]
fn interoperates_with_registry_proc_macros_in_direct_and_exported_module_trees() {
    let temp = TestDirectory::new();
    let project = temp.0.join("project");
    let cargo_home = temp.0.join("cargo-home");
    let vendor = fs::canonicalize(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/cargo-registry/vendor"),
    )
    .unwrap();
    fs::create_dir_all(&cargo_home).unwrap();
    let vendor_text = vendor.to_str().unwrap();
    fs::write(
        cargo_home.join("config.toml"),
        format!(
            "[source.crates-io]\nreplace-with = \"fixture\"\n\n[source.fixture]\ndirectory = {vendor_text:?}\n"
        ),
    )
    .unwrap();
    fs::create_dir_all(project.join("modulos")).unwrap();
    fs::write(
        project.join("oxido.toml"),
        "[language]\nsyntax = \"indentation\"\nedition = \"2024\"\n\n[dependencies.paste]\nversion = \"1\"\n\n[dependencies.serde]\nversion = \"1\"\nfeatures = [\"derive\"]\n\n[dependencies.serde_json]\nversion = \"1\"\n\n[dependencies.tokio]\nversion = \"1\"\nfeatures = [\"macros\", \"rt-multi-thread\"]\n",
    )
    .unwrap();
    fs::write(
        project.join("principal.ox"),
        "sintaxis indentacion\n#[path = \"modulos/datos.ox\"]\nmodulo datos;\nmacro_rules! conservar { ($($token:tt)*) => { stringify!($($token)*) }; }\n#[tokio::main(flavor = \"current_thread\")]\nasincrono funcion principal():\n    dejar dato = datos::crear()\n    dejar serializado = serde_json::to_string(&dato).expect(\"serialización\")\n    dejar recuperado: datos::Dato = serde_json::from_str(&serializado).expect(\"deserialización\")\n    dejar seleccionado = tokio::select! {\n        valor = async { recuperado.valor } => valor,\n    }\n    imprimir!(\"{}:{}:{}:{}:{}\", serializado, seleccionado, conservar!(si tipo verdadero), serde_json::json!({\"si\": true, \"tipo\": \"texto\"}), datos::macro_si());\n",
    )
    .unwrap();
    fs::write(
        project.join("modulos/datos.ox"),
        "sintaxis indentacion\n#[path = \"auxiliar.rs\"]\nmodulo auxiliar;\npaste::paste! {\n    pub fn [<macro_ si>]() -> &'static str {\n        stringify!({ identificador: [tipo, verdadero], texto: \"si tipo verdadero\" })\n    }\n}\n#[derive(serde::Serialize, serde::Deserialize)]\npub estructura Dato:\n    #[serde(rename = \"valor_especial\")]\n    pub valor: i32\npub funcion crear() -> Dato:\n    Dato { valor: auxiliar::valor() }\n",
    )
    .unwrap();
    fs::write(
        project.join("modulos/auxiliar.rs"),
        "pub fn valor() -> i32 { 42 }\n",
    )
    .unwrap();

    let fetched = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&project)
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["descargar-dependencias", "principal.ox", "--confirmar"])
        .output()
        .unwrap();
    assert!(
        fetched.status.success(),
        "{}",
        String::from_utf8_lossy(&fetched.stderr)
    );

    let compiled = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&project)
        .env("CARGO_HOME", &cargo_home)
        .args(["compilar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&project)
        .env("CARGO_HOME", &cargo_home)
        .args(["ejecutar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}",
        String::from_utf8_lossy(&executed.stderr)
    );
    let stdout = String::from_utf8_lossy(&executed.stdout);
    assert!(
        stdout.contains(r#"{"valor_especial":42}:42:si tipo verdadero:{"si":true,"tipo":"texto"}:{ identificador : [tipo, verdadero], texto : "si tipo verdadero" }"#),
        "{stdout}"
    );

    let export_source_tree = project.join("exportado");
    fs::create_dir_all(export_source_tree.join("src")).unwrap();
    let export = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&project)
        .args([
            "exportar",
            "principal.ox",
            "--salida",
            "exportado/src/main.rs",
        ])
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    fs::write(
        export_source_tree.join("Cargo.toml"),
        "[package]\nname = \"oxido_generated\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n\n[dependencies]\npaste = \"1\"\nserde = { version = \"1\", features = [\"derive\"] }\nserde_json = \"1\"\ntokio = { version = \"1\", features = [\"macros\", \"rt-multi-thread\"] }\n",
    )
    .unwrap();
    fs::copy(
        project.join("Cargo.lock"),
        export_source_tree.join("Cargo.lock"),
    )
    .unwrap();
    let exported_project = temp.0.join("exportado-independiente");
    copy_directory(
        &export_source_tree.join("src"),
        &exported_project.join("src"),
    );
    fs::copy(
        export_source_tree.join("Cargo.toml"),
        exported_project.join("Cargo.toml"),
    )
    .unwrap();
    fs::copy(
        export_source_tree.join("Cargo.lock"),
        exported_project.join("Cargo.lock"),
    )
    .unwrap();
    remove_source_maps(&exported_project.join("src"));
    let independent_build = Command::new("cargo")
        .arg("build")
        .arg("--locked")
        .arg("--offline")
        .arg("--manifest-path")
        .arg(exported_project.join("Cargo.toml"))
        .current_dir(&exported_project)
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_TARGET_DIR", exported_project.join("target"))
        .output()
        .unwrap();
    assert!(
        independent_build.status.success(),
        "{}",
        String::from_utf8_lossy(&independent_build.stderr)
    );
    let independent_run = Command::new("cargo")
        .arg("run")
        .arg("--locked")
        .arg("--offline")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(exported_project.join("Cargo.toml"))
        .current_dir(&exported_project)
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_TARGET_DIR", exported_project.join("target"))
        .output()
        .unwrap();
    assert!(
        independent_run.status.success(),
        "{}",
        String::from_utf8_lossy(&independent_run.stderr)
    );
    assert_eq!(independent_run.stdout, executed.stdout);
    assert!(
        !fs::read_to_string(exported_project.join("src/main.rs"))
            .unwrap()
            .contains(project.to_str().unwrap())
    );

    let valid_source = fs::read_to_string(project.join("principal.ox")).unwrap();
    let invalid_source = valid_source.replace(
        "serde_json::json!({\"si\": true, \"tipo\": \"texto\"})",
        "serde_json::json!(@)",
    );
    assert_ne!(valid_source, invalid_source);
    fs::write(project.join("principal.ox"), invalid_source).unwrap();
    let invalid_macro = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&project)
        .env("CARGO_HOME", &cargo_home)
        .args(["compilar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    let diagnostics = String::from_utf8_lossy(&invalid_macro.stderr);
    assert!(!invalid_macro.status.success());
    assert!(diagnostics.contains("principal.ox:"), "{diagnostics}");
    assert!(diagnostics.contains("error"), "{diagnostics}");

    let bad_export_source_tree = project.join("exportacion-fallida");
    fs::create_dir_all(bad_export_source_tree.join("src")).unwrap();
    let bad_export = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&project)
        .args([
            "exportar",
            "principal.ox",
            "--salida",
            "exportacion-fallida/src/main.rs",
        ])
        .output()
        .unwrap();
    assert!(
        bad_export.status.success(),
        "{}",
        String::from_utf8_lossy(&bad_export.stderr)
    );
    fs::write(
        bad_export_source_tree.join("Cargo.toml"),
        "[package]\nname = \"oxido_generated\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n\n[dependencies]\npaste = \"1\"\nserde = { version = \"1\", features = [\"derive\"] }\nserde_json = \"1\"\ntokio = { version = \"1\", features = [\"macros\", \"rt-multi-thread\"] }\n",
    )
    .unwrap();
    fs::copy(
        project.join("Cargo.lock"),
        bad_export_source_tree.join("Cargo.lock"),
    )
    .unwrap();
    let bad_exported_project = temp.0.join("exportacion-fallida-independiente");
    copy_directory(
        &bad_export_source_tree.join("src"),
        &bad_exported_project.join("src"),
    );
    fs::copy(
        bad_export_source_tree.join("Cargo.toml"),
        bad_exported_project.join("Cargo.toml"),
    )
    .unwrap();
    fs::copy(
        bad_export_source_tree.join("Cargo.lock"),
        bad_exported_project.join("Cargo.lock"),
    )
    .unwrap();
    remove_source_maps(&bad_exported_project.join("src"));
    let exported_error = Command::new("cargo")
        .args([
            "check",
            "--locked",
            "--offline",
            "--message-format=human",
            "--manifest-path",
        ])
        .arg(bad_exported_project.join("Cargo.toml"))
        .current_dir(&bad_exported_project)
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_TARGET_DIR", bad_exported_project.join("target"))
        .output()
        .unwrap();
    let exported_diagnostics = String::from_utf8_lossy(&exported_error.stderr);
    assert!(!exported_error.status.success());
    assert!(
        exported_diagnostics.contains("src/main.rs:"),
        "{exported_diagnostics}"
    );
}

#[test]
fn cargo_detects_modified_vendor_and_lockfile_checksums_during_compile() {
    let temp = TestDirectory::new();
    let fixture_vendor = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/cargo-registry/vendor");
    let altered_vendor = temp.0.join("altered-vendor");
    copy_directory(&fixture_vendor, &altered_vendor);
    let itoa_source = altered_vendor.join("itoa/src/lib.rs");
    let mut contents = fs::read_to_string(&itoa_source).unwrap();
    contents.push_str("\n// checksum regression fixture\n");
    fs::write(itoa_source, contents).unwrap();

    let altered_project = temp.0.join("altered-project");
    let altered_home = temp.0.join("altered-cargo-home");
    create_registry_project(&altered_project, &altered_home, &altered_vendor);
    let altered_plan = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&altered_project)
        .env("CARGO_HOME", &altered_home)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["descargar-dependencias", "principal.ox"])
        .output()
        .unwrap();
    assert!(altered_plan.status.success());
    let altered_fetch = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&altered_project)
        .env("CARGO_HOME", &altered_home)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["descargar-dependencias", "principal.ox", "--confirmar"])
        .output()
        .unwrap();
    assert!(
        altered_fetch.status.success(),
        "{}",
        String::from_utf8_lossy(&altered_fetch.stderr)
    );
    let altered_compile = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&altered_project)
        .env("CARGO_HOME", &altered_home)
        .args(["compilar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(!altered_compile.status.success());
    assert!(String::from_utf8_lossy(&altered_compile.stderr).contains("checksum"));
    assert!(altered_project.join("Cargo.lock").is_file());

    let valid_vendor = fs::canonicalize(fixture_vendor).unwrap();
    let checksum_project = temp.0.join("checksum-project");
    let checksum_home = temp.0.join("checksum-cargo-home");
    create_registry_project(&checksum_project, &checksum_home, &valid_vendor);
    let plan = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&checksum_project)
        .env("CARGO_HOME", &checksum_home)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["descargar-dependencias", "principal.ox"])
        .output()
        .unwrap();
    assert!(plan.status.success());
    let fetch = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&checksum_project)
        .env("CARGO_HOME", &checksum_home)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["descargar-dependencias", "principal.ox", "--confirmar"])
        .output()
        .unwrap();
    assert!(
        fetch.status.success(),
        "{}",
        String::from_utf8_lossy(&fetch.stderr)
    );
    let lockfile = checksum_project.join("Cargo.lock");
    let lock = fs::read_to_string(&lockfile).unwrap();
    let changed = lock.replace(
        "8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert_ne!(lock, changed);
    fs::write(&lockfile, changed).unwrap();
    let compile = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&checksum_project)
        .env("CARGO_HOME", &checksum_home)
        .args(["compilar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(!compile.status.success());
    assert!(String::from_utf8_lossy(&compile.stderr).contains("checksum"));
}

fn create_registry_project(
    project: &std::path::Path,
    cargo_home: &std::path::Path,
    vendor: &std::path::Path,
) {
    fs::create_dir_all(project).unwrap();
    fs::create_dir_all(cargo_home).unwrap();
    let vendor_text = vendor.to_str().unwrap();
    fs::write(
        cargo_home.join("config.toml"),
        format!("[source.crates-io]\nreplace-with = \"fixture\"\n\n[source.fixture]\ndirectory = {vendor_text:?}\n"),
    )
    .unwrap();
    fs::write(
        project.join("oxido.toml"),
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n\n[dependencies.itoa]\nversion = \"1.0\"\n\n[dependencies.futures-executor]\nversion = \"0.3.32\"\n\n[dependencies.serde]\nversion = \"1\"\nfeatures = [\"derive\"]\n\n[dependencies.serde_json]\nversion = \"1\"\n\n[dependencies.tokio]\nversion = \"1\"\nfeatures = [\"macros\", \"rt-multi-thread\"]\n",
    )
    .unwrap();
    fs::write(
        project.join("principal.ox"),
        "sintaxis llaves\nasync fn resultado() -> i32 { 42 }\nfn main() { let mut buffer = itoa::Buffer::new(); let resultado = futures_executor::block_on(resultado()); println!(\"{}:{}\", buffer.format(42), resultado); }\n",
    )
    .unwrap();
}

fn copy_directory(source: &std::path::Path, destination: &std::path::Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        if metadata.is_dir() {
            copy_directory(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).unwrap();
        }
    }
}

fn remove_source_maps(directory: &std::path::Path) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        if metadata.is_dir() {
            remove_source_maps(&path);
        } else if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(".oxido-map.json"))
        {
            fs::remove_file(path).unwrap();
        }
    }
}

#[test]
fn audits_unsafe_tokens_and_can_reject_them_without_compiling() {
    let temp = TestDirectory::new();
    let rust = temp.0.join("muestra.rs");
    fs::write(
        &rust,
        "fn main() {\n    // unsafe en comentario\n    let texto = \"unsafe en literal\";\n    unsafe { core::hint::black_box(()); }\n}\n",
    )
    .unwrap();

    let report = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .args(["auditar-unsafe", rust.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(stdout.contains("muestra.rs:4:"), "{stdout}");
    assert!(!stdout.contains("muestra.rs:2:"), "{stdout}");
    assert!(!stdout.contains("muestra.rs:3:"), "{stdout}");

    let rejected = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .args(["auditar-unsafe", rust.to_str().unwrap(), "--rechazar"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("--rechazar"));
}

#[test]
fn audits_the_oxido_keyword_alias_without_running_cargo() {
    let temp = TestDirectory::new();
    let ox = temp.0.join("muestra.ox");
    fs::write(
        &ox,
        "sintaxis llaves\nfuncion principal() { inseguro { core::hint::black_box(()); } }\n",
    )
    .unwrap();

    let report = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .args(["auditar-unsafe", ox.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    assert!(String::from_utf8_lossy(&report.stdout).contains("muestra.ox:2:"));
}

#[test]
fn reports_project_risk_surface_without_leaking_absolute_paths() {
    let temp = TestDirectory::new();
    let root = temp.0.join("project");
    fs::create_dir_all(root.join("macro_lib/src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='audit_fixture'\nversion='0.1.0'\nedition='2024'\n\n[dependencies.macro_lib]\npath='macro_lib'\n",
    )
    .unwrap();
    fs::write(root.join("build.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("macro_lib/Cargo.toml"),
        "[package]\nname='macro_lib'\nversion='0.1.0'\nedition='2024'\n\n[lib]\nproc-macro=true\n",
    )
    .unwrap();
    fs::write(
        root.join("src.rs"),
        "fn revisar() { unsafe { core::hint::black_box(()); } let _ = std::process::Command::new(\"echo\"); extern \"C\" { fn sistema(); } }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .args(["auditar-proyecto", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Tokens unsafe/inseguro: 1"), "{stdout}");
    assert!(stdout.contains("build.rs: 1"), "{stdout}");
    assert!(stdout.contains("macro_lib/Cargo.toml"), "{stdout}");
    assert!(stdout.contains("Bloques FFI detectados: 1"), "{stdout}");
    assert!(
        stdout.contains("Llamadas Command::new detectadas: 1"),
        "{stdout}"
    );
    assert!(!stdout.contains(root.to_str().unwrap()), "{stdout}");
}
