//! Which Bot API server the bot talks to: the cloud one (`api.telegram.org`) by default, or a
//! self-hosted [telegram-bot-api](https://github.com/tdlib/telegram-bot-api) server.

use std::{env, path::Path, sync::LazyLock};

use anyhow::{Context, Result, anyhow};
use teloxide::{Bot, types::InputFile};
use url::Url;

/// Upload limit of the cloud Bot API.
pub const CLOUD_MAX_FILE_SIZE_MB: u64 = 50;

/// Upload limit of a Bot API server in `--local` mode.
pub const LOCAL_MAX_FILE_SIZE_MB: u64 = 2000;

/// `RUSTY_DLP_API_LOCAL`: the server runs in `--local` mode and shares the filesystem with the bot
/// (the download directory and the server's working directory are mounted at the same paths).
/// Files are then sent by path instead of being uploaded, and files from `getFile` are read from disk.
static LOCAL: LazyLock<bool> = LazyLock::new(|| {
    env::var("RUSTY_DLP_API_LOCAL").is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        )
    })
});

pub fn is_local() -> bool {
    *LOCAL
}

/// Points the bot at `RUSTY_DLP_API_URL` if it is set.
pub fn configure(bot: Bot) -> Result<Bot> {
    // Compose passes unset variables as empty ones.
    let Some(url) = env::var("RUSTY_DLP_API_URL").ok().filter(|url| !url.is_empty()) else {
        if is_local() {
            return Err(anyhow!(
                "RUSTY_DLP_API_LOCAL is set, but RUSTY_DLP_API_URL is not"
            ));
        }
        return Ok(bot);
    };

    let url = Url::parse(&url).context("RUSTY_DLP_API_URL is not a valid URL")?;
    Ok(bot.set_api_url(url))
}

/// A file to send: by its path for a local server, uploaded otherwise.
pub fn input_file(path: &Path) -> InputFile {
    if is_local() {
        // The server resolves the path on its side, so it has to be absolute.
        if let Ok(url) = path
            .canonicalize()
            .map_err(drop)
            .and_then(Url::from_file_path)
        {
            return InputFile::url(url);
        }
    }

    InputFile::file(path)
}

/// Downloads a file by the `file_path` from `getFile`.
pub async fn download_file(bot: &Bot, file_path: &str) -> Result<Vec<u8>> {
    // NOTE(pencelheimer): in `--local` mode the server doesn't serve files over HTTP, `file_path`
    // is an absolute path in its working directory.
    if is_local() && Path::new(file_path).is_absolute() {
        return tokio::fs::read(file_path).await.with_context(|| {
            format!("Can't read {file_path}, is the Bot API server's directory mounted?")
        });
    }

    let mut data = Vec::new();
    teloxide::net::Download::download_file(bot, file_path, &mut data).await?;
    Ok(data)
}
