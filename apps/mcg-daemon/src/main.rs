use std::{net::SocketAddr, sync::Arc};

use mcg_config::AppConfig;
use mcg_runtime::AppRuntime;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("mcg=info")),
        )
        .init();

    let config = AppConfig::default();
    let address: SocketAddr = config.bind_address.parse()?;
    let runtime = Arc::new(AppRuntime::open(config)?);

    info!(%address, "starting MCG daemon");
    mcg_api::serve(address, runtime).await?;
    info!("MCG daemon stopped cleanly");
    Ok(())
}
