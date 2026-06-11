use std::{env, fmt, path::PathBuf};

use anyhow::{Context, Result, bail};
use tempfile::TempDir;
use tokio::process::Command;
use tracing::debug;

#[derive(Clone, Copy, Debug)]
pub enum MediaKind {
    Video,
    Audio,
}

impl AsRef<str> for MediaKind {
    fn as_ref(&self) -> &str {
        match self {
            MediaKind::Video => "video",
            MediaKind::Audio => "audio",
        }
    }
}

impl fmt::Display for MediaKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

pub struct YtDlp {
    executable: PathBuf,
    cookies: Option<PathBuf>,
    _tmp_dir: TempDir,
    output_dir: PathBuf,
}

impl YtDlp {
    pub fn new() -> Result<Self> {
        let executable = env::var("YT_DLP_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("yt-dlp"));
        let cookies = env::var("YT_DLP_COOKIES").ok().map(PathBuf::from);
        let tmp_dir = tempfile::tempdir().context("failed to create temp dir")?;
        let output_dir = tmp_dir.path().to_path_buf();
        Ok(Self { executable, cookies, _tmp_dir: tmp_dir, output_dir })
    }

    pub async fn download(&self, url: &str, kind: MediaKind) -> Result<PathBuf> {
        let work_dir = tempfile::Builder::new()
            .tempdir_in(&self.output_dir)
            .context("failed to create work dir")?
            .keep();

        let output_template = work_dir
            .join("%(title)s.%(ext)s")
            .to_string_lossy()
            .to_string();

        let mut cmd = Command::new(&self.executable);
        cmd.arg("--no-playlist")
            .arg("--embed-thumbnail")
            .args(["--output", &output_template]);

        if let Some(cookies) = &self.cookies {
            cmd.args(["--cookies", cookies.to_str().context("non-UTF-8 cookies path")?]);
        }

        cmd.arg(url);

        if let MediaKind::Audio = kind {
            cmd.arg("--extract-audio")
                .args(["--audio-format", "mp3"])
                .arg("--embed-metadata");
        }

        let output = cmd.output().await.context("failed to spawn yt-dlp")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("yt-dlp failed: {}", stderr.trim());
        }

        let path = std::fs::read_dir(&work_dir)
            .context("failed to read work dir")?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.is_file())
            .context("yt-dlp produced no output file")?;

        debug!(?path, "download complete");
        Ok(path)
    }
}
