//! Unpackaged real-process fixture for active Runtime cancellation and late-result tests.

#[tokio::main]
async fn main() {
    if let Err(error) =
        mcg_runtime_worker::run_delayed_fixture_from_stdin(std::time::Duration::from_secs(30)).await
    {
        eprintln!("delayed Runtime fixture failed: {error}");
        std::process::exit(1);
    }
}
