extern crate futures;
extern crate serde;
extern crate serde_json;
extern crate percent_encoding;

use errors::*;
use std::collections::HashMap;
use std::error;
use std::io::prelude::*;
use std::fs::File;
use self::futures::Future;
use self::percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub token: String,
    pub rikka_name: String
}

/* Load configuration from file
 * Return Err if failed to read file
 * or illegal configuration
 */
pub fn load_config(config: &str) -> Result<Config> {
    Ok(serde_json::from_str(&read_file_str(config)?)?)
}

/*
 * Read file to a string
 */
fn read_file_str(file: &str) -> Result<String> {
    let mut file = File::open(file)?;
    let mut ret = String::new();
    file.read_to_string(&mut ret)?;
    Ok(ret)
}

/*
 * Convert a HashMap to HTTP query string
 */
pub fn build_query_string(params: HashMap<String, Box<ToString>>) -> String {
    params.iter()
        .map(|(k, v)| {
            format!("{}={}", k, utf8_percent_encode(&v.to_string(), DEFAULT_ENCODE_SET).to_string())
        })
        .fold(String::new(), |x, y| x + &y)
}

// Glue code to make error-chain work with futures
// Source: <https://github.com/alexcrichton/sccache/blob/master/src/errors.rs>
pub type SFuture<T> = Box<Future<Item = T, Error = Error>>;

pub trait FutureChainErr<T> {
    fn chain_err<F, E>(self, callback: F) -> SFuture<T>
        where F: FnOnce() -> E + 'static,
              E: Into<ErrorKind>;
}

impl<F> FutureChainErr<F::Item> for F
    where F: Future + 'static,
          F::Error: error::Error + Send + 'static,
{
    fn chain_err<C, E>(self, callback: C) -> SFuture<F::Item>
        where C: FnOnce() -> E + 'static,
              E: Into<ErrorKind>,
    {
        Box::new(self.then(|r| r.chain_err(callback)))
    }
}