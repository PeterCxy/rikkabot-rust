extern crate futures;
extern crate futures_cpupool;
extern crate hyper;
extern crate hyper_tls;
extern crate percent_encoding;
extern crate rand;
extern crate time;
extern crate tokio_core;

// Introduce the serde macros for use in other modules
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;

// Macro for chaining errors
#[macro_use]
extern crate error_chain;

// Logger
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use futures::Future;
use futures_cpupool::CpuPool;
use std::env;
use std::panic;
use std::rc::Rc;
use tokio_core::reactor::Core;

#[macro_use]
mod utils;
mod state;
#[macro_use]
mod telegram;
mod bot;

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
    let pool = Rc::new(CpuPool::new(4));

    let work = bot::bot_main(&mut tg, config, pool.clone())
        .and_then(|tg| tg.spin_update_loop());
    core.run(work).unwrap();
}
