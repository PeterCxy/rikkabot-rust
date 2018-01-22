use futures::Future;
use std::collections::HashMap;
use telegram::{Message, Result, Telegram, Update};
use time;
use utils::{self, BoxFuture, Config, FutureChainErr};

macro_rules! cmd_fn_type {
    () => (fn (&mut Telegram, &Config, &str, &Message, Vec<&str>) -> BoxFuture<'a, ()>)
}

fn command_map<'a>() -> HashMap<String, cmd_fn_type!()> {
    string_hashmap! {
        cmd_fn_type!();
        "hello" => cmd_hello,
        "ping" => cmd_ping
    }
}

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
            
            // Find the implementation of the invoked command
            let cmd_map = command_map();
            if cmd_map.contains_key(&cmd_name) {
                return cmd_map.get(&cmd_name).unwrap()(tg, config, username, msg, args);
            } else {
                warn!("Unkown command: /{}", cmd_name);
            }
        }
    }
    // TODO: implement an automatic sticker bot and allow user to get statistics / a random sticker
    utils::return_empty()
}

#[allow(unused_variables)]
fn cmd_hello<'a>(tg: &mut Telegram, config: &Config, username: &str, msg: &Message, args: Vec<&str>) -> BoxFuture<'a, ()> {
    Box::new(tg.post("sendMessage", params!{
        "chat_id" => msg.chat.id,
        "reply_to_message_id" => msg.message_id,
        "text" => "Hello, Rikka Rikka Ri~"
    }).map(|_| ()))
}

#[allow(unused_variables)]
fn cmd_ping<'a>(tg: &mut Telegram, config: &Config, usernasme: &str, msg: &Message, args: Vec<&str>) -> BoxFuture<'a, ()> {
    let t = time::get_time();
    Box::new(tg.post("sendMessage", params!{
        "chat_id" => msg.chat.id,
        "reply_to_message_id" => msg.message_id,
        "text" => format!("Latency: {}ms", t.sec * 1000 + (t.nsec as i64) / 1000 / 1000 - msg.date * 1000)
    }).map(|_| ()))
}