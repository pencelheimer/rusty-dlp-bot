use std::sync::Arc;

use teloxide::types::{Message, MessageKind};

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

pub fn guest_message_data(msg: Message, config: Arc<Config>) -> Option<GuestMessageData> {
    let MessageKind::Common(common) = &msg.kind else {
        return None;
    };

    let query_id = common.guest_query_id.clone()?;

    let link = links_in_msg(msg.clone(), config.clone())
        .map(|links| links.first().clone())
        .or_else(|| links_in_reply(msg, config).map(|links| links.first().clone()));

    Some(GuestMessageData { query_id, link })
}
