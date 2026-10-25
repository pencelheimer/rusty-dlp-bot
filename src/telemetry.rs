use tracing_subscriber::EnvFilter;

pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("RUSTY_DLP_LOG")
                .unwrap_or_else(|_| EnvFilter::new("warn,rusty_dlp_bot=info")),
        )
        .init();
}
