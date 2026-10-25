use std::sync::Arc;

use anyhow::Result;
use dptree::Handler;
use teloxide::{
    dispatching::DpHandlerDescription, prelude::*, sugar::request::RequestReplyExt, types::ChatAction,
};
use tracing::{instrument, warn};

use crate::{
    config::Config,
    downloader::Downloader,
    extractors::links::{LinksInMsg, links_in_msg},
    ml,
    utils::bot_ext::{ALBUM_SIZE, BotExt as _},
};

pub fn dispatcher() -> Handler<'static, Result<()>, DpHandlerDescription> {
    Update::filter_message()
        .filter_map(links_in_msg)
        .endpoint(handler)
}

#[instrument(skip_all, fields(chat_id = %msg.chat.id, msg_id = %msg.id))]
pub async fn handler(
    bot: Bot,
    downloader: Downloader,
    config: Arc<Config>,
    msg: Message,
    links: LinksInMsg,
) -> Result<()> {
    let results = bot
        .poll_as_chat_action(
            msg.chat.id,
            ChatAction::UploadDocument,
            downloader.download_many(links.iter().take(ALBUM_SIZE).cloned()),
        )
        .await;

    let mut failures = Vec::new();
    let medias: Vec<_> = results
        .into_iter()
        .filter_map(|result| {
            result
                .inspect_err(|e| {
                    warn!(error = format!("{e:#}"), "download failed");
                    failures.push(format!("{e:#}"));
                })
                .ok()
        })
        .collect();

    if !failures.is_empty() {
        bot.send_message(
            msg.chat.id,
            format!("Failed to download media:\n{}", failures.join("\n")),
        )
        .reply_to(msg.id)
        .await?;
    }

    // NOTE(pencelheimer): nothing is sent when every download failed
    if let Err(e) = bot.send_medias(msg.chat.id, &medias, Some(msg.id)).await {
        warn!(error = %e, "failed to send media");
        bot.send_message(
            msg.chat.id,
            format!(
                ml!(
                    ///Failed to send media.
                    ///Contact an admin if it didn't help ({}).
                    ///
                    ///Error: {}
                ),
                config.admin_contacts(),
                e
            ),
        )
        .reply_to(msg.id)
        .await?;
    }

    Ok(())
}
