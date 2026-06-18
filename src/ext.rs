use std::{future::Future, time::Duration};

use teloxide::{
    Bot, RequestError,
    requests::Requester,
    types::{
        ChatAction, ChatId, InlineQueryResult, InlineQueryResultArticle, InputMessageContent,
        InputMessageContentText, SentGuestMessage,
    },
};

pub trait BotExt {
    async fn poll_as_chat_action<F, T>(&self, chat_id: ChatId, action: ChatAction, task: F) -> T
    where
        F: Future<Output = T>;

    async fn answer_guest_query_with_text(
        &self,
        guest_query_id: impl Into<String>,
        id: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<SentGuestMessage, RequestError>;
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
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<SentGuestMessage, RequestError> {
        self.answer_guest_query(
            guest_query_id,
            InlineQueryResult::Article(InlineQueryResultArticle::new(
                id,
                title,
                InputMessageContent::Text(InputMessageContentText::new(content)),
            )),
        )
        .await
    }
}
