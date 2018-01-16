extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate serde_json;
extern crate tokio_core;

use errors::*;
use std::collections::HashMap;
use self::futures::{Future, Stream};
use self::hyper::{Chunk, Client, Uri};
use self::hyper_tls::HttpsConnector;
use self::tokio_core::reactor::{Handle};

use utils;
use utils::{SFuture, FutureChainErr};

pub struct Telegram {
    tokio_handle: Handle,
    token: String,
    last_update: i64
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

    pub fn get(&self, method: &str, params: HashMap<String, Box<ToString>>) -> BoxFutureResponse {
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
}

// Types
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    ok: bool,
    result: Option<Result>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RUpdate {
    update_id: i64
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Result {
    Update(RUpdate),
    Updates(Vec<RUpdate>)
}

pub type BoxFutureResponse = SFuture<Response>;