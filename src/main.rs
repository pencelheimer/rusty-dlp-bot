use anyhow::Result;
use teloxide::{
    Bot,
    dispatching::{Dispatcher, UpdateFilterExt as _},
    requests::{Requester, ResponseResult},
    types::{Message, Update},
};
use tracing::warn;

#[tokio::main]
async fn main() -> Result<()> {
    // TODO(pencelheimer): enable tracing for the CLI or journald

    let _ = dotenvy::dotenv().inspect_err(|_| warn!("`.env` is not loaded"));

    let bot = Bot::from_env();

    let handler = dptree::entry().branch(Update::filter_message().endpoint(message_handler));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn message_handler(
    bot: Bot,
    msg: Message,
) -> ResponseResult<()> {
    let Some(text) = msg.text() else {
        return Ok(());
    };

    bot.send_message(msg.chat.id, format!("got {text}")).await?;

    Ok(())
}
