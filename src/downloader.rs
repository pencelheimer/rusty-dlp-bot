use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, anyhow};
use tokio::{process::Command, sync::Semaphore, task::JoinSet};
use tracing::instrument;

use crate::{
    config::Config,
    extractors::links::Link,
    media::{Media, MediaKind},
};

const MP3_QUALITY: &str = "192K";

/// How many files of one bulk download are fetched at the same time.
const BULK_CONCURRENCY: usize = 3;

const UPDATE_COMMAND_VAR: &str = "RUSTY_DLP_UPDATE_COMMAND";

const OUTPUT_DIR: &str = "output";

fn fmt_size(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

/// The `ERROR:` lines yt-dlp printed, or all of stderr if there are none.
fn yt_dlp_error(stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let errors: Vec<_> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("ERROR: "))
        .collect();

    if errors.is_empty() {
        stderr.trim().to_owned()
    } else {
        errors.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct Downloader {
    config: Arc<Config>,

    yt_dlp: PathBuf,
    /// `None` lets yt-dlp find ffmpeg in `PATH`, `--ffmpeg-location` only takes a path.
    ffmpeg: Option<PathBuf>,
}

impl Downloader {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            yt_dlp: env::var_os("RUSTY_DLP_YT_DLP").map_or_else(|| PathBuf::from("yt-dlp"), PathBuf::from),
            ffmpeg: env::var_os("RUSTY_DLP_FFMPEG").map(PathBuf::from),
        }
    }

    /// Runs the shell command from `RUSTY_DLP_UPDATE_COMMAND`. How yt-dlp was installed decides how
    /// it has to be updated, so there is no default. ffmpeg is not covered.
    pub async fn update(&self) -> Result<()> {
        let command = env::var(UPDATE_COMMAND_VAR)
            .map_err(|_| anyhow!("Updating is not configured, set {UPDATE_COMMAND_VAR}"))?;

        let output = Command::new("sh")
            .args(["-c", &command])
            .output()
            .await
            .context("Can't run the update command")?;

        if !output.status.success() {
            return Err(anyhow!(
                "the update command exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(())
    }

    #[instrument(skip_all, fields(url = link.url.as_str()))]
    pub async fn download(&self, link: &Link) -> Result<Media> {
        // TODO(pencelheimer): maybe should check the first segment as well? e.g. for handling "youtube.com/playlist" differently

        // NOTE(pencelheimer): every download gets its own directory, so the same video requested
        // twice at once doesn't clash. It is removed on drop, so on any error below as well
        fs::create_dir_all(OUTPUT_DIR).context("Can't create the output directory")?;
        let dir = tempfile::tempdir_in(OUTPUT_DIR).context("Can't create a download directory")?;

        let path = self.run_yt_dlp(link, dir.path()).await?;

        let media = Media::new(link.kind, path, dir);
        self.ensure_fits(&media)?;

        Ok(media)
    }

    /// Downloads the files concurrently (at most `BULK_CONCURRENCY` at a time). The results come in
    /// the order of completion, the errors say which url failed. A failed download doesn't affect
    /// the others.
    pub async fn download_many(&self, links: impl IntoIterator<Item = Link>) -> Vec<Result<Media>> {
        let permits = Arc::new(Semaphore::new(BULK_CONCURRENCY));

        let mut tasks = JoinSet::new();
        for link in links {
            let downloader = self.clone();
            let permits = permits.clone();
            tasks.spawn(async move {
                let _permit = permits
                    .acquire_owned()
                    .await
                    .expect("the semaphore is never closed");
                downloader
                    .download(&link)
                    .await
                    .with_context(|| link.url.to_string())
            });
        }

        tasks.join_all().await
    }

    /// Downloads the best quality into `dir` and returns the path of the final file.
    async fn run_yt_dlp(&self, link: &Link, dir: &Path) -> Result<PathBuf> {
        let limit = self.config.max_file_size();

        let mut command = Command::new(&self.yt_dlp);
        command
            // NOTE(pencelheimer): the download is aborted together with the handler, e.g. on a timeout
            .kill_on_drop(true)
            .args(["--no-playlist", "--no-simulate", "--print", "after_move:filepath"])
            .args(["--max-filesize", &limit.to_string()])
            .arg("--paths")
            .arg(dir)
            .args(["--output", "%(title).150B.%(ext)s"])
            .arg("--embed-metadata");

        if let Some(ffmpeg) = &self.ffmpeg {
            command.arg("--ffmpeg-location").arg(ffmpeg);
        }
        if self.config.cookies_file.is_file() {
            command.arg("--cookies").arg(&self.config.cookies_file);
        }

        match link.kind {
            // NOTE(pencelheimer): H.264 is preferred at the same resolution, Telegram plays it everywhere
            MediaKind::Video => command.args([
                "--format", "bv*+ba/b",
                "--format-sort", "res,vcodec:h264",
                "--merge-output-format", "mp4",
            ]),
            MediaKind::Audio => command.args([
                "--format", "ba/b",
                "--extract-audio",
                "--audio-format", "mp3",
                "--audio-quality", MP3_QUALITY,
                // NOTE(pencelheimer): the cover, as jpg, which MP3 supports
                "--embed-thumbnail",
                "--convert-thumbnails", "jpg",
            ]),
        };

        let output = command
            .arg("--")
            .arg(link.url.as_str())
            .output()
            .await
            .context("Can't run yt-dlp")?;

        if !output.status.success() {
            return Err(anyhow!("{}", yt_dlp_error(&output.stderr)));
        }

        // NOTE(pencelheimer): yt-dlp skips a file over `--max-filesize` without failing, nothing is
        // printed then
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .last()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("The file is too large to send, the limit is {}", fmt_size(limit)))
    }

    fn ensure_fits(&self, media: &Media) -> Result<()> {
        let limit = self.config.max_file_size();
        let size = fs::metadata(&media.path)
            .context("Can't read the size of the downloaded file")?
            .len();

        if size > limit {
            return Err(anyhow!(
                "The file is too large to send: it is {}, the limit is {}",
                fmt_size(size),
                fmt_size(limit)
            ));
        }

        Ok(())
    }
}
