use errors;
use errors::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;
use futures::{Future, Stream};
use futures::future::Executor;
use hyper::{Body, Chunk, Client, Method, Request, Uri};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use rand;
use serde_json;
use tokio_core::reactor::{Handle};

use utils;
use utils::{BoxFuture, FutureChainErr};

macro_rules! assert_result {
    /*
     * Shorthand to assert the response type
     * used on `and_then` when calling an API.
     * 
     * Replaces $r with the actual value of
     * the result if it matches $t. Otherwise,
     * directly end the current function and return $d.
     * Note that $t must contain $r contained in
     * the parenthesis.
     * 
     * Example:
     * |result| {
     *     assert_result!(Result::Update(result), result, default_value);
     *     // And now `result` will contain the actual result.
     * }
     */
    ($t:pat, $r:ident, $d:expr) => {
        let _result = {
            match $r {
                $t => Some($r),
                x => {
                    warn!("Response type mismatch: expected {}, found {:?}", stringify!($t), x);
                    None
                }
            }
        };
        if let None = _result {
            return $d;
        }
        #[allow(unused_mut)]
        let mut $r = _result.unwrap();
    };
}

const REQ_TIMEOUT: u32 = 600;

pub struct Telegram {
    tokio_handle: Handle,
    client: Client<HttpsConnector<HttpConnector>, Body>,
    token: String,
    last_update: i64,
    subscribers: HashMap<i64, Rc<Fn(i64, &mut Telegram, &Update) -> BoxFuture<'static, ()>>>
}

// The Telegram API call implementation
#[allow(dead_code)]
impl Telegram {
    /*
     * Initialize a Telegram instance
     */
    pub fn new(tokio_handle: Handle, token: &str) -> Telegram {
        // Create Hyper client object before anything starts
        let client = Client::configure()
            .connector(HttpsConnector::new(4, &tokio_handle)
                .expect("WTF: Cannot create HTTPS agent"))
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

    pub fn get<'a, 'b>(&'b self, method: &str, params: HashMap<String, Box<ToString>>) -> BoxFuture<'a, Result> {
        Box::new(self.client
            .get(self.uri_for_method_with_params(method, params))
            .and_then(|res| res.body().concat2())
            .chain_err(|| "GET request failed")
            .and_then(parse_body))
    }

    pub fn post<'a, 'b>(&'b self, method: &str, params: HashMap<String, Box<ToString>>) -> BoxFuture<'a, Result> {
        Box::new(self.client
            .request({
                let mut req: Request<Body> = Request::new(Method::Post, self.uri_for_method(method));
                let qs = utils::build_query_string(params);
                req.set_body(qs.clone());
                {
                    let headers = req.headers_mut();
                    headers.set_raw("content-length", format!("{}", qs.len()));
                    headers.set_raw("content-type", "application/x-www-form-urlencoded");
                }
                req
            })
            .and_then(|res| res.body().concat2())
            .chain_err(|| "POST request failed")
            .and_then(parse_body))
    }

    fn next_update<'a>(&'a mut self) -> BoxFuture<'a, (&mut Telegram, Vec<Update>)> {
        info!("Fetching update since {}", self.last_update);
        Box::new(self.get("getUpdates", params!{
            "timeout" => REQ_TIMEOUT,
            "offset" => self.last_update
        }).then(|result| {
            // Ignore any error arising from this operation
            // Just treat it as empty result.
            match result {
                Ok(resp) => Ok::<Result, Error>(resp),
                Err(e) => {
                    error!("Error while fetching new update: {:?}", e);
                    Ok(Result::Nothing)
                }
            }
        }).and_then(move |result| {
            assert_result!(Result::Updates(result), result, Ok((self, vec![])));
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
        }))
    }

    /*
     * Spin up the tail-recursive loop to fetch new messages
     */
    pub fn spin_update_loop<'a>(&'a mut self) -> BoxFuture<'a, ()> {
        Box::new(self.next_update()
            .and_then(move |(new_self, res)| {
                if res.len() != 0 {
                    let subscribers = new_self.get_subscribers();
                    // Dispatch every update to every subscriber
                    for u in res.iter() {
                        for (id, f) in &subscribers {
                            // Executing subscribers will return a Future
                            let fut = f(id.clone(), new_self, u)
                                .map_err(|e| {
                                    warn!("Error suppressed: {:?}", e);
                                    ()
                                });
                            // Add it to the event loop provided by Tokio
                            if let Err(err) =  new_self.tokio_handle.execute(fut) {
                                error!("Failed to schedule subscriber {}, {:?}", id, err);
                            }
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
     * 
     * The subscribers will receive a mutable reference
     * to this Telegram object in order to unsubscribe if needed.
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

fn parse_body(body: Chunk) -> errors::Result<Result> {
    serde_json::from_slice::<Response>(&body)
        .chain_err(|| "Decode failed")
        .and_then(|resp| {
            if !resp.ok {
                return Err("Telegram server error.".into());
            }
                        
            if let Some(result) = resp.result {
                return Ok(result);
            } else {
                return Err("Telegram server error.".into());
            }
        })
}

// Types
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    ok: bool,
    result: Option<Result>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chat {
    pub id: i64
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub message_id: i64,
    pub chat: Chat,
    pub text: Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Result {
    Updates(Vec<Update>),
    User(User),
    Message(Message),
    Nothing
}