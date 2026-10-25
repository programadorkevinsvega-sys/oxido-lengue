use oxido_compiler::config::{ProjectConfig, SyntaxMode};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn parses_only_declared_language_data() {
    let config = ProjectConfig::parse("[language]\nsyntax = 'indentation'\nedition = '2024'\n")
        .expect("configuración válida");
    assert_eq!(config.language.syntax, Some(SyntaxMode::Indentation));
    assert!(ProjectConfig::parse("[language]\nedition = '2024'\n").is_ok());
}

#[test]
fn rejects_unknown_fields_and_unsupported_editions() {
    assert!(
        ProjectConfig::parse(
            "[language]\nsyntax = 'braces'\nedition = '2024'\ncommand = 'rm -rf'\n"
        )
        .is_err()
    );
    assert!(ProjectConfig::parse("[language]\nsyntax = 'braces'\nedition = '2021'\n").is_err());
}

#[test]
fn rejects_configuration_larger_than_the_resource_limit() {
    let input = " ".repeat(64 * 1024 + 1);
    let error = ProjectConfig::parse(&input).unwrap_err();
    assert!(error.contains("64 KiB"));
}

#[test]
fn rejects_local_dependency_paths_that_escape_the_project_root() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("reloj del sistema válido")
        .as_nanos();
    let base = std::env::temp_dir().join(format!("oxido-config-path-{nonce}"));
    let project = base.join("project");
    let outside = base.join("outside");
    fs::create_dir_all(&project).expect("crear raíz temporal");
    fs::create_dir_all(&outside).expect("crear directorio externo");
    fs::write(
        outside.join("Cargo.toml"),
        "[package]\nname='externo'\nversion='0.1.0'\nedition='2024'\n",
    )
    .expect("crear manifiesto de prueba");

    let config = ProjectConfig::parse(
        "[language]\nedition='2024'\n[dependencies.externo]\npath='../outside'\n",
    )
    .expect("TOML de dependencia válida antes de resolverla");
    let error = config
        .resolve_dependencies(&project)
        .expect_err("la ruta no debe salir del proyecto");
    assert!(error.contains("sale de la raíz del proyecto"), "{error}");

    fs::remove_dir_all(base).expect("limpiar fixtures de configuración");
}

#[cfg(unix)]
#[test]
fn rejects_local_dependency_symlinks_that_escape_the_project_root() {
    use std::os::unix::fs::symlink;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("reloj del sistema válido")
        .as_nanos();
    let base = std::env::temp_dir().join(format!("oxido-config-symlink-{nonce}"));
    let project = base.join("project");
    let outside = base.join("outside");
    fs::create_dir_all(&project).expect("crear raíz temporal");
    fs::create_dir_all(&outside).expect("crear crate externo");
    fs::write(
        outside.join("Cargo.toml"),
        "[package]\nname='externo'\nversion='0.1.0'\nedition='2024'\n",
    )
    .expect("crear manifiesto de prueba");
    symlink(&outside, project.join("vendor-externo")).expect("crear enlace simbólico de prueba");

    let config = ProjectConfig::parse(
        "[language]\nedition='2024'\n[dependencies.externo]\npath='vendor-externo'\n",
    )
    .expect("TOML de dependencia válida antes de resolverla");
    let error = config
        .resolve_dependencies(&project)
        .expect_err("el enlace no debe salir del proyecto");
    assert!(error.contains("sale de la raíz del proyecto"), "{error}");

    fs::remove_dir_all(base).expect("limpiar fixtures de configuración");
}

#[test]
fn rejects_executable_options_in_a_local_dependency_declaration() {
    let config = ProjectConfig::parse(
        "[language]\nedition='2024'\n[dependencies.local]\npath='vendor/local'\ncommand='sh'\n",
    );
    assert!(config.is_err());
}

#[test]
fn accepts_registry_dependencies_with_valid_versions_and_features() {
    let config = ProjectConfig::parse(
        "[language]\nedition='2024'\n[dependencies.itoa]\nversion='1.0'\ndefault-features=false\n",
    )
    .expect("dependencia de registro declarativa");
    assert_eq!(config.dependencies["itoa"].version.as_deref(), Some("1.0"));
    assert!(!config.dependencies["itoa"].default_features);
    let error = ProjectConfig::parse(
        "[language]\nedition='2024'\n[dependencies.itoa]\nversion='no-semver'\n",
    )
    .expect_err("Cargo version requirement must be valid SemVer");
    assert!(error.contains("requisito de versión"), "{error}");
}

#[test]
fn rejects_dependencies_that_mix_registry_and_path_sources() {
    let error = ProjectConfig::parse(
        "[language]\nedition='2024'\n[dependencies.itoa]\npath='vendor/itoa'\nversion='1'\n",
    )
    .expect_err("una dependencia no puede mezclar path y version");
    assert!(error.contains("exactamente uno"), "{error}");
}

#[test]
fn rejects_git_url_and_custom_registry_sources() {
    for unsupported_source in [
        "git='https://example.invalid/repo.git'",
        "url='https://example.invalid/crate'",
        "registry='empresa'",
    ] {
        let input = format!(
            "[language]\nedition='2024'\n[dependencies.crate_ejemplo]\nversion='1'\n{unsupported_source}\n"
        );
        assert!(
            ProjectConfig::parse(&input).is_err(),
            "fuente Cargo no soportada: {unsupported_source}"
        );
    }
}
