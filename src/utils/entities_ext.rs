use std::collections::HashSet;

use teloxide::types::{MessageEntityKind, MessageEntityRef};
use url::Url;

pub trait EntitiesExt {
    /// Collects the links Telegram has marked in a message: plain ones (`url` entities, parsed from
    /// their text) and hidden ones (`text_link` entities, e.g. "listen here" with an attached URL).
    fn links(&self) -> HashSet<Url>;
}

impl EntitiesExt for [MessageEntityRef<'_>] {
    fn links(&self) -> HashSet<Url> {
        self.iter()
            .filter_map(|entity| match entity.kind() {
                MessageEntityKind::Url => entity.text().parse().ok(),
                MessageEntityKind::TextLink { url } => Some(url.clone()),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use teloxide::types::MessageEntity;

    use super::*;

    #[test]
    fn finds_plain_and_hidden_links_without_duplicates() {
        let text = "listen here https://youtu.be/plain youtu.be/bare https://youtu.be/plain";
        let hidden: Url = "https://youtu.be/hidden".parse().unwrap();
        let entities = [
            MessageEntity::text_link(hidden.clone(), 0, 11),
            MessageEntity::new(MessageEntityKind::Url, 12, 22),
            MessageEntity::new(MessageEntityKind::Url, 35, 13),
            MessageEntity::new(MessageEntityKind::Url, 49, 22),
        ];

        let links = MessageEntityRef::parse(text, &entities).links();

        assert_eq!(links, HashSet::from([hidden, "https://youtu.be/plain".parse().unwrap()]));
    }
}
