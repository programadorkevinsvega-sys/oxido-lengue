use std::{
    env, fs,
    io::{self, Write},
    process,
    thread,
    time::Duration,
};

fn main() -> io::Result<()> {
    if let Some(pid_file) = env::var_os("OXIDO_FAKE_CARGO_PID_FILE") {
        fs::write(pid_file, process::id().to_string())?;
    }

    println!(r#"{{"reason":"compiler-artifact","target":{{"name":"fixture_crate"}}}}"#);
    io::stdout().flush()?;

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

    println!(
        r#"{{"reason":"build-finished","success":{}}}"#,
        exit_code == 0
    );
    io::stdout().flush()?;
    process::exit(exit_code);
}
