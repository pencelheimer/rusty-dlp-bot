use std::{future::Future, time::Duration};

use anyhow::{Result, anyhow};
use teloxide::{
    Bot, RequestError,
    requests::Requester,
    sugar::request::RequestReplyExt,
    types::{
        ChatAction, ChatId, InlineQueryResult, InlineQueryResultArticle, InputFile,
        InputMessageContent, InputMessageContentText, Message, MessageId, SentGuestMessage,
    },
};

use tracing::warn;

use crate::media::{Media, MediaKind};

/// The most files Telegram accepts in one album.
pub const ALBUM_SIZE: usize = 10;

pub trait BotExt {
    async fn poll_as_chat_action<F, T>(&self, chat_id: ChatId, action: ChatAction, task: F) -> T
    where
        F: Future<Output = T>;

    async fn answer_guest_query_with_text(
        &self,
        guest_query_id: impl Into<String>,
        id: impl Into<String>,
        content: impl Into<String> + Clone,
    ) -> Result<SentGuestMessage, RequestError>;

    async fn send_media(
        &self,
        chat_id: ChatId,
        media: &Media,
        reply_to: Option<MessageId>,
    ) -> Result<Message, RequestError>;

    /// Sends the files as albums. Telegram doesn't mix videos and audios in an album, so they are
    /// sent separately, in chunks of at most `ALBUM_SIZE`. A lone file is sent as is.
    async fn send_medias(
        &self,
        chat_id: ChatId,
        medias: &[Media],
        reply_to: Option<MessageId>,
    ) -> Result<(), RequestError>;

    async fn set_media_inline(
        &self,
        chat_id: ChatId,
        target_msg_id: impl Into<String>,
        media: &Media,
    ) -> Result<()>;
}

impl BotExt for Bot {
    async fn poll_as_chat_action<F, T>(&self, chat_id: ChatId, action: ChatAction, task: F) -> T
    where
        F: Future<Output = T>,
    {
        tokio::pin!(task);
        let mut interval = tokio::time::interval(Duration::from_secs(4));
        loop {
            tokio::select! {
                res = &mut task => return res,
                _ = interval.tick() => {
                    let _ = self.send_chat_action(chat_id, action).await;
                }
            }
        }
    }

    async fn answer_guest_query_with_text(
        &self,
        guest_query_id: impl Into<String>,
        id: impl Into<String>,
        content: impl Into<String> + Clone,
    ) -> Result<SentGuestMessage, RequestError> {
        self.answer_guest_query(
            guest_query_id,
            InlineQueryResult::Article(InlineQueryResultArticle::new(
                id,
                content.clone(),
                InputMessageContent::Text(InputMessageContentText::new(content)),
            )),
        )
        .await
    }

    async fn send_media(
        &self,
        chat_id: ChatId,
        media: &Media,
        reply_to: Option<MessageId>,
    ) -> Result<Message, RequestError> {
        let file = InputFile::file(&media.path);
        match media.kind {
            MediaKind::Video => {
                let mut request = self.send_video(chat_id, file);
                if let Some(id) = reply_to {
                    request = request.reply_to(id);
                }
                request.await
            }
            MediaKind::Audio => {
                let mut request = self.send_audio(chat_id, file);
                if let Some(id) = reply_to {
                    request = request.reply_to(id);
                }
                request.await
            }
        }
    }

    async fn send_medias(
        &self,
        chat_id: ChatId,
        medias: &[Media],
        reply_to: Option<MessageId>,
    ) -> Result<(), RequestError> {
        for kind in [MediaKind::Video, MediaKind::Audio] {
            let of_kind: Vec<&Media> = medias.iter().filter(|media| media.kind == kind).collect();

            for chunk in of_kind.chunks(ALBUM_SIZE) {
                if let [media] = chunk {
                    self.send_media(chat_id, media, reply_to).await?;
                    continue;
                }

                let album = chunk
                    .iter()
                    .map(|media| kind.to_input_media(InputFile::file(&media.path)));
                let mut request = self.send_media_group(chat_id, album);
                if let Some(id) = reply_to {
                    request = request.reply_to(id);
                }
                request.await?;
            }
        }

        Ok(())
    }

    async fn set_media_inline(
        &self,
        chat_id: ChatId,
        target_msg_id: impl Into<String>,
        media: &Media,
    ) -> Result<()> {
        let msg = self.send_media(chat_id, media, None).await?;

        let file_id = media.kind.file_id_from(&msg);

        // NOTE(pencelheimer): the upload is already done at this point, so a failed cleanup must
        // not fail the request
        if let Err(e) = self.delete_message(chat_id, msg.id).await {
            warn!(error = %e, "failed to delete the staging message");
        }

        let file_id =
            file_id.ok_or_else(|| anyhow!("Internal error: missing file_id after upload"))?;

        self.edit_message_media_inline(target_msg_id, media.kind.to_input_media(InputFile::file_id(file_id))).await?;

        Ok(())
    }
}
