use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let status = Command::new("python3")
        .arg("tools/aurion_guard.py")
        .args(args)
        .status();

    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!("failed to launch Aurion guard: {error}");
            ExitCode::FAILURE
        }
    }
}
