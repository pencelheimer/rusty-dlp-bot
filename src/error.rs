#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("failed to download video: {0}")]
    Download(#[source] anyhow::Error),

    #[error("failed to send video: {0}")]
    Send(#[source] anyhow::Error),
}
