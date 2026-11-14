use std::process::Command;

#[test]
fn version_flags_print_the_package_version_and_reject_extra_arguments() {
    let expected = format!("oxido {}\n", env!("CARGO_PKG_VERSION"));
    for flag in ["--version", "-V"] {
        let output = Command::new(env!("CARGO_BIN_EXE_oxido"))
            .arg(flag)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(output.stderr.is_empty());
    }

    let extra_argument = Command::new(env!("CARGO_BIN_EXE_oxido"))
        .args(["--version", "nuevo"])
        .output()
        .unwrap();
    assert!(!extra_argument.status.success());
    assert!(String::from_utf8_lossy(&extra_argument.stderr).contains("uso: oxido"));
}
