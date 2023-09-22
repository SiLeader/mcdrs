use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Eq, PartialEq)]
pub enum MemcachedError {
    NoExistenceCommand,
    NotFound,
    AlreadyExists,
    FailedToParseInteger,
    Client(String),
    Server(String),
}

#[derive(Debug, Eq, PartialEq)]
pub enum MemcachedResponse {
    Stored,
    Deleted,
    NoValue,
    Value {
        key: String,
        flags: u32,
        expire: Duration,
        value: String,
    },
    Statistics(HashMap<String, String>),
    Version(String),
}
