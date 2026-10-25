use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use tokio::{process::Command, sync::Semaphore, task::JoinSet};
use tracing::instrument;
use yt_dlp::{
    cache::CacheConfig,
    client::{Downloader as YtDlpDownloader, Libraries},
    download::{AudioCodec, PostProcessConfig},
    model::Video,
};

use crate::{
    config::Config,
    extractors::links::Link,
    media::{Media, MediaKind},
};

const MP3_BITRATE: &str = "192k";

/// How many files of one bulk download are fetched at the same time.
const BULK_CONCURRENCY: usize = 3;

const CACHE_TTL: Duration = Duration::from_secs(60 * 60);
const UPDATE_COMMAND_VAR: &str = "RUSTY_DLP_UPDATE_COMMAND";

const OUTPUT_DIR: &str = "output";
const CACHE_DIR: &str = "cache";

fn fmt_size(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

#[derive(Debug, Clone)]
pub struct Downloader {
    config: Arc<Config>,

    yt_dlp: YtDlpDownloader,
}

impl Downloader {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let libs = Libraries::new(
            env::var_os("RUSTY_DLP_YT_DLP").map_or_else(|| PathBuf::from("yt-dlp"), PathBuf::from),
            env::var_os("RUSTY_DLP_FFMPEG").map_or_else(|| PathBuf::from("ffmpeg"), PathBuf::from),
        );

        let ttl = Some(CACHE_TTL.as_secs());
        let cache_config = CacheConfig::builder()
            .cache_dir(PathBuf::from(CACHE_DIR))
            .video_ttl(ttl)
            .playlist_ttl(ttl)
            .download_ttl(ttl)
            .build();

        // NOTE(pencelheimer): The file must exist from the start for `/setcookies` to take effect
        // without a restart. An empty jar is harmless.
        if !config.cookies_file.is_file() {
            fs::write(&config.cookies_file, "# Netscape HTTP Cookie File\n")
                .with_context(|| format!("Can't create {}", config.cookies_file.display()))?;
        }

        let yt_dlp = YtDlpDownloader::builder(libs, OUTPUT_DIR)
            .with_cache_config(cache_config)
            .with_cookies(&config.cookies_file)
            .build()
            .await
            .context("Can't init the yt-dlp downloader")?;

        Ok(Self { yt_dlp, config })
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

        let video = self
            .yt_dlp
            .fetch_video_infos(&link.url)
            .await
            .context("Can't fetch the video infos from the given URL")?;

        let path = match link.kind {
            MediaKind::Video => self.download_video(&video).await?,
            MediaKind::Audio => self.download_audio(&video).await?,
        };

        let media = Media { kind: link.kind, path };
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

    async fn download_video(&self, video: &Video) -> Result<PathBuf> {
        self.yt_dlp
            .download_video(video, format!("{}.mp4", video.title))
            .await
            .context("Can't save video")
    }

    async fn download_audio(&self, audio: &Video) -> Result<PathBuf> {
        // NOTE(pencelheimer): YouTube has no MP3 streams, so the best audio is downloaded as is and converted
        let source = self
            .yt_dlp
            .download_audio_stream(audio, format!("{}.source", audio.title))
            .await
            .context("Can't save audio")?;

        // NOTE(pencelheimer): a leftover file would make ffmpeg fail
        let target = format!("{}.mp3", audio.title);
        let _ = fs::remove_file(Path::new(OUTPUT_DIR).join(&target));

        let config = PostProcessConfig::new()
            .with_audio_codec(AudioCodec::MP3)
            .with_audio_bitrate(MP3_BITRATE);

        let converted = self
            .yt_dlp
            .postprocess_video(&source, target, config)
            .await
            .context("Can't convert audio to MP3");

        let _ = fs::remove_file(&source);
        converted
    }
}
