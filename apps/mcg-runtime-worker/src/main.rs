#[tokio::main]
async fn main() {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() == 1 && arguments[0] == "--qualify-runtime-worker" {
        let executable = match std::env::current_exe()
            .map_err(mcg_platform::PlatformError::from)
            .and_then(mcg_platform::runtime_worker_executable_hash)
        {
            Ok(hash) => hash,
            Err(error) => {
                eprintln!("runtime worker qualification failed: {error}");
                std::process::exit(1);
            }
        };
        println!(
            "{}",
            serde_json::json!({
                "executable_hash": executable,
                "protocol_version": mcg_contracts::PROTOCOL_V1,
                "role": "cognition",
                "storage_linked": false,
            })
        );
        return;
    }
    if !arguments.is_empty() {
        eprintln!("runtime worker accepts no arguments outside package qualification");
        std::process::exit(2);
    }
    if let Err(error) = mcg_runtime_worker::run_from_stdin().await {
        eprintln!("runtime worker failed: {error}");
        std::process::exit(1);
    }
}
