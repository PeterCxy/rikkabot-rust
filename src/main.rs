// Introduce the serde macros for use in other modules
#[macro_use]
extern crate serde_derive;

use std::env;

mod utils;

fn main() {
    // Load the first argument as configuration
    let config = utils::load_config(
        &(env::args().nth(1)
            .expect("Please supply path to the JSON configuration"))
    );
    println!("{:?}", config);
}
