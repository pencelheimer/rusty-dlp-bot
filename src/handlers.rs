use std::sync::Arc;

use linkify::{LinkFinder, LinkKind};
use teloxide::{
    Bot,
    requests::{Requester, ResponseResult},
    sugar::request::RequestReplyExt,
    types::{
        ChatId, FileId, InlineQueryResult, InlineQueryResultArticle, InputFile, InputMedia,
        InputMediaAudio, InputMediaVideo, InputMessageContent, InputMessageContentText, Message,
        MessageId, MessageKind, User,
    },
    utils::command::BotCommands,
};
use tracing::{debug, info, instrument, warn};

use crate::{
    config::Config, download::download_and_send, downloader::Downloader, yt_dlp::MediaKind,
};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum Command {
    #[command(description = "show this help message")]
    Start,
    #[command(description = "download audio or video from a URL")]
    Dl(String),
}

fn resolve_url<'a>(arg: &'a str, replied: Option<&'a Message>) -> Option<&'a str> {
    if !arg.is_empty() {
        Some(arg)
    } else {
        replied.and_then(|m| m.text()).and_then(extract_url)
    }
}

// TODO(pencelheimer): spawns new finder on each msg, should at least make it the global static
fn extract_url(text: &str) -> Option<&str> {
    LinkFinder::new()
        .kinds(&[LinkKind::Url])
        .links(text)
        .next()
        .map(|l| l.as_str())
}

/// Downloads `url` and uploads it to the caller's private chat to get a Telegram
/// `file_id`. Returns `(file_id, upload_message_id)` on success, or a
/// user-facing error string on failure.
#[instrument(skip_all, fields(url = url.get(..80).unwrap_or(url), ?kind))]
async fn stage_download(
    bot: &Bot,
    downloader: &Downloader,
    url: &str,
    kind: MediaKind,
    caller: &User,
) -> Result<(FileId, MessageId), String> {
    let path = downloader.download(url, kind).await.map_err(|e| {
        warn!(error = %e, "download failed");
        format!("Download failed: {e}")
    })?;

    info!(?path, "download complete, uploading to caller chat");
    let caller_chat: ChatId = caller.id.into();

    let m = match kind {
        MediaKind::Video => bot.send_video(caller_chat, InputFile::file(&path)).await,
        MediaKind::Audio => bot.send_audio(caller_chat, InputFile::file(&path)).await,
    }
    .map_err(|e| {
        warn!(error = %e, "upload to caller chat failed");
        "To receive downloads, open a private chat with this bot and send /start first.".to_owned()
    })?;

    let file_id = match kind {
        MediaKind::Video => m.video().map(|v| v.file.id.clone()),
        MediaKind::Audio => m.audio().map(|a| a.file.id.clone()),
    }
    .ok_or_else(|| "Internal error: missing file_id after upload".to_owned())?;

    Ok((file_id, m.id))
}

#[instrument(skip_all, fields(chat_id = %msg.chat.id, msg_id = %msg.id))]
pub async fn on_auto(
    bot: Bot,
    msg: Message,
    config: Arc<Config>,
    downloader: Arc<Downloader>,
) -> ResponseResult<()> {
    let Some(text) = msg.text() else {
        return Ok(());
    };

    let Some(url) = extract_url(text) else {
        debug!("no url in message");
        return Ok(());
    };

    let Some(kind) = config.kind_for(url) else {
        debug!(url, "url not in configured domains");
        return Ok(());
    };
    info!(
        ?kind,
        url = url.get(..80).unwrap_or(url),
        "recognized, downloading"
    );

    if let Err(e) = download_and_send(&bot, msg.chat.id, msg.id, url, kind, &downloader).await {
        warn!(error = %e, "download failed");
        bot.send_message(msg.chat.id, format!("`{e}`"))
            .reply_to(msg.id)
            .await?;
    }

    Ok(())
}

#[instrument(skip_all, fields(chat_id = %msg.chat.id, msg_id = %msg.id))]
pub async fn on_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    config: Arc<Config>,
    downloader: Arc<Downloader>,
) -> ResponseResult<()> {
    if let Command::Start = cmd {
        bot.send_message(msg.chat.id, Command::descriptions().to_string())
            .await?;
        return Ok(());
    }

    let Command::Dl(arg) = cmd else { return Ok(()) };

    let Some(url) = resolve_url(arg.trim(), msg.reply_to_message()) else {
        bot.send_message(
            msg.chat.id,
            "Provide a URL, or reply to a message containing one.",
        )
        .reply_to(msg.id)
        .await?;
        return Ok(());
    };

    let kind = config.kind_for(url).unwrap_or(MediaKind::Video);
    info!(
        ?kind,
        url = url.get(..80).unwrap_or(url),
        "recognized, downloading via command"
    );

    if let Err(e) = download_and_send(&bot, msg.chat.id, msg.id, url, kind, &downloader).await {
        warn!(error = %e, "download failed");
        bot.send_message(msg.chat.id, format!("`{e}`"))
            .reply_to(msg.id)
            .await?;
    }

    Ok(())
}

#[instrument(skip_all, fields(chat_id = %msg.chat.id, msg_id = %msg.id))]
pub async fn on_guest_message(
    bot: Bot,
    msg: Message,
    config: Arc<Config>,
    downloader: Arc<Downloader>,
) -> ResponseResult<()> {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let Some(url) = extract_url(text) else {
        debug!("no url in guest message");
        return Ok(());
    };
    let Some(kind) = config.kind_for(url) else {
        debug!(
            url = url.get(..80).unwrap_or(url),
            "url not in configured domains"
        );
        return Ok(());
    };

    let MessageKind::Common(common) = &msg.kind else {
        warn!("guest message is not MessageKind::Common");
        return Ok(());
    };
    let (Some(guest_query_id), Some(caller)) = (&common.guest_query_id, &msg.from) else {
        warn!(guest_query_id = ?common.guest_query_id, from = ?msg.from, "missing guest_query_id or sender");
        return Ok(());
    };
    info!(guest_query_id, caller_id = %caller.id, ?kind, "starting guest download");

    let sent = bot
        .answer_guest_query(
            guest_query_id,
            InlineQueryResult::Article(InlineQueryResultArticle::new(
                "progress",
                "⏳ Downloading...",
                InputMessageContent::Text(InputMessageContentText::new("Downloading...")),
            )),
        )
        .await?;

    match stage_download(&bot, &downloader, url, kind, caller).await {
        Ok((file_id, upload_id)) => {
            let caller_chat: ChatId = caller.id.into();
            let _ = bot.delete_message(caller_chat, upload_id).await;
            let media = match kind {
                MediaKind::Video => {
                    InputMedia::Video(InputMediaVideo::new(InputFile::file_id(file_id)))
                }
                MediaKind::Audio => {
                    InputMedia::Audio(InputMediaAudio::new(InputFile::file_id(file_id)))
                }
            };
            bot.edit_message_media_inline(&sent.inline_message_id, media)
                .await?;
            info!("guest query answered with media");
        }
        Err(e) => {
            bot.edit_message_text_inline(&sent.inline_message_id, e)
                .await?;
        }
    }

    Ok(())
}
