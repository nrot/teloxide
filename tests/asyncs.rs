#[cfg(test)]
#[cfg(feature = "macros")]
mod tests {
    use teloxide::{prelude2::*, types::Update, utils::command::BotCommand};

    #[derive(BotCommand, Clone)]
    enum SimpleCommand {
        #[command(description = "Some command")]
        Help,
    }

    #[derive(Clone)]
    struct ConfigParameters {
        text: String,
    }

    async fn simple_commands_handler(
        msg: Message,
        bot: AutoSend<Bot>,
        cmd: SimpleCommand,
        cfg: ConfigParameters,
    ) -> Result<(), teloxide::RequestError> {
        let txt = match cmd {
            SimpleCommand::Help => SimpleCommand::descriptions(),
        };
        bot.send_message(msg.chat_id(), txt).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_move_thread() {
        teloxide::enable_logging!();
        let bot = Bot::new("some_telegram_token").auto_send();
        let handler = Update::filter_message().branch(
            dptree::entry().filter_command::<SimpleCommand>().endpoint(simple_commands_handler),
        );
        let mut disp = Dispatcher::builder(bot, handler).build();
        let f_disp = disp.setup_ctrlc_handler().dispatch();
        let worker = tokio::spawn(f_disp);
    }
}
