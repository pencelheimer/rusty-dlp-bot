mod config;
mod download;
mod downloader;
mod error;
mod ext;
mod handlers;
mod telemetry;
mod yt_dlp;

use std::sync::Arc;

use anyhow::Result;
use teloxide::{
    Bot,
    dispatching::{Dispatcher, HandlerExt as _, UpdateFilterExt as _},
    requests::Requester,
    types::Update,
    utils::command::BotCommands,
};
use tracing::{info, warn};

use config::Config;
use downloader::Downloader;
use handlers::Command;
use yt_dlp::YtDlp;

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init();

    let _ = dotenvy::dotenv().inspect_err(|_| warn!("`.env` is not loaded"));

    let config = Arc::new(Config::new()?);
    let downloader = Arc::new(Downloader::new(YtDlp::new()?));

    let bot = Bot::from_env();

    bot.set_my_commands(Command::bot_commands()).await?;

    info!("starting bot");

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .branch(
                    dptree::entry()
                        .filter_command::<Command>()
                        .endpoint(handlers::on_command),
                )
                .branch(dptree::entry().endpoint(handlers::on_auto)),
        )
        .branch(Update::filter_guest_message().endpoint(handlers::on_guest_message));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![config, downloader])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    info!("shutting down");
    Ok(())
}
