use std::sync::Arc;

use teloxide::{Bot, requests::Requester, sugar::request::RequestReplyExt, types::{ChatAction, ChatId, InputFile, MessageId}};
use tracing::{info, instrument};

use crate::{error::DownloadError, ext::BotExt, yt_dlp::{MediaKind, YtDlp}};

#[instrument(skip_all, fields(%url, ?kind))]
pub async fn download_and_send(
    bot: &Bot,
    chat_id: ChatId,
    reply_to: MessageId,
    url: &str,
    kind: MediaKind,
    yt_dlp: &Arc<YtDlp>,
) -> Result<(), DownloadError> {
    info!("starting download");

    let chat_action = match kind {
        MediaKind::Video => ChatAction::UploadVideo,
        MediaKind::Audio => ChatAction::UploadDocument,
    };

    let path = bot
        .poll_as_chat_action(chat_id, chat_action, yt_dlp.download(url, kind))
        .await
        .map_err(DownloadError::Download)?;

    info!(?path, "download complete, sending");

    match kind {
        MediaKind::Video => {
            bot.send_video(chat_id, InputFile::file(&path))
                .reply_to(reply_to)
                .await
                .map_err(|e| DownloadError::Send(e.into()))?;
        }
        MediaKind::Audio => {
            bot.send_audio(chat_id, InputFile::file(&path))
                .reply_to(reply_to)
                .await
                .map_err(|e| DownloadError::Send(e.into()))?;
        }
    }

    info!("sent successfully");
    Ok(())
}
