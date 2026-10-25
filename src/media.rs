use std::{fmt, path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize};
use teloxide::types::{
    FileId, InputFile, InputMedia, InputMediaAudio, InputMediaVideo, Message,
};
use tempfile::TempDir;

// NOTE(pencelheimer): also update the impl FromStr when adding new variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaKind {
    Video,
    Audio,
    // TODO(pencelheimer): introduce gallery-dl for images downloading
    // Image,
}

/// A downloaded file. It lives in its own temporary directory, which is removed when the media is
/// dropped, so the handlers don't have to clean up on every exit path.
#[derive(Debug)]
pub struct Media {
    pub kind: MediaKind,
    pub path: PathBuf,
    _dir: TempDir,
}

impl Media {
    pub fn new(kind: MediaKind, path: PathBuf, dir: TempDir) -> Self {
        Self { kind, path, _dir: dir }
    }
}

impl MediaKind {
    /// Builds an teloxide `InputMedia` of this kind from the file.
    pub fn to_input_media(self, file: InputFile) -> InputMedia {
        match self {
            MediaKind::Video => InputMedia::Video(InputMediaVideo::new(file)),
            MediaKind::Audio => InputMedia::Audio(InputMediaAudio::new(file)),
        }
    }

    /// Extracts the uploaded file's id from a teloxide message this kind of media was sent in.
    pub fn file_id_from(self, msg: &Message) -> Option<FileId> {
        match self {
            MediaKind::Video => msg.video().map(|v| v.file.id.clone()),
            MediaKind::Audio => msg.audio().map(|a| a.file.id.clone()),
        }
    }
}

impl fmt::Display for MediaKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MediaKind::Video => "video",
            MediaKind::Audio => "audio",
        })
    }
}

impl FromStr for MediaKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "video" => Ok(MediaKind::Video),
            "audio" => Ok(MediaKind::Audio),
            other => Err(format!("unknown media kind `{other}`, expected `video` or `audio`")),
        }
    }
}
