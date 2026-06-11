use anyhow::Result;
use tracing::warn;

#[tokio::main]
async fn main() -> Result<()> {
    // TODO(pencelheimer): enable tracing for the CLI or journald

    let _ = dotenvy::dotenv().inspect_err(|_| warn!("`.env` is not loaded"));

    Ok(())
}
