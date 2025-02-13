use std::{error::Error, str::FromStr};

use chrono::Duration;
use teloxide::{prelude2::*, types::ChatPermissions, utils::command::BotCommand};

// Derive BotCommand to parse text with a command into this enumeration.
//
//  1. rename = "lowercase" turns all the commands into lowercase letters.
//  2. `description = "..."` specifies a text before all the commands.
//
// That is, you can just call Command::descriptions() to get a description of
// your commands in this format:
// %GENERAL-DESCRIPTION%
// %PREFIX%%COMMAND% - %DESCRIPTION%
#[derive(BotCommand, Clone)]
#[command(
    rename = "lowercase",
    description = "Use commands in format /%command% %num% %unit%",
    parse_with = "split"
)]
enum Command {
    #[command(description = "kick user from chat.")]
    Kick,
    #[command(description = "ban user in chat.")]
    Ban {
        time: u64,
        unit: UnitOfTime,
    },
    #[command(description = "mute user in chat.")]
    Mute {
        time: u64,
        unit: UnitOfTime,
    },
    Help,
}

#[derive(Clone)]
enum UnitOfTime {
    Seconds,
    Minutes,
    Hours,
}

impl FromStr for UnitOfTime {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        match s {
            "h" | "hours" => Ok(UnitOfTime::Hours),
            "m" | "minutes" => Ok(UnitOfTime::Minutes),
            "s" | "seconds" => Ok(UnitOfTime::Seconds),
            _ => Err("Allowed units: h, m, s"),
        }
    }
}

// Calculates time of user restriction.
fn calc_restrict_time(time: u64, unit: UnitOfTime) -> Duration {
    match unit {
        UnitOfTime::Hours => Duration::hours(time as i64),
        UnitOfTime::Minutes => Duration::minutes(time as i64),
        UnitOfTime::Seconds => Duration::seconds(time as i64),
    }
}

type Bot = AutoSend<teloxide::Bot>;

// Kick a user with a replied message.
async fn kick_user(bot: Bot, msg: Message) -> Result<(), Box<dyn Error + Send + Sync>> {
    match msg.reply_to_message() {
        Some(replied) => {
            // bot.unban_chat_member can also kicks a user from a group chat.
            bot.unban_chat_member(msg.chat.id, replied.from().unwrap().id).await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Use this command in reply to another message").await?;
        }
    }
    Ok(())
}

// Mute a user with a replied message.
async fn mute_user(
    bot: Bot,
    msg: Message,
    time: Duration,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match msg.reply_to_message() {
        Some(replied) => {
            bot.restrict_chat_member(
                msg.chat.id,
                replied.from().expect("Must be MessageKind::Common").id,
                ChatPermissions::empty(),
            )
            .until_date(msg.date + time)
            .await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Use this command in a reply to another message!")
                .await?;
        }
    }
    Ok(())
}

// Ban a user with replied message.
async fn ban_user(
    bot: Bot,
    msg: Message,
    time: Duration,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match msg.reply_to_message() {
        Some(replied) => {
            bot.kick_chat_member(
                msg.chat.id,
                replied.from().expect("Must be MessageKind::Common").id,
            )
            .until_date(msg.date + time)
            .await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Use this command in a reply to another message!")
                .await?;
        }
    }
    Ok(())
}

async fn action(
    bot: Bot,
    msg: Message,
    command: Command,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match command {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions()).await?;
        }
        Command::Kick => kick_user(bot, msg).await?,
        Command::Ban { time, unit } => ban_user(bot, msg, calc_restrict_time(time, unit)).await?,
        Command::Mute { time, unit } => mute_user(bot, msg, calc_restrict_time(time, unit)).await?,
    };

    Ok(())
}

#[tokio::main]
async fn main() {
    teloxide::enable_logging!();
    log::info!("Starting admin_bot...");

    let bot = teloxide::Bot::from_env().auto_send();

    teloxide::repls2::commands_repl(bot, action, Command::ty()).await;
}
