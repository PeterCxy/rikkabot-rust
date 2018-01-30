use futures::Future;
use futures_cpupool::CpuPool;
use serde_json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::str::FromStr;
use std::ops::Deref;
use std::rc::Rc;
use utils;
use utils::BoxFuture;

const SAVE_THRESHOLD: i32 = 10;

/*
 * A State to be used in a single-threaded context
 * DO NOT use this with multi-threading
 */
pub struct State {
    pool: Rc<CpuPool>,
    state_file: String,
    state: RefCell<HashMap<String, String>>,
    diff: RefCell<i32>
}

impl State {
    pub fn new(pool: Rc<CpuPool>, state_file: String) -> State {
        State {
            pool,
            state_file,
            state: RefCell::new(HashMap::new()),
            diff: RefCell::new(0)
        }
    }

    pub fn put(&self, key: &str, value: &ToString) {
        *self.diff.borrow_mut() += 1;
        self.state.borrow_mut().insert(key.to_string(), value.to_string());
    }

    pub fn get<T>(&self, key: &str) -> Option<T>
        where T: FromStr {
            self.state.borrow().get(key)
                .and_then(|value| FromStr::from_str(value).ok())
    }

    pub fn keys(&self) -> Vec<String> {
        self.state.borrow().keys().map(|k| k.to_string()).collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self.state.borrow().deref()).unwrap()
    }

    // Load state from state_file
    // Takes ownership of self. Will give it back in the future.
    pub fn load<'a>(self) -> BoxFuture<'a, State> {
        Box::new(utils::read_file_str_async(self.pool.deref(), self.state_file.clone())
            .then(move |res| {
                match res {
                    Ok(s) => *self.state.borrow_mut() = serde_json::from_str(&s).unwrap_or_else(|_| {
                        warn!("Failed to decode {}. Starting fresh.", self.state_file);
                        HashMap::new()
                    }),
                    Err(_) => warn!("{} does not exist. Starting fresh.", self.state_file)
                }
                Ok(self)
            }))
    }

    pub fn save<'a>(&self) -> BoxFuture<'a, ()> {
        info!("Saving state to {}", self.state_file);
        *self.diff.borrow_mut() = 0;
        utils::write_file_str_async(self.pool.deref(), self.state_file.clone(), self.to_json())
    }

    pub fn save_if_needed<'a>(&self) -> BoxFuture<'a, ()> {
        if *self.diff.borrow() >= SAVE_THRESHOLD {
            self.save()
        } else {
            utils::return_empty()
        }
    }
}