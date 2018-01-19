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
        .collect::<Vec<String>>()
        .join("&")
}

/*
 * Build HTTP request params (HashMap<String, Box<ToString>>)
 * Usage:
 *  let options = params!{
 *      "key1" => value2,
 *      "key2" => value2
 * }
 */
macro_rules! params {
    (
        $(
            $x:expr => $y:expr
        ),*
    ) => {
        // Expand to a block so that we can directly assign to a variable
        {
            #[allow(unused_mut)]
            let mut m: HashMap<String, Box<ToString>> = HashMap::new();
            $(
                m.insert(String::from($x), Box::new($y));
            )*
            m
        }
    }
}

// Glue code to make error-chain work with futures
// Source: <https://github.com/alexcrichton/sccache/blob/master/src/errors.rs>
// Modified to avoid static lifetimes
pub type BoxFuture<'a, T> = Box<'a + Future<Item = T, Error = Error>>;

pub trait FutureChainErr<'a, T> {
    fn chain_err<F, E>(self, callback: F) -> BoxFuture<'a, T>
        where F: FnOnce() -> E + 'a,
              E: Into<ErrorKind>;
}

impl<'a, F> FutureChainErr<'a, F::Item> for F
    where F: Future + 'a,
          F::Error: error::Error + Send + 'static,
{
    fn chain_err<C, E>(self, callback: C) -> BoxFuture<'a, F::Item>
        where C: FnOnce() -> E + 'a,
              E: Into<ErrorKind>,
    {
        Box::new(self.then(|r| r.chain_err(callback)))
    }
}