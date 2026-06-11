mod download;
mod error;
mod ext;
mod handler;
mod yt_dlp;

use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use teloxide::{
    Bot,
    dispatching::{Dispatcher, UpdateFilterExt as _},
    types::Update,
};
use tracing::warn;

use handler::message_handler;
use yt_dlp::{MediaKind, YtDlp};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let _ = dotenvy::dotenv().inspect_err(|_| warn!("`.env` is not loaded"));

    let yt_dlp = Arc::new(YtDlp::new()?);

    let allowed_domains: Arc<HashMap<String, MediaKind>> = Arc::new(
        [
            ("www.reddit.com", MediaKind::Video),
            ("vt.tiktok.com", MediaKind::Video),
            ("www.instagram.com", MediaKind::Video),
            ("music.youtube.com", MediaKind::Audio),
            ("soundcloud.com", MediaKind::Audio),
        ]
        .into_iter()
        .map(|(url, kind)| {
            (url.into(), kind)
        })
        .collect(),
    );

    let bot = Bot::from_env();

    let handler = dptree::entry().branch(Update::filter_message().endpoint(message_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![allowed_domains, yt_dlp])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
