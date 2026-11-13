use std::{fs, process::Command};

struct TestDirectory(std::path::PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("el reloj debe avanzar")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("oxido-build-workspace-{nonce}"));
        fs::create_dir(&path).expect("el directorio de prueba debe crearse");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("no se pudo limpiar directorio de prueba: {error}");
        }
    }
}

#[test]
fn build_imported_workspace_leaves_release_binary_under_project_target() {
    let temp = TestDirectory::new();
    let rust = temp.0.join("rust");
    let oxido = temp.0.join("oxido");
    let app = rust.join("app");
    fs::create_dir_all(app.join("src")).expect("la carpeta Rust debe crearse");
    fs::write(
        rust.join("Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\"app\"]\n",
    )
    .expect("Cargo.toml debe escribirse");
    fs::write(
        app.join("Cargo.toml"),
        "[package]\nname = \"fixture_backend\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("Cargo.toml del miembro debe escribirse");
    fs::write(
        app.join("src/main.rs"),
        "fn main() { println!(\"workspace-release-ok\"); }\n",
    )
    .expect("main.rs debe escribirse");
    let lock = Command::new("cargo")
        .current_dir(&rust)
        .args(["generate-lockfile", "--offline"])
        .output()
        .expect("Cargo debe poder generar el lockfile local");
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );

    let imported = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&temp.0)
        .args(["importar-proyecto", "rust", "--salida", "oxido"])
        .output()
        .expect("la importación debe iniciarse");
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );

    let built = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .current_dir(&temp.0)
        .args([
            "build",
            "oxido/app/src/main.ox",
            "--release",
            "--confiar-dependencias",
        ])
        .output()
        .expect("build del workspace debe iniciarse");
    assert!(
        built.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    let executable = oxido
        .join("target/oxido/release")
        .join(format!("fixture_backend{}", std::env::consts::EXE_SUFFIX));
    assert!(executable.is_file(), "falta {}", executable.display());
    let output = Command::new(executable)
        .output()
        .expect("el binario release importado debe arrancar");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "workspace-release-ok"
    );
}
