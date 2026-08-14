use clap::{Parser, Subcommand};
use mcg_runtime::RuntimeStatus;

#[derive(Debug, Parser)]
#[command(name = "mcg", version, about = "MetaCognitive Governor CLI")]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:7777")]
    host: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show daemon status.
    Status {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Status { json } => {
            let url = format!("{}/api/v1/system/status", cli.host.trim_end_matches('/'));
            let status: RuntimeStatus = reqwest::get(url).await?.error_for_status()?.json().await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("{} {} ({:?})", status.service, status.version, status.state);
            }
        }
    }

    Ok(())
}
