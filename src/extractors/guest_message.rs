use std::sync::Arc;

use teloxide::types::{Me, Message, MessageKind};

use crate::{
    config::Config,
    extractors::links::{Link, links_in_msg, links_in_reply},
};

#[derive(Debug, Clone)]
pub struct GuestMessageData {
    pub query_id: String,
    /// The first supported link from the message, or from the reply if the message has none. A
    /// guest message holds a single file, so the rest are ignored.
    pub link: Option<Link>,
}

pub fn guest_message_data(msg: Message, config: Arc<Config>, me: Me) -> Option<GuestMessageData> {
    let MessageKind::Common(common) = &msg.kind else {
        return None;
    };

    let query_id = common.guest_query_id.clone()?;

    // NOTE(pencelheimer): replying to the bot's own message (e.g. the sent media) makes a guest
    // message too. Without a new link in it, there is nothing to do, so it is ignored silently
    let replies_to_bot = msg.reply_to_message().is_some_and(|reply| {
        [reply.from.as_ref(), reply.via_bot.as_ref()]
            .into_iter()
            .flatten()
            .any(|user| user.id == me.user.id)
    });

    let link = links_in_msg(msg.clone(), config.clone())
        .map(|links| links.first().clone())
        .or_else(|| links_in_reply(msg, config).map(|links| links.first().clone()));

    if link.is_none() && replies_to_bot {
        return None;
    }

    Some(GuestMessageData { query_id, link })
}
