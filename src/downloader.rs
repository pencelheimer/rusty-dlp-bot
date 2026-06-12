use std::path::PathBuf;

use anyhow::Result;
use tracing::{info, instrument};

use crate::yt_dlp::{MediaKind, YtDlp};

pub struct Downloader {
    yt_dlp: YtDlp,
}

impl Downloader {
    pub fn new(yt_dlp: YtDlp) -> Self {
        Self { yt_dlp }
    }

    #[instrument(skip(self), fields(url = url.get(..80).unwrap_or(url), ?kind))]
    pub async fn download(&self, url: &str, kind: MediaKind) -> Result<PathBuf> {
        info!("starting download");
        self.yt_dlp.download(url, kind).await
    }
}
