use std::{future::Future, time::Duration};

use teloxide::{Bot, requests::Requester, types::{ChatAction, ChatId}};

pub trait BotExt {
    async fn poll_as_chat_action<F, T>(&self, chat_id: ChatId, action: ChatAction, task: F) -> T
    where
        F: Future<Output = T>;
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
}
