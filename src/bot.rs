use futures::{future, Future};
use std::collections::HashMap;
use telegram::{Result, Telegram, Update};
use utils::{BoxFuture, Config, FutureChainErr};

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

#[allow(unused_variables)]
fn bot_on_update<'a>(tg: &mut Telegram, config: &Config, username: &str, update: &Update) -> BoxFuture<'a, ()> {
    info!("New update received: {:?}", update);
    Box::new(future::ok(()))
}