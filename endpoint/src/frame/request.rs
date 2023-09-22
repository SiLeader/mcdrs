use crate::handler::WriteOptions;

#[derive(Debug)]
pub(crate) enum MemcachedRequest {
    Set {
        key: String,
        value: String,
        options: WriteOptions,
    },
    Add {
        key: String,
        value: String,
        options: WriteOptions,
    },
    Replace {
        key: String,
        value: String,
        options: WriteOptions,
    },
    Append {
        key: String,
        value: String,
        options: WriteOptions,
    },
    Prepend {
        key: String,
        value: String,
        options: WriteOptions,
    },
    Get {
        key: String,
    },
    Delete {
        key: String,
    },
    Incr {
        key: String,
        diff: i64,
    },
    Decr {
        key: String,
        diff: i64,
    },
    Stats,
    Version,
    Unsupported,
}
