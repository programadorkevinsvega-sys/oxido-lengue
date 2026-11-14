use std::{
    env, fs,
    io::{self, Write},
    process,
    thread,
    time::Duration,
};

fn emit_event(event: &str) -> io::Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{event}")?;
    stdout.flush()
}

fn main() -> io::Result<()> {
    if let Some(pid_file) = env::var_os("OXIDO_FAKE_CARGO_PID_FILE") {
        fs::write(pid_file, process::id().to_string())?;
    }

    emit_event(
        r#"{"reason":"compiler-artifact","package_id":"path+file:///private/oxido-workspaces/build-123/fixture#fixture_package@0.1.0","target":{"name":"fixture_crate","kind":["bin"]}}"#,
    )?;
    emit_event(
        r#"{"reason":"compiler-artifact","package_id":"path+file:///private/oxido-workspaces/build-123/fixture#fixture_package@0.1.0","target":{"name":"fixture_crate","kind":["bin"]}}"#,
    )?;
    emit_event(
        r#"{"reason":"compiler-artifact","package_id":"path+file:///private/oxido-workspaces/build-123/member#member_package@0.1.0","target":{"name":"member_crate","kind":["bin"]}}"#,
    )?;
    emit_event(
        r#"{"reason":"build-script-executed","out_dir":"/private/oxido-workspaces/build-123/target"}"#,
    )?;

    let delay = env::var("OXIDO_FAKE_CARGO_DELAY_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(700);
    thread::sleep(Duration::from_millis(delay));

    let exit_code = env::var("OXIDO_FAKE_CARGO_EXIT")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0);
    if let Some(finished_file) = env::var_os("OXIDO_FAKE_CARGO_FINISHED_FILE") {
        fs::write(finished_file, "finished")?;
    }

    emit_event(&format!(
        r#"{{"reason":"build-finished","success":{}}}"#,
        exit_code == 0
    ))?;
    emit_event(&format!(
        r#"{{"reason":"build-finished","success":{}}}"#,
        exit_code == 0
    ))?;
    process::exit(exit_code);
}
