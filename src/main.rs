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

use futures::Future;
use std::env;
use tokio_core::reactor::Core;

#[macro_use]
mod utils;
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
    // Load the first argument as configuration
    let config = utils::load_config(
        &(env::args().nth(1)
            .expect("Please supply path to the JSON configuration"))
    ).expect("Failed to decode configuration file.");
    println!("{:?}", config);

    // Create the tokio event machine
    let mut core = Core::new().unwrap();
    let mut tg = telegram::Telegram::new(core.handle(), &config.token);

    // TEST
    tg.subscribe(|_, tg, update| {
        tg.subscribe(|id, tg, _| {
            println!("This should happen alternately.");
            tg.unsubscribe(id);
        });
        println!("{:?}", update);
    });
    let work = tg.spin_update_loop();
    core.run(work).unwrap();
}
