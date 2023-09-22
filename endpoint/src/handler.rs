use async_trait::async_trait;
use std::time::Duration;

pub use crate::frame::{MemcachedError, MemcachedResponse};

#[derive(Debug, Clone)]
pub struct WriteOptions {
    pub flags: u32,
    pub expire: Duration,
}

pub type MemcachedResult = Result<MemcachedResponse, MemcachedError>;

#[async_trait]
pub trait MemcachedHandler: Send + Sync {
    async fn set(&self, key: String, value: String, options: WriteOptions) -> MemcachedResult;
    async fn add(&self, key: String, value: String, options: WriteOptions) -> MemcachedResult;
    async fn replace(&self, key: String, value: String, options: WriteOptions) -> MemcachedResult;
    async fn append(&self, key: String, value: String, options: WriteOptions) -> MemcachedResult;
    async fn prepend(&self, key: String, value: String, options: WriteOptions) -> MemcachedResult;
    async fn get(&self, key: String) -> MemcachedResult;
    async fn delete(&self, key: String) -> MemcachedResult;
    async fn increment(&self, key: String, diff: i64) -> MemcachedResult;
    async fn decrement(&self, key: String, diff: i64) -> MemcachedResult;
    async fn statistics(&self) -> MemcachedResult;
}
