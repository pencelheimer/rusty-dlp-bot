use std::{fs, sync::Arc};

use anyhow::{Context, Result, anyhow};
use dptree::Handler;
use teloxide::{
    dispatching::DpHandlerDescription,
    net::Download,
    prelude::*,
    sugar::request::RequestReplyExt,
    types::ParseMode,
    utils::{command::BotCommands, html},
};
use tracing::{info, instrument, warn};

use crate::{config::Config, downloader::Downloader, media::MediaKind, ml};

pub fn dispatcher() -> Handler<'static, Result<()>, DpHandlerDescription> {
    Update::filter_message()
        .filter_command::<Command>()
        .endpoint(handler)
}


#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
pub enum Command {
    #[command(description = "show the welcome message")]
    Start,

    #[command(description = "show this help")]
    Help,

    #[command(description = "list the supported hosts")]
    Sources,

    #[command(description = "(admin) add a host: /addsource <host> <video|audio>")]
    AddSource(String),

    #[command(description = "(admin) remove a host: /removesource <host>")]
    RemoveSource(String),

    #[command(description = "(admin) update yt-dlp")]
    Update,

    #[command(description = "(admin) list admins")]
    Admins,

    #[command(description = "(admin) add an admin: /addadmin <@username>")]
    AddAdmin(String),

    #[command(description = "(admin) remove an admin: /removeadmin <@username>")]
    RemoveAdmin(String),

    #[command(
        description = "(admin) replace cookies: send cookies.txt with this caption, or reply to it"
    )]
    SetCookies,
}

impl Command {
    fn admin_only(&self) -> bool {
        !matches!(self, Command::Start | Command::Help | Command::Sources)
    }
}

const START: &str = ml!(
    ///Send me a link to a video or a track and I'll download it.
    ///In groups you can just post the link, and I'll catch it.
    ///Also you can just tag me in any chat!
    ///
    ////help - all commands
    ////sources - supported hosts
);

#[instrument(skip_all, fields(chat_id = %msg.chat.id, ?cmd))]
async fn handler(
    bot: Bot,
    msg: Message,
    cmd: Command,
    config: Arc<Config>,
    downloader: Downloader,
) -> Result<()> {
    let is_admin = config.is_admin(msg.from.as_ref().and_then(|u| u.username.as_deref()));

    let text = if cmd.admin_only() && !is_admin {
        Ok("This command is for admins only".to_owned())
    } else {
        match cmd {
            Command::Start => Ok(START.to_owned()),
            Command::Help => Ok(html::escape(&Command::descriptions().to_string())),
            Command::Sources => Ok(sources(&config)),
            Command::AddSource(args) => add_source(&config, &args),
            Command::RemoveSource(args) => remove_source(&config, &args),
            Command::Update => downloader.update().await.map(|()| "✅ yt-dlp updated".to_owned()),
            Command::Admins => Ok(admins(&config)),
            Command::AddAdmin(args) => add_admin(&config, &args),
            Command::RemoveAdmin(args) => remove_admin(&config, &args),
            Command::SetCookies => set_cookies(&bot, &msg, &config).await,
        }
    };

    let text = text
        .inspect(|_| info!("command handled"))
        .unwrap_or_else(|e| {
            warn!(error = format!("{e:#}"), "command failed");
            format!("Failed: {}", html::escape(&format!("{e:#}")))
        });

    bot.send_message(msg.chat.id, text)
        .reply_to(msg.id)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}

fn code(text: &str) -> String {
    html::code_inline(&html::escape(text))
}

fn sources(config: &Config) -> String {
    let list = config
        .sources()
        .iter()
        .map(|(host, kind)| format!("• {} → {kind}", code(host)))
        .collect::<Vec<_>>()
        .join("\n");

    if list.is_empty() { "No sources configured".to_owned() } else { list }
}

fn add_source(config: &Config, args: &str) -> Result<String> {
    let mut parts = args.split_whitespace();
    let (Some(host), Some(kind), None) = (parts.next(), parts.next(), parts.next()) else {
        return Ok("Usage: /addsource &lt;host&gt; &lt;video|audio&gt;".to_owned());
    };

    let kind: MediaKind = kind.parse().map_err(|e: String| anyhow!(e))?;
    let host = host.to_lowercase();

    let note = config
        .add_source(host.clone(), kind)?
        .map(|previous| format!(" (was {previous})"))
        .unwrap_or_default();

    Ok(format!("✅ {} → {kind}{note}", code(&host)))
}

fn remove_source(config: &Config, args: &str) -> Result<String> {
    let host = args.trim().to_lowercase();
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Ok("Usage: /removesource &lt;host&gt;".to_owned());
    }

    Ok(match config.remove_source(&host)? {
        Some(_) => format!("🗑 Removed {}", code(&host)),
        None => "No such host in the sources".to_owned(),
    })
}

fn admins(config: &Config) -> String {
    let list = config
        .admins()
        .iter()
        .map(|admin| format!("• {}", html::escape(admin)))
        .collect::<Vec<_>>()
        .join("\n");

    if list.is_empty() { "No admins".to_owned() } else { list }
}

/// The `@username` argument without the `@`, if it is a valid Telegram username.
fn username(args: &str) -> Option<&str> {
    let name = args.trim().trim_start_matches('@');
    let valid = (5..=32).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    valid.then_some(name)
}

fn add_admin(config: &Config, args: &str) -> Result<String> {
    let Some(name) = username(args) else {
        return Ok("Usage: /addadmin &lt;@username&gt;".to_owned());
    };

    Ok(if config.add_admin(name)? {
        format!("✅ @{} is an admin now", html::escape(name))
    } else {
        "Already an admin".to_owned()
    })
}

fn remove_admin(config: &Config, args: &str) -> Result<String> {
    let Some(name) = username(args) else {
        return Ok("Usage: /removeadmin &lt;@username&gt;".to_owned());
    };

    Ok(if config.remove_admin(name)? {
        format!("🗑 @{} is no longer an admin", html::escape(name))
    } else {
        "No such admin".to_owned()
    })
}

async fn set_cookies(bot: &Bot, msg: &Message, config: &Config) -> Result<String> {
    let Some(doc) = msg
        .document()
        .or_else(|| msg.reply_to_message().and_then(|m| m.document()))
    else {
        return Ok(
            "Send a cookies.txt file with the caption /setcookies, or reply to one with /setcookies"
                .to_owned(),
        );
    };

    let file = bot.get_file(doc.file.id.clone()).await?;
    let mut data = Vec::new();
    bot.download_file(&file.path, &mut data).await?;

    fs::write(&config.cookies_file, data).context("Can't write the cookies")?;

    Ok("✅ Cookies updated".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames() {
        assert_eq!(username("@pencel_z_kavunom"), Some("pencel_z_kavunom"));
        assert_eq!(username("ab"), None);
        assert_eq!(username("bad name"), None);
    }
}
