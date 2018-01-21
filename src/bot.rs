use futures::Future;
use std::collections::HashMap;
use telegram::{Message, Result, Telegram, Update};
use utils::{self, BoxFuture, Config, FutureChainErr};

/*
 * Initialize the bot
 * Fetches the username and sets up the subscriber
 * Passes the Telegram object reference back.
 */
pub fn bot_main<'a>(tg: &'a mut Telegram, config: Config) -> BoxFuture<'a, &mut Telegram> {
    tg.get("getMe", params!{})
        .and_then(move |result| {
            assert_result!(Result::User(result), result, Err("I must exist.".into()));
            let name = result.username.expect("I must have a username.");
            info!("I am @{}", name);
            tg.subscribe(move |_, tg, update| bot_on_update(tg, &config, &name, update));
            return Ok(tg);
        })
        .chain_err(|| "Failed to fetch bot username.")
}

fn bot_on_update<'a>(tg: &mut Telegram, config: &Config, username: &str, update: &Update) -> BoxFuture<'a, ()> {
    info!("New update received: {:?}", update);
    if let Some(ref msg) = update.message {
        // A new Message
        bot_on_message(tg, config, username, msg)
    } else {
        // Unrecognized update. Just ignore it.
        warn!("Unrecognized update received. Ignoring.");
        utils::return_empty()
    }
}

#[allow(unused_variables)]
fn bot_on_message<'a>(tg: &mut Telegram, config: &Config, username: &str, msg: &Message) -> BoxFuture<'a, ()> {
    if let Some(ref text) = msg.text {
        // A text message
        if text.starts_with("/") {
            // (Possibly) a command
            let args: Vec<&str> = text.split(" ").collect(); // TODO: proper argument parser
            let cmd = args[0];
            let username_tail = &format!("{}{}", "@", username);
            if cmd.contains("@") && !cmd.ends_with(username_tail) {
                // A command can contain `@` to indicate the callee
                // e.g. /test@Rikka
                // If the username does not match the bot we are operating
                // Then it is not a command for us
                return utils::return_empty();
            }
            
            // This is a command for us. Trim off the unneeded information
            // And see what the command is.
            let cmd_name = cmd.replacen("/", "", 1).replace(username_tail, "");
            info!("Command invoked: /{} from message {}", cmd_name, msg.message_id);
            // TODO: Finish implementation
        }
    }
    // TODO: implement an automatic sticker bot
    utils::return_empty()
}