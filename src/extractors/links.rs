use std::{collections::HashSet, marker::PhantomData, ops::Deref, sync::Arc};

use teloxide::types::Message;
use url::Url;

use crate::{config::Config, media::MediaKind, utils::entities_ext::EntitiesExt as _};

/// A link from one of the configured sources.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Link {
    pub url: Url,
    pub kind: MediaKind,
}

/// A non-empty set of the supported links. `S` only tells where they were found, so that `dptree`
/// can inject the links from different places as different types.
///
/// The set is never empty: the field is private and `from_supported` is the only constructor.
#[derive(Debug, Clone)]
pub struct Links<S>(HashSet<Link>, PhantomData<S>);

#[derive(Debug, Clone)]
pub enum InMsg {}

#[derive(Debug, Clone)]
pub enum InReply {}

pub type LinksInMsg = Links<InMsg>;
pub type LinksInReply = Links<InReply>;

impl<S> Links<S> {
    /// Keeps only the links from the configured sources, `None` if there are none.
    pub fn from_supported(links: impl IntoIterator<Item = Url>, config: &Config) -> Option<Self> {
        let links: HashSet<_> = links
            .into_iter()
            .filter_map(|url| config.kind_for(&url).map(|kind| Link { url, kind }))
            .collect();

        (!links.is_empty()).then_some(Self(links, PhantomData))
    }

    pub fn first(&self) -> &Link {
        self.0.iter().next().expect("`Links` is never empty")
    }
}

impl<S> Deref for Links<S> {
    type Target = HashSet<Link>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn links_in_msg(msg: Message, config: Arc<Config>) -> Option<LinksInMsg> {
    LinksInMsg::from_supported(msg.parse_entities()?.links(), &config)
}

pub fn links_in_reply(msg: Message, config: Arc<Config>) -> Option<LinksInReply> {
    let reply = msg.reply_to_message()?;
    let entities = reply.parse_entities().or_else(|| reply.parse_caption_entities())?;
    LinksInReply::from_supported(entities.links(), &config)
}

