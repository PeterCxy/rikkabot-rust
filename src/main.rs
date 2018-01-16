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
use std::collections::HashMap;
use std::env;
use tokio_core::reactor::Core;

mod telegram;
mod utils;

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
    let tg = telegram::Telegram::new(core.handle(), &config.token);

    // TEST
    let mut map: HashMap<String, Box<ToString>> = HashMap::new();
    map.insert(String::from("timeout"), Box::new("600"));
    let work = tg.get("getUpdates", map)
        .and_then(|res| {
            println!("{:?}", res);
            Ok(())
        });
    core.run(work).unwrap();
}
