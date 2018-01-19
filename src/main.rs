extern crate futures;
extern crate hyper;
extern crate tokio_core;

// Introduce the serde macros for use in other modules
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

// Macro for chaining errors
#[macro_use]
extern crate error_chain;

// Logger
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use futures::future;
use std::env;
use std::panic;
use tokio_core::reactor::Core;

#[macro_use]
mod utils;
#[macro_use]
mod telegram;

mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain! {
        foreign_links {
            Hyper(::hyper::error::Error);
            SerdeJson(::serde_json::Error);
            IO(::std::io::Error);
        }
    }
}

fn main() {
    // Initialize logging and panic hook.
    // Accept log level via RIKKA_LOG env
    if let Err(_) = env::var("RIKKA_LOG") {
        env::set_var("RIKKA_LOG", "info");
    }

    pretty_env_logger::init_custom_env("RIKKA_LOG");
    panic::set_hook(Box::new(|info| {
        let mut err_str = "null".to_string();
        if let Some(s) = info.payload().downcast_ref::<String>() {
            err_str = s.to_string();
        } else if let Some(s) = info.payload().downcast_ref::<&str>() {
            err_str = s.to_string();
        }
        error!("panic: {}", err_str);
        if let Some(location) = info.location() {
            error!("panic location: {:?}", location);
        }
    }));

    // Load the first argument as configuration
    let config = utils::load_config(
        &(env::args().nth(1)
            .expect("Please supply path to the JSON configuration"))
    ).expect("Failed to decode configuration file.");
    info!("Config loaded.");

    // Create the tokio event machine
    let mut core = Core::new().expect("WTF: Cannot create event loop.");
    let mut tg = telegram::Telegram::new(core.handle(), &config.token);

    // TEST
    tg.subscribe(|_, tg, update| {
        tg.subscribe(|id, tg, _| {
            info!("This should happen alternately.");
            tg.unsubscribe(id);
            Box::new(future::ok(()))
        });
        info!("new update: {:?}", update);
        Box::new(future::ok(()))
    });
    let work = tg.spin_update_loop();
    core.run(work).unwrap();
}
