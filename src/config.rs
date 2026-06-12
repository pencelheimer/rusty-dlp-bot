use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::yt_dlp::MediaKind;

#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    pub video: Vec<String>,
    #[serde(default)]
    pub audio: Vec<String>,
}

impl Config {
    pub fn new() -> Result<Self> {
        let path: PathBuf = env::var("CONFIG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("config.toml"));

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    pub fn filter_video(&self, url: impl AsRef<str>) -> bool {
        self.video.iter().any(|v| url.as_ref().contains(v))
    }

    pub fn filter_audio(&self, url: impl AsRef<str>) -> bool {
        self.audio.iter().any(|a| url.as_ref().contains(a))
    }

    pub fn kind_for(&self, url: impl AsRef<str>) -> Option<MediaKind> {
        let url = url.as_ref();
        if self.filter_video(url) {
            Some(MediaKind::Video)
        } else if self.filter_audio(url) {
            Some(MediaKind::Audio)
        } else {
            None
        }
    }
}
