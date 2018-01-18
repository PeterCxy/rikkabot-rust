extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate rand;
extern crate serde_json;
extern crate tokio_core;

use errors::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;
use self::futures::{Future, Stream};
use self::futures::future::Executor;
use self::hyper::{Body, Chunk, Client, Uri};
use self::hyper::client::HttpConnector;
use self::hyper_tls::HttpsConnector;
use self::tokio_core::reactor::{Handle};

use utils;
use utils::{BoxFuture, FutureChainErr};

const REQ_TIMEOUT: u32 = 600;

pub struct Telegram {
    tokio_handle: Handle,
    client: Client<HttpsConnector<HttpConnector>, Body>,
    token: String,
    last_update: i64,
    subscribers: HashMap<i64, Rc<Fn(i64, &mut Telegram, &Update) -> BoxFuture<'static, ()>>>
}

// The Telegram API call implementation
impl Telegram {
    /*
     * Initialize a Telegram instance
     */
    pub fn new(tokio_handle: Handle, token: &str) -> Telegram {
        // Create Hyper client object before anything starts
        let client = Client::configure()
            .connector(HttpsConnector::new(4, &tokio_handle).unwrap())
            .build(&tokio_handle);

        // Initialize the Telegram struct
        Telegram {
            tokio_handle,
            client,
            token: String::from(token),
            last_update: 0,
            subscribers: HashMap::new()
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

    pub fn get<'a, 'b>(&'b self, method: &str, params: HashMap<String, Box<ToString>>) -> BoxFuture<'a, Response> {
        Box::new(self.client
            .get(self.uri_for_method_with_params(method, params))
            .and_then(|res| res.body().concat2())
            .chain_err(|| "GET request failed")
            .and_then(|body: Chunk| {
                serde_json::from_slice::<Response>(&body)
                    .chain_err(|| "Decode failed")
            }))
    }

    fn next_update<'a>(&'a mut self) -> BoxFuture<'a, (&mut Telegram, Vec<Update>)> {
        self.get("getUpdates", params!{
            "timeout" => REQ_TIMEOUT,
            "offset" => self.last_update
        }).and_then(move |resp| {
            if !resp.ok {
                return Err("Failed to fetch updates.".into());
            }

            if let Some(Result::Updates(mut result)) = resp.result {
                if result.len() == 0 {
                    // Do nothing if result is empty
                    // This happens if no message received
                    // within timeout
                    return Ok((self, result));
                }

                // Telegram API did not guarantee the order of messages
                // Although in fact they do
                // To be safe, just ensure the sorting here.
                result.sort_by(|x, y| {
                    if x.update_id < y.update_id {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                });

                // Update the value of `last_update`
                // On the next request, Telegram will
                // mark the old messages as `read`.
                self.last_update = result[result.len() - 1].update_id + 1;
                return Ok((self, result));
            } else {
                return Err("Failed to decode updates.".into());
            }
        }).chain_err(|| "Failed to fetch updates.")
    }

    /*
     * Spin up the tail-recursive loop to fetch new messages
     * TODO: a better way to handle errors
     *       Currently, it will break on errors
     */
    pub fn spin_update_loop<'a>(&'a mut self) -> BoxFuture<'a, ()> {
        Box::new(self.next_update()
            .and_then(move |(new_self, res)| {
                let subscribers = new_self.get_subscribers();
                // Dispatch every update to every subscriber
                for u in res.iter() {
                    for (id, f) in &subscribers {
                        // Executing subscribers will return a Future
                        let fut = f(id.clone(), new_self, u).map_err(|_| ());
                        // Add it to the event loop provided by Tokio
                        if let Err(err) =  new_self.tokio_handle.execute(fut) {
                            println!("Failed to schedule subscriber {}, {:?}", id, err);
                        }
                    }
                }
                new_self.spin_update_loop()
            }))
    }

    fn get_subscribers(&mut self) -> HashMap<i64, Rc<Fn(i64, &mut Telegram, &Update) -> BoxFuture<'static, ()>>> {
        self.subscribers.clone()
    }

    /*
     * Subscribe to `update` events
     * Every callback has its own id
     * which will be passed as the first argument of the closure.
     */
    pub fn subscribe<F>(&mut self, f: F)
        where F: 'static + Fn(i64, &mut Telegram, &Update) -> BoxFuture<'static, ()>
    {
        self.subscribers.insert(rand::random::<i64>(), Rc::new(f));
    }

    /*
     * Remove a previously subscribed callback with id
     */
    pub fn unsubscribe(&mut self, id: i64) {
        self.subscribers.remove(&id);
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