use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 || arguments[0] != "--apply" {
        eprintln!("usage: mcg-updater --apply ABSOLUTE_PRIVATE_REQUEST.json");
        return ExitCode::from(2);
    }
    let request = PathBuf::from(&arguments[1]);
    match mcg_updater::run_staged_windows_update(&request) {
        Ok(outcome) if outcome.result == "succeeded" => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(error) => {
            if let Err(recovery) = mcg_updater::recover_packaged_desktop_after_supervisor_error() {
                eprintln!("Windows update supervision failed and recovery also failed: {recovery}");
            }
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
