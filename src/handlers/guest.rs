use std::{sync::Arc, time::Duration};

use anyhow::{Result, anyhow};
use dptree::Handler;
use teloxide::{
    dispatching::DpHandlerDescription,
    prelude::*,
    types::{ParseMode, User},
    utils::html,
};
use tracing::{instrument, warn};

use crate::{
    config::Config,
    downloader::Downloader,
    extractors::{
        guest_message::{GuestMessageData, guest_message_data},
        sender::sender,
    },
    ml,
    utils::bot_ext::BotExt as _,
};

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(2 * 60);

pub fn dispatcher() -> Handler<'static, Result<()>, DpHandlerDescription> {
    Update::filter_guest_message()
        .filter_map(guest_message_data)
        .filter_map(sender)
        .endpoint(handler)
}

#[instrument(skip_all, fields(chat_id = %msg.chat.id, msg_id = %msg.id))]
pub async fn handler(
    bot: Bot,
    downloader: Downloader,
    config: Arc<Config>,
    msg: Message,
    guest_data: GuestMessageData,
    sender: User,
) -> Result<()> {
    let sent = bot
        .answer_guest_query_with_text(guest_data.query_id, "progress", "⏳ Downloading...")
        .await?;

    let Some(link) = guest_data.link else {
        bot.edit_message_text_inline(
            &sent.inline_message_id,
            "There is no supported URL to download",
        )
        .await?;

        return Ok(());
    };

    let result = tokio::time::timeout(DOWNLOAD_TIMEOUT, downloader.download(&link))
        .await
        .unwrap_or_else(|_| {
            Err(anyhow!(
                "download timed out after {} minutes",
                DOWNLOAD_TIMEOUT.as_secs() / 60
            ))
        });

    let media = match result {
        Ok(media) => media,
        Err(e) => {
            warn!(error = format!("{e:#}"), "download failed");
            bot.edit_message_text_inline(
                &sent.inline_message_id,
                format!("Failed to download media: {e:#}"),
            )
            .await?;

            return Ok(());
        }
    };

    let media_set = bot
        .set_media_inline(sender.id.into(), &sent.inline_message_id, &media)
        .await;

    if let Err(e) = media_set {
        warn!(error = format!("{e:#}"), "failed to send media");
        bot.edit_message_text_inline(
            &sent.inline_message_id,
            format!(
                ml!(
                    ///Failed to send media.
                    ///Have you started a private chat with the bot?
                    ///Contact an admin if it didn't help ({}).
                    ///
                    ///Error: <blockquote expandable="true">{}</blockquote>
                ),
                html::escape(&config.admin_contacts()),
                html::escape(&format!("{e:#}"))
            ),
        )
        .parse_mode(ParseMode::Html)
        .await?;

        return Ok(());
    }

    Ok(())
}
