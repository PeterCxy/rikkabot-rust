extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate serde_json;
extern crate tokio_core;

use errors::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use self::futures::{Future, Stream};
use self::hyper::{Chunk, Client, Uri};
use self::hyper_tls::HttpsConnector;
use self::tokio_core::reactor::{Handle};

use utils;
use utils::{SFuture, FutureChainErr};

const REQ_TIMEOUT: u32 = 600;

pub struct Telegram {
    tokio_handle: Handle,
    token: String,
    pub last_update: i64
}

// The Telegram API call implementation
impl Telegram {
    /*
     * Initialize a Telegram instance
     */
    pub fn new(tokio_handle: Handle, token: &str) -> Telegram {
        Telegram {
            tokio_handle,
            token: String::from(token),
            last_update: 0
        }
    }

    fn uri_for_method(&self, method: &str) -> Uri {
        format!("https://api.telegram.org/bot{}/{}", self.token, method)
            .parse()
            .expect("Illegal URL")
    }

    fn uri_for_method_with_params(&self, method: &str, params: HashMap<String, Box<ToString>>) -> Uri {
        let qs = utils::build_query_string(params);
        format!("https://api.telegram.org/bot{}/{}?{}", self.token, method, qs)
            .parse()
            .expect("Illegal URL")
    }

    pub fn get<'a>(&self, method: &str, params: HashMap<String, Box<ToString>>) -> BoxFutureResponse<'a> {
        Client::configure()
            .connector(HttpsConnector::new(4, &self.tokio_handle).unwrap())
            .build(&self.tokio_handle)
            .get(self.uri_for_method_with_params(method, params))
            .and_then(|res| res.body().concat2())
            .chain_err(|| "GET request failed")
            .and_then(|body: Chunk| {
                serde_json::from_slice::<Response>(&body)
                    .chain_err(|| "Decode failed")
            })
            .chain_err(|| "GET request failed")
    }

    pub fn next_update<'a>(&'a mut self) -> SFuture<'a, Vec<Update>> {
        self.get("getUpdates", params!{
            "timeout" => REQ_TIMEOUT,
            "offset" => self.last_update
        }).and_then(move |resp| {
            if !resp.ok {
                return Err("Failed to fetch updates.".into());
            }

            if let Some(Result::Updates(mut result)) = resp.result {
                result.sort_by(|x, y| {
                    if x.update_id < y.update_id {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                });
                self.last_update = result[result.len() - 1].update_id;
                return Ok(result);
            } else {
                return Err("Failed to decode updates.".into());
            }
        }).chain_err(|| "Failed to fetch updates.")
    }
}

// Types
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    ok: bool,
    result: Option<Result>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Update {
    update_id: i64,
    message: Option<Message>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    message_id: i64,
    text: Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Result {
    Updates(Vec<Update>)
}

pub type BoxFutureResponse<'a> = SFuture<'a, Response>;