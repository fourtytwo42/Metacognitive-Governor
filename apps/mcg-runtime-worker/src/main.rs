#[tokio::main]
async fn main() {
    if let Err(error) = mcg_runtime_worker::run_from_stdin().await {
        eprintln!("runtime worker failed: {error}");
        std::process::exit(1);
    }
}
