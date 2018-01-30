use errors::*;
use serde_json;
use std::collections::HashMap;
use std::error;
use std::io::prelude::*;
use std::fs::File;
use futures::{future, Future};
use futures_cpupool::CpuPool;
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub token: String,
    pub rikka_name: String,
    pub state_file: String
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
pub fn read_file_str(file: &str) -> Result<String> {
    let mut file = File::open(file)?;
    let mut ret = String::new();
    file.read_to_string(&mut ret)?;
    Ok(ret)
}

pub fn read_file_str_async<'a>(pool: &CpuPool, file: String) -> BoxFuture<'a, String> {
    Box::new(pool.spawn_fn(move || {
        read_file_str(&file)
    }))
}

/*
 * Write string to file
 */
pub fn write_file_str(file: &str, text: &str) -> Result<()> {
    let mut file = File::create(file)?;
    file.write_all(text.as_bytes())
        .chain_err(|| "Failed to write to file")
}

pub fn write_file_str_async<'a>(pool: &CpuPool, file: String, text: String) -> BoxFuture<'a, ()> {
    Box::new(pool.spawn_fn(move || {
        write_file_str(&file, &text)
    }))
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
 * A typed HashMap literal.
 * Automatic inference does not seem to work
 * when the value needs to be boxed.
 * 
 * Syntax:
 * 
 * let m = hashmap!{
 *     KeyType; ValueType;
 *     key1 => value1,
 *     key2 => value2,
 *     ....
 * };
 */
macro_rules! hashmap {
    (
        $kt:ty; $vt: ty;
        $(
            $x:expr => $y:expr
        ),*
    ) => {
        {
            #[allow(unused_mut)]
            let mut m: HashMap<$kt, $vt> = HashMap::new();
            $(
                m.insert($x, $y);
            )*
            m
        }
    }
}

macro_rules! string_hashmap {
    (
        $vt: ty;
        $(
            $x:expr => $y:expr
        ),*
    ) => {
        hashmap! {
            String; $vt;
            $(
                String::from($x) => $y
            ),*
        }
    }
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
        string_hashmap!{
            Box<ToString>;
            $(
                $x => Box::new($y)
            ),*
        }
    }
}

pub fn return_empty<'a>() -> BoxFuture<'a, ()> {
    Box::new(future::ok(()))
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