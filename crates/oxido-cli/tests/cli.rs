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
    assert!(imported_ox.contains("//! Documentación del crate del ejemplo."));
    assert!(imported_ox.contains("doc = \"Punto de entrada del programa de ejemplo.\""));
    assert!(!imported_ox.contains("comentario ordinario"));
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
