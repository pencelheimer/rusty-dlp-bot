use std::{collections::HashMap, sync::Arc};

use teloxide::{
    Bot,
    requests::Requester,
    sugar::request::RequestReplyExt,
    types::{Message},
};
use teloxide::requests::ResponseResult;
use tracing::warn;
use url::Url;

use crate::{
    download::download_and_send,
    yt_dlp::{MediaKind, YtDlp},
};

pub async fn message_handler(
    bot: Bot,
    msg: Message,
    allowed_domains: Arc<HashMap<String, MediaKind>>,
    yt_dlp: Arc<YtDlp>,
) -> ResponseResult<()> {
    let Some(text) = msg.text() else {
        return Ok(());
    };

    let Ok(parsed) = Url::parse(text) else {
        return Ok(());
    };

    let Some(&kind) = parsed.host_str().and_then(|h| allowed_domains.get(h)) else {
        return Ok(());
    };

    if let Err(e) = download_and_send(&bot, msg.chat.id, msg.id, parsed.as_str(), kind, &yt_dlp).await {
        warn!(error = %e, "download failed");
        bot.send_message(msg.chat.id, format!("`{e}`"))
            .reply_to(msg.id)
            .await?;
    }

    Ok(())
}
