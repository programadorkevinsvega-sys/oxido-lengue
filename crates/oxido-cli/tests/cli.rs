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
        "sintaxis llaves\n#[macro_fixture::identity]\nfn main() { build_fixture::marker(); }\n",
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
        .args(["compilar", "principal.ox", "--confiar-dependencias"])
        .output()
        .unwrap();
    assert!(
        trusted.status.success(),
        "{}",
        String::from_utf8_lossy(&trusted.stderr)
    );
    assert_eq!(fs::read_to_string(build_marker).unwrap(), "build");
    assert_eq!(fs::read_to_string(macro_marker).unwrap(), "proc-macro");
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
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n\n[dependencies.itoa]\nversion = \"1.0\"\n\n[dependencies.futures-executor]\nversion = \"0.3.32\"\n",
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
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n\n[dependencies.itoa]\nversion = \"1.0\"\n\n[dependencies.futures-executor]\nversion = \"0.3.32\"\n",
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
