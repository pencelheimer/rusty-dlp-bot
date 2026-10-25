use teloxide::types::{Message, User};

pub fn sender(msg: Message) -> Option<User> {
    msg.from
}
