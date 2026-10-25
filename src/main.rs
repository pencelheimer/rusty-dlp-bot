mod config;
mod downloader;
mod media;
mod telemetry;
mod extractors {
    pub mod guest_message;
    pub mod links;
    pub mod sender;
}
mod handlers {
    pub mod commands;
    pub mod guest;
    pub mod normal;
}
mod utils {
    pub mod bot_ext;
    pub mod entities_ext;

#[macro_export]
    macro_rules! ml {
    (
        $(#[doc = $doc:expr])*
    ) => {
        concat!($($doc, "\n"),*)
    };
}
}

use std::{env, sync::Arc};

use anyhow::{Context, Result};
use teloxide::{Bot, dispatching::Dispatcher, prelude::*, utils::command::BotCommands};
use tracing::{info, warn};

use crate::{handlers::commands::Command, config::Config, downloader::Downloader};

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init();

    let _ = dotenvy::dotenv().inspect_err(|_| warn!("`.env` is not loaded"));

    // TODO(pencelheimer): point the bot at the local Bot API server (`Bot::set_api_url`, e.g. from
    // `RUSTY_DLP_API_URL`) to lift the 50 MB upload limit of the cloud Bot API.
    let bot = Bot::new(env::var("RUSTY_DLP_BOT_TOKEN").context("RUSTY_DLP_BOT_TOKEN is not set")?);

    let config = Arc::new(Config::new().context("Can't load config")?);

    let downloader = Downloader::new(config.clone());

    bot.set_my_commands(Command::bot_commands())
        .await
        .context("Can't register bot commands")?;

    info!("starting bot");

    let handler = dptree::entry()
        .branch(handlers::commands::dispatcher())
        .branch(handlers::guest::dispatcher())
        .branch(handlers::normal::dispatcher());

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![config, downloader])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    info!("shutting down");

    Ok(())
}
