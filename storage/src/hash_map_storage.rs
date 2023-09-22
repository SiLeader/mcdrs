use async_trait::async_trait;
use endpoint::{
    MemcachedError, MemcachedHandler, MemcachedResponse, MemcachedResult, WriteOptions,
};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::RwLock;

struct McdValue {
    value: String,
    flags: u32,
    expire: Duration,
}

impl McdValue {
    fn new(value: String, options: WriteOptions) -> Self {
        Self {
            value,
            flags: options.flags,
            expire: options.expire,
        }
    }

    fn to_options(&self) -> WriteOptions {
        WriteOptions {
            flags: self.flags,
            expire: self.expire,
        }
    }
}

#[derive(Default)]
pub struct HashMapStorage {
    hash_map: RwLock<HashMap<String, McdValue>>,
}

#[async_trait]
impl MemcachedHandler for HashMapStorage {
    async fn set(&self, key: String, value: String, options: WriteOptions) -> MemcachedResult {
        let mut hm = self.hash_map.write().await;
        hm.insert(key, McdValue::new(value, options));
        Ok(MemcachedResponse::Stored)
    }

    async fn add(&self, key: String, value: String, options: WriteOptions) -> MemcachedResult {
        let mut hm = self.hash_map.write().await;
        if hm.contains_key(key.as_str()) {
            return Err(MemcachedError::AlreadyExists);
        }
        hm.insert(key, McdValue::new(value, options));
        Ok(MemcachedResponse::Stored)
    }

    async fn replace(&self, key: String, value: String, options: WriteOptions) -> MemcachedResult {
        let mut hm = self.hash_map.write().await;
        if !hm.contains_key(key.as_str()) {
            return Err(MemcachedError::NotFound);
        }
        hm.insert(key, McdValue::new(value, options));
        Ok(MemcachedResponse::Stored)
    }

    async fn append(&self, key: String, value: String, options: WriteOptions) -> MemcachedResult {
        let mut hm = self.hash_map.write().await;
        let Some(old_value) = hm.get(key.as_str()) else {
            return Err(MemcachedError::NotFound);
        };
        let new_value = format!("{}{value}", old_value.value);

        hm.insert(key, McdValue::new(new_value, options));
        Ok(MemcachedResponse::Stored)
    }

    async fn prepend(&self, key: String, value: String, options: WriteOptions) -> MemcachedResult {
        let mut hm = self.hash_map.write().await;
        let Some(old_value) = hm.get(key.as_str()) else {
            return Err(MemcachedError::NotFound);
        };
        let new_value = format!("{value}{}", old_value.value);

        hm.insert(key, McdValue::new(new_value, options));
        Ok(MemcachedResponse::Stored)
    }

    async fn get(&self, key: String) -> MemcachedResult {
        let hm = self.hash_map.read().await;
        match hm.get(key.as_str()) {
            Some(value) => Ok(MemcachedResponse::Value {
                key,
                flags: value.flags,
                expire: value.expire,
                value: value.value.to_string(),
            }),
            None => Err(MemcachedError::NotFound),
        }
    }

    async fn delete(&self, key: String) -> MemcachedResult {
        let mut hm = self.hash_map.write().await;

        if hm.remove(key.as_str()).is_none() {
            Err(MemcachedError::NotFound)
        } else {
            Ok(MemcachedResponse::Deleted)
        }
    }

    async fn increment(&self, key: String, diff: i64) -> MemcachedResult {
        let mut hm = self.hash_map.write().await;
        let Some(old_value) = hm.get(key.as_str()) else {
            return Err(MemcachedError::NotFound);
        };

        let current = i64::from_str(old_value.value.as_str())
            .map_err(|_| MemcachedError::FailedToParseInteger)?;
        let new = current + diff;

        let new_value = McdValue::new(new.to_string(), old_value.to_options());

        hm.insert(key, new_value);
        Ok(MemcachedResponse::Stored)
    }

    async fn decrement(&self, key: String, diff: i64) -> MemcachedResult {
        let mut hm = self.hash_map.write().await;
        let Some(old_value) = hm.get(key.as_str()) else {
            return Err(MemcachedError::NotFound);
        };

        let current = i64::from_str(old_value.value.as_str())
            .map_err(|_| MemcachedError::FailedToParseInteger)?;
        let new = current - diff;

        let new_value = McdValue::new(new.to_string(), old_value.to_options());

        hm.insert(key, new_value);
        Ok(MemcachedResponse::Stored)
    }

    async fn statistics(&self) -> MemcachedResult {
        Ok(MemcachedResponse::Statistics(HashMap::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_if_absent() {
        let storage = HashMapStorage::default();

        let result = storage.get("key".to_string()).await;

        assert_eq!(result, Err(MemcachedError::NotFound));
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .set("key".to_string(), "value".to_string(), options)
            .await
            .expect("Can set");
        let result = storage.get("key".to_string()).await;

        assert_eq!(
            result,
            Ok(MemcachedResponse::Value {
                key: "key".to_string(),
                flags: 0,
                expire: Duration::from_secs(0),
                value: "value".to_string()
            })
        );
    }

    // add
    #[tokio::test]
    async fn test_add_if_absent() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .add("key".to_string(), "value".to_string(), options)
            .await
            .expect("Can add");
    }

    #[tokio::test]
    async fn test_add_if_exists() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .set("key".to_string(), "value".to_string(), options.clone())
            .await
            .expect("Can set");

        let result = storage
            .add("key".to_string(), "value2".to_string(), options)
            .await;
        assert_eq!(result, Err(MemcachedError::AlreadyExists));
    }

    // replace
    #[tokio::test]
    async fn test_replace_if_absent() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };

        let result = storage
            .replace("key".to_string(), "value".to_string(), options)
            .await;
        assert_eq!(result, Err(MemcachedError::NotFound));
    }

    #[tokio::test]
    async fn test_replace_if_exists() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .set("key".to_string(), "value".to_string(), options.clone())
            .await
            .expect("Can set");

        storage
            .replace("key".to_string(), "value2".to_string(), options)
            .await
            .expect("Can replace");

        let result = storage.get("key".to_string()).await;

        assert_eq!(
            result,
            Ok(MemcachedResponse::Value {
                key: "key".to_string(),
                flags: 0,
                expire: Duration::from_secs(0),
                value: "value2".to_string()
            })
        );
    }

    // append
    #[tokio::test]
    async fn test_append_if_absent() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };

        let result = storage
            .append("key".to_string(), "value".to_string(), options)
            .await;
        assert_eq!(result, Err(MemcachedError::NotFound));
    }

    #[tokio::test]
    async fn test_append_if_exists() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .set("key".to_string(), "value".to_string(), options.clone())
            .await
            .expect("Can set");

        storage
            .append("key".to_string(), "value2".to_string(), options)
            .await
            .expect("Can append");

        let result = storage.get("key".to_string()).await;

        assert_eq!(
            result,
            Ok(MemcachedResponse::Value {
                key: "key".to_string(),
                flags: 0,
                expire: Duration::from_secs(0),
                value: "valuevalue2".to_string()
            })
        );
    }

    // prepend
    #[tokio::test]
    async fn test_prepend_if_absent() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };

        let result = storage
            .prepend("key".to_string(), "value".to_string(), options)
            .await;
        assert_eq!(result, Err(MemcachedError::NotFound));
    }

    #[tokio::test]
    async fn test_prepend_if_exists() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .set("key".to_string(), "value".to_string(), options.clone())
            .await
            .expect("Can set");

        storage
            .prepend("key".to_string(), "value2".to_string(), options)
            .await
            .expect("Can prepend");

        let result = storage.get("key".to_string()).await;

        assert_eq!(
            result,
            Ok(MemcachedResponse::Value {
                key: "key".to_string(),
                flags: 0,
                expire: Duration::from_secs(0),
                value: "value2value".to_string()
            })
        );
    }

    // delete
    #[tokio::test]
    async fn test_delete_if_absent() {
        let storage = HashMapStorage::default();

        let result = storage.delete("key".to_string()).await;
        assert_eq!(result, Err(MemcachedError::NotFound));
    }

    #[tokio::test]
    async fn test_delete_if_exists() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .set("key".to_string(), "value".to_string(), options)
            .await
            .expect("Can set");

        storage.delete("key".to_string()).await.expect("Can delete");

        let result = storage.get("key".to_string()).await;

        assert_eq!(result, Err(MemcachedError::NotFound));
    }

    // increment
    #[tokio::test]
    async fn test_increment_if_absent() {
        let storage = HashMapStorage::default();

        let result = storage.increment("key".to_string(), 5).await;
        assert_eq!(result, Err(MemcachedError::NotFound));
    }

    #[tokio::test]
    async fn test_increment_if_exists() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .set("key".to_string(), "100".to_string(), options.clone())
            .await
            .expect("Can set");

        storage
            .increment("key".to_string(), 5)
            .await
            .expect("Can increment");

        let result = storage.get("key".to_string()).await;

        assert_eq!(
            result,
            Ok(MemcachedResponse::Value {
                key: "key".to_string(),
                flags: 0,
                expire: Duration::from_secs(0),
                value: "105".to_string()
            })
        );
    }

    #[tokio::test]
    async fn test_increment_cannot_parse_as_integer() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .set("key".to_string(), "value".to_string(), options.clone())
            .await
            .expect("Can set");

        let result = storage.increment("key".to_string(), 5).await;

        assert_eq!(result, Err(MemcachedError::FailedToParseInteger));

        let result = storage.get("key".to_string()).await;

        assert_eq!(
            result,
            Ok(MemcachedResponse::Value {
                key: "key".to_string(),
                flags: 0,
                expire: Duration::from_secs(0),
                value: "value".to_string()
            })
        );
    }

    // decrement
    #[tokio::test]
    async fn test_decrement_if_absent() {
        let storage = HashMapStorage::default();

        let result = storage.decrement("key".to_string(), 5).await;
        assert_eq!(result, Err(MemcachedError::NotFound));
    }

    #[tokio::test]
    async fn test_decrement_if_exists() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .set("key".to_string(), "100".to_string(), options.clone())
            .await
            .expect("Can set");

        storage
            .decrement("key".to_string(), 5)
            .await
            .expect("Can decrement");

        let result = storage.get("key".to_string()).await;

        assert_eq!(
            result,
            Ok(MemcachedResponse::Value {
                key: "key".to_string(),
                flags: 0,
                expire: Duration::from_secs(0),
                value: "95".to_string()
            })
        );
    }

    #[tokio::test]
    async fn test_decrement_cannot_parse_as_integer() {
        let storage = HashMapStorage::default();

        let options = WriteOptions {
            flags: 0,
            expire: Duration::from_secs(0),
        };
        storage
            .set("key".to_string(), "value".to_string(), options.clone())
            .await
            .expect("Can set");

        let result = storage.decrement("key".to_string(), 5).await;

        assert_eq!(result, Err(MemcachedError::FailedToParseInteger));

        let result = storage.get("key".to_string()).await;

        assert_eq!(
            result,
            Ok(MemcachedResponse::Value {
                key: "key".to_string(),
                flags: 0,
                expire: Duration::from_secs(0),
                value: "value".to_string()
            })
        );
    }
}
