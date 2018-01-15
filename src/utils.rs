extern crate serde;
extern crate serde_json;

use std::io;
use std::io::prelude::*;
use std::fs::File;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub token: String,
    pub rikka_name: String
}

/* Load configuration from file
 * Return Err if failed to read file
 * or illegal configuration
 */
pub fn load_config(config: &str) -> Result<Config, io::Error> {
    Ok(serde_json::from_str(&read_file_str(config)?)?)
}

/*
 * Read file to a string
 */
fn read_file_str(file: &str) -> Result<String, io::Error> {
    let mut file = File::open(file)?;
    let mut ret = String::new();
    file.read_to_string(&mut ret)?;
    Ok(ret)
}