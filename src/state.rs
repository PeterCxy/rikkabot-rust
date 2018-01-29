use serde_json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::str::FromStr;
use std::ops::Deref;

/*
 * A State to be used in a single-threaded context
 * DO NOT use this with multi-threading
 */
pub struct State {
    state: RefCell<HashMap<String, String>>
}

impl State {
    pub fn new() -> State {
        State {
            state: RefCell::new(HashMap::new())
        }
    }

    pub fn put(&self, key: &str, value: &ToString) {
        self.state.borrow_mut().insert(key.to_string(), value.to_string());
    }

    pub fn get<T>(&self, key: &str) -> Option<T>
        where T: FromStr {
            self.state.borrow().get(key)
                .and_then(|value| FromStr::from_str(value).ok())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self.state.borrow().deref()).unwrap()
    }
}