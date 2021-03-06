use futures::Future;
use futures_cpupool::CpuPool;
use rand;
use rand::Rng;
use state::State;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;
use telegram::{Message, Result, Telegram, Update, User};
use time;
use utils::{self, BoxFuture, Config, FutureChainErr};

const HELP_STR: &str = r#"
Hello, I am the bot that tries to become Rikka! (or at least one day ^_^)

/hello - Say hello to Rikka!
/help - Print this message.
/ping - Is Rikka here now?
/rikka - Rikka Rikka Ri!
"#;

macro_rules! cmd_fn_type {
    () => (fn (&mut Telegram, &State, &Config, &str, &Message, Vec<&str>) -> BoxFuture<'a, ()>)
}

fn command_map<'a>() -> HashMap<String, cmd_fn_type!()> {
    string_hashmap! {
        cmd_fn_type!();
        "hello" => cmd_hello,
        "help" => cmd_help,
        "print_cmds" => cmd_print_cmds,
        "ping" => cmd_ping,
        "stats" => cmd_stats,
        "rikka" => cmd_rikka
    }
}

/*
 * Initialize the bot
 * Fetches the username and sets up the subscriber
 * Passes the Telegram object reference back.
 */
pub fn bot_main<'a>(tg: &'a mut Telegram, config: Config, pool: Rc<CpuPool>) -> BoxFuture<'a, &'a mut Telegram> {
    tg.get("getMe", params!{})
        .and_then(|result| {
            assert_result!(Result::User(result), result, Err("I must exist.".into()));
            Ok(result)
        })
        .and_then(move |result| {
            let name = result.username.expect("I must have a username.");
            info!("I am @{}", name);
            let state = State::new(pool.clone(), config.state_file.clone());
            state.load()
                .map(move |state| (config, state, name))
        })
        .and_then(move |(config, state, name)| {
            tg.subscribe(move |_, tg, update| bot_on_update(tg, &state, &config, &name, update));
            Ok(tg)
        })
        .chain_err(|| "Failed to fetch bot username.")
}

fn bot_on_update<'a>(tg: &mut Telegram, state: &State, config: &Config, username: &str, update: &Update) -> BoxFuture<'a, ()> {
    info!("New update received: {:?}", update);
    if let Some(ref msg) = update.message {
        // A new Message
        bot_on_message(tg, state, config, username, msg)
    } else {
        // Unrecognized update. Just ignore it.
        warn!("Unrecognized update received. Ignoring.");
        utils::return_empty()
    }
}

#[allow(unused_variables)]
fn bot_on_message<'a>(tg: &mut Telegram, state: &State, config: &Config, username: &str, msg: &Message) -> BoxFuture<'a, ()> {
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
                return cmd_map.get(&cmd_name).unwrap()(tg, state, config, username, msg, args);
            } else {
                warn!("Unkown command: /{}", cmd_name);
            }
        }
    } else if let Some(ref sticker) = msg.sticker {
        if is_rikka(config, &msg.from) {
            info!("Sticker from Rikka! ID: {}", sticker.file_id);
            // Write to state
            let key = format!("sticker_{}", sticker.file_id);
            let num: i64 = state.get(&key).unwrap_or(0) + 1;
            info!("Recorded use of sticker {}: {}", sticker.file_id, num);
            state.put(&key, &num);
            let total: i64 = state.get("sticker_total").unwrap_or(0) + 1;
            info!("Recorded total stickers: {}", total);
            state.put("sticker_total", &total);
            return state.save_if_needed();
        }
    }
    utils::return_empty()
}

fn is_rikka(config: &Config, usr: &Option<User>) -> bool {
    let res = usr.as_ref().and_then(|user| user.username.as_ref())
        .and_then(|username| Some(username == &config.rikka_name));
    match res {
        Some(result) => result,
        None => false
    }
}

#[allow(unused_variables)]
fn cmd_hello<'a>(tg: &mut Telegram, state: &State, config: &Config, username: &str, msg: &Message, args: Vec<&str>) -> BoxFuture<'a, ()> {
    Box::new(tg.post("sendMessage", params!{
        "chat_id" => msg.chat.id,
        "reply_to_message_id" => msg.message_id,
        "text" => "Hello, Rikka Rikka Ri~"
    }).map(|_| ()))
}

#[allow(unused_variables)]
fn cmd_help<'a>(tg: &mut Telegram, state: &State, config: &Config, username: &str, msg: &Message, args: Vec<&str>) -> BoxFuture<'a, ()> {
    Box::new(tg.post("sendMessage", params!{
        "chat_id" => msg.chat.id,
        "reply_to_message_id" => msg.message_id,
        "text" => HELP_STR
    }).map(|_| ()))
}

// Hidden command: print available commands for use with BotFather
#[allow(unused_variables)]
fn cmd_print_cmds<'a>(tg: &mut Telegram, state: &State, config: &Config, username: &str, msg: &Message, args: Vec<&str>) -> BoxFuture<'a, ()> {
    let cmds: Vec<String> = String::from(HELP_STR).lines()
        .filter(|l| l.starts_with("/"))
        .map(|l| String::from(&l[1..]))
        .collect();
    Box::new(tg.post("sendMessage", params!{
        "chat_id" => msg.chat.id,
        "reply_to_message_id" => msg.message_id,
        "text" => cmds.join("\n")
    }).map(|_| ()))
}

#[allow(unused_variables)]
fn cmd_ping<'a>(tg: &mut Telegram, state: &State, config: &Config, username: &str, msg: &Message, args: Vec<&str>) -> BoxFuture<'a, ()> {
    let t = time::now_utc().to_timespec();
    Box::new(tg.post("sendMessage", params!{
        "chat_id" => msg.chat.id,
        "reply_to_message_id" => msg.message_id,
        "text" => format!("Latency: {}ms", t.sec * 1000 + (t.nsec as i64) / 1000 / 1000 - msg.date * 1000)
    }).map(|_| ()))
}

#[allow(unused_variables)]
fn cmd_stats<'a>(tg: &mut Telegram, state: &State, config: &Config, username: &str, msg: &Message, args: Vec<&str>) -> BoxFuture<'a, ()> {
    Box::new(tg.post("sendMessage", params!{
        "chat_id" => msg.chat.id,
        "reply_to_message_id" => msg.message_id,
        "text" => format!("```\n{}\n```", state.to_json()),
        "parse_mode" => "markdown"
    }).map(|_| ()))
}

/*
 * Choose a random sticker
 * based on the rate of appearance of all the 
 * recorded stickers sent by Rikka.
 * return None if error occurred.
 */
fn random_sticker(state: &State) -> Option<String> {
    let total: i64 = state.get("sticker_total").unwrap_or(0);
    if total == 0 {
        return None;
    }
    let rnd_target = rand::thread_rng().gen_range(0, total);
    let mut records: Vec<(String, i64)> = state.keys().into_iter()
        .filter(|k| k.starts_with("sticker_") && k != "sticker_total")
        .map(|k| (k.clone(), state.get::<i64>(&k).unwrap()))
        .collect();    
    records.sort_by(|&(_, v1), &(_, v2)| {
        if v1 < v2 {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    });
    let mut acc: i64 = 0;
    for (k, v) in records {
        acc += v;
        if acc >= rnd_target {
            return Some(k.replace("sticker_", ""));
        }
    }
    None
}

#[allow(unused_variables)]
fn cmd_rikka<'a>(tg: &mut Telegram, state: &State, config: &Config, username: &str, msg: &Message, args: Vec<&str>) -> BoxFuture<'a, ()> {
    let sticker_id = random_sticker(state);
    if let None = sticker_id {
        return utils::return_empty();
    }
    Box::new(tg.post("sendSticker", params!{
        "chat_id" => msg.chat.id,
        "sticker" => sticker_id.unwrap()
    }).map(|_| ()))
}