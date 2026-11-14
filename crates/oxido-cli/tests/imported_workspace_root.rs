use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("el reloj debe avanzar")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("oxido-workspace-root-{nonce}"));
        fs::create_dir(&path).expect("el directorio temporal debe crearse");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("no se pudo limpiar el directorio de prueba: {error}");
        }
    }
}

fn cli(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(directory)
        .args(args)
        .output()
        .expect("la CLI debe iniciarse")
}

fn create_workspace(source: &Path, destination: &Path, multi_bin: bool) {
    fs::create_dir_all(source.join("apps/app/src")).expect("el miembro app debe crearse");
    fs::create_dir_all(source.join("apps/app/tests")).expect("las pruebas deben crearse");
    fs::create_dir_all(source.join("apps/app/tools")).expect("el binario debe crearse");
    fs::create_dir_all(source.join("apps/single/src")).expect("el miembro single debe crearse");
    fs::write(
        source.join("Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\"apps/app\", \"apps/single\"]\n",
    )
    .expect("el manifiesto del workspace debe escribirse");
    fs::write(
        source.join("apps/app/Cargo.toml"),
        if multi_bin {
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[[bin]]\nname = \"worker\"\npath = \"tools/worker.rs\"\n"
        } else {
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"
        },
    )
    .expect("el manifiesto app debe escribirse");
    fs::write(
        source.join("apps/single/Cargo.toml"),
        "[package]\nname = \"single\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("el manifiesto single debe escribirse");
    fs::write(
        source.join("apps/app/src/main.rs"),
        "fn main() { println!(\"app-main-ok\"); }\n",
    )
    .expect("el binario app debe escribirse");
    fs::write(
        source.join("apps/app/tools/worker.rs"),
        "fn main() { println!(\"worker-ok\"); }\n",
    )
    .expect("el binario worker debe escribirse");
    fs::write(
        source.join("apps/app/tests/workspace.rs"),
        "#[test]\nfn workspace_test_runs() { assert_eq!(2 + 2, 4); }\n",
    )
    .expect("la prueba de integración debe escribirse");
    fs::write(
        source.join("apps/single/src/main.rs"),
        "fn main() { println!(\"single-ok\"); }\n",
    )
    .expect("el binario single debe escribirse");
    let lock = Command::new("cargo")
        .current_dir(source)
        .args(["generate-lockfile", "--offline"])
        .output()
        .expect("Cargo debe generar el lockfile offline");
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let imported = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(destination.parent().expect("destino con padre"))
        .args([
            "importar-proyecto",
            source.to_str().expect("ruta UTF-8"),
            "--salida",
            destination.file_name().unwrap().to_str().unwrap(),
        ])
        .output()
        .expect("la importación debe iniciarse");
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
}

#[test]
fn workspace_root_commands_and_binary_selection_match_cargo() {
    let temp = TestDirectory::new();
    let source = temp.0.join("rust");
    let imported = temp.0.join("oxido");
    create_workspace(&source, &imported, true);

    assert!(!imported.join("Cargo.toml").exists());
    assert!(!imported.join("apps/app/Cargo.toml").exists());
    assert!(imported.join("oxido.toml").is_file());

    let help = cli(&imported, &["ejecutar", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("-p <paquete>"));
    assert!(String::from_utf8_lossy(&help.stdout).contains("--bin <nombre>"));

    let checked = cli(
        &imported,
        &["verificar", "--confiar-dependencias", "--json"],
    );
    assert!(
        checked.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&checked.stdout).expect("workspace JSON válido");
    assert_eq!(report["schemaVersion"], 1);
    assert_eq!(report["success"], true);
    assert!(report["diagnostics"].is_array());
    assert!(report["output"].is_array());

    let built = cli(
        &imported,
        &["compilar", "--release", "--confiar-dependencias"],
    );
    assert!(
        built.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    for binary in ["app", "worker", "single"] {
        assert!(
            imported
                .join("target/oxido/release")
                .join(format!("{binary}{}", std::env::consts::EXE_SUFFIX))
                .is_file(),
            "falta el binario de workspace {binary}"
        );
    }

    let tested = cli(&imported, &["probar", "--confiar-dependencias"]);
    assert!(
        tested.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&tested.stdout),
        String::from_utf8_lossy(&tested.stderr)
    );

    let ambiguous = cli(&imported, &["ejecutar", "--confiar-dependencias"]);
    assert!(!ambiguous.status.success());
    let ambiguity = String::from_utf8_lossy(&ambiguous.stderr);
    assert!(ambiguity.contains("varios binarios ejecutables"));
    assert!(ambiguity.contains("worker"));
    assert!(ambiguity.contains("single"));
    assert!(ambiguity.contains("-p <paquete> --bin <nombre>"));

    let selected_package_and_bin = cli(
        &imported,
        &[
            "ejecutar",
            "-p",
            "app",
            "--bin",
            "worker",
            "--confiar-dependencias",
        ],
    );
    assert!(
        selected_package_and_bin.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&selected_package_and_bin.stdout),
        String::from_utf8_lossy(&selected_package_and_bin.stderr)
    );
    assert!(String::from_utf8_lossy(&selected_package_and_bin.stdout).contains("worker-ok"));

    let selected_bin = cli(
        &imported,
        &["ejecutar", "--bin", "single", "--confiar-dependencias"],
    );
    assert!(
        selected_bin.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&selected_bin.stdout),
        String::from_utf8_lossy(&selected_bin.stderr)
    );
    assert!(String::from_utf8_lossy(&selected_bin.stdout).contains("single-ok"));

    let package_with_one_bin = cli(
        &imported,
        &["ejecutar", "-p", "single", "--confiar-dependencias"],
    );
    assert!(package_with_one_bin.status.success());
    assert!(String::from_utf8_lossy(&package_with_one_bin.stdout).contains("single-ok"));

    for (args, message) in [
        (vec!["ejecutar", "-p"], "falta el valor de -p <paquete>"),
        (
            vec!["ejecutar", "--bin"],
            "falta el valor de --bin <nombre>",
        ),
        (
            vec!["ejecutar", "-p", "missing", "--confiar-dependencias"],
            "no existe el paquete `missing`",
        ),
        (
            vec!["ejecutar", "--bin", "missing", "--confiar-dependencias"],
            "no existe el binario `missing`",
        ),
        (
            vec!["verificar", "-p", "app"],
            "solo se admiten con `oxido ejecutar`",
        ),
        (
            vec!["ejecutar", "-p", "app", "-p", "single"],
            "uso: oxido ejecutar",
        ),
        (
            vec!["ejecutar", "--bin", "app", "--bin", "single"],
            "uso: oxido ejecutar",
        ),
    ] {
        let output = cli(&imported, &args);
        assert!(!output.status.success(), "args={args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "args={args:?}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let explicit_source_with_selector = cli(
        &imported,
        &[
            "ejecutar",
            "apps/app/src/main.ox",
            "--bin",
            "single",
            "--confiar-dependencias",
        ],
    );
    assert!(!explicit_source_with_selector.status.success());
    assert!(
        String::from_utf8_lossy(&explicit_source_with_selector.stderr)
            .contains("no se combinan con un archivo `.ox` explícito")
    );

    fs::write(
        imported.join("apps/app/src/main.ox"),
        "sintaxis llaves\nfn main() { let value: u32 = \"error\"; println!(\"{value}\"); }\n",
    )
    .expect("la fuente de diagnóstico debe escribirse");
    let failed = cli(
        &imported,
        &["verificar", "--confiar-dependencias", "--json"],
    );
    assert!(!failed.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&failed.stdout).expect("el error JSON debe conservar esquema");
    assert_eq!(report["schemaVersion"], 1);
    let diagnostic = report["diagnostics"]
        .as_array()
        .expect("diagnósticos en forma de lista")
        .iter()
        .find(|diagnostic| diagnostic["file"] == "apps/app/src/main.ox")
        .expect("la ruta de diagnóstico debe ser relativa al workspace");
    assert_eq!(diagnostic["severity"], "error");
    assert_eq!(diagnostic["line"], 2);
}

#[test]
fn imported_workspace_root_with_one_binary_runs_without_selection() {
    let temp = TestDirectory::new();
    let source = temp.0.join("rust");
    let imported = temp.0.join("oxido");
    fs::create_dir_all(source.join("app/src")).expect("el paquete debe crearse");
    fs::write(
        source.join("Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\"app\"]\n",
    )
    .expect("el workspace debe escribirse");
    fs::write(
        source.join("app/Cargo.toml"),
        "[package]\nname = \"only_binary\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("el manifiesto debe escribirse");
    fs::write(
        source.join("app/src/main.rs"),
        "fn main() { println!(\"automatic-selection-ok\"); }\n",
    )
    .expect("el binario debe escribirse");
    let lock = Command::new("cargo")
        .current_dir(&source)
        .args(["generate-lockfile", "--offline"])
        .output()
        .expect("Cargo debe generar lockfile");
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let imported_result = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&temp.0)
        .args(["importar-proyecto", "rust", "--salida", "oxido"])
        .output()
        .expect("la importación debe iniciarse");
    assert!(
        imported_result.status.success(),
        "{}",
        String::from_utf8_lossy(&imported_result.stderr)
    );

    let run = cli(&imported, &["ejecutar", "--confiar-dependencias"]);
    assert!(
        run.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("automatic-selection-ok"));
}
