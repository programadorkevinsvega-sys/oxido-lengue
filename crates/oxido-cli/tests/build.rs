use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct TestDirectory(std::path::PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("el reloj debe avanzar")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("oxido-build-{label}-{nonce}"));
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

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_oxido"))
}

#[test]
fn check_validates_without_binary_and_build_release_keeps_the_executable() {
    let temp = TestDirectory::new("single");
    let source = temp.0.join("main.ox");
    fs::write(
        &source,
        "sintaxis llaves\nfuncion principal() { println!(\"release-artifact-ok\"); }\n",
    )
    .expect("la fuente .ox debe escribirse");

    let checked = cli()
        .current_dir(&temp.0)
        .args(["check", "main.ox"])
        .output()
        .expect("check debe iniciarse");
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let release_dir = temp.0.join("target/oxido/release");
    assert!(
        !release_dir
            .join(format!("oxido_generated{}", std::env::consts::EXE_SUFFIX))
            .exists()
    );

    let compiled = cli()
        .current_dir(&temp.0)
        .args(["compilar", "main.ox"])
        .output()
        .expect("compilar debe conservarse como alias de build");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert!(
        temp.0
            .join("target/oxido/debug")
            .join(format!("oxido_generated{}", std::env::consts::EXE_SUFFIX))
            .is_file()
    );

    let built = cli()
        .current_dir(&temp.0)
        .args(["build", "main.ox", "--release", "--json"])
        .output()
        .expect("build debe iniciarse");
    assert!(
        built.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&built.stdout).expect("build --json debe emitir JSON");
    assert_eq!(report["artifactDirectory"], "target/oxido/release");

    let executable = release_dir.join(format!("oxido_generated{}", std::env::consts::EXE_SUFFIX));
    assert!(executable.is_file(), "falta {}", executable.display());
    let output = Command::new(executable)
        .output()
        .expect("el ejecutable release debe arrancar");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "release-artifact-ok"
    );
}

#[cfg(unix)]
#[test]
fn build_rejects_a_symlinked_target_directory() {
    use std::os::unix::fs::symlink;

    let temp = TestDirectory::new("target-symlink");
    fs::write(
        temp.0.join("main.ox"),
        "sintaxis llaves\nfuncion principal() {}\n",
    )
    .expect("la fuente debe escribirse");
    let outside = temp.0.join("outside");
    fs::create_dir(&outside).expect("la carpeta externa debe crearse");
    symlink(&outside, temp.0.join("target")).expect("el enlace debe crearse");

    let built = cli()
        .current_dir(&temp.0)
        .args(["build", "main.ox"])
        .output()
        .expect("build debe iniciarse");
    assert!(!built.status.success());
    assert!(String::from_utf8_lossy(&built.stderr).contains("no es un directorio seguro"));
    assert_eq!(
        fs::read_dir(outside)
            .expect("la carpeta debe leerse")
            .count(),
        0
    );
}
