use log::{debug, warn};
use std::fmt::Write;
use std::num::{IntErrorKind, ParseIntError};
use std::str::FromStr;
use std::time::Duration;
use tokio_util::bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

mod request;
mod response;

use crate::handler::WriteOptions;
pub(crate) use request::*;
pub use response::*;

trait OrElseResult {
    type Error;
    type Item;

    fn or_else_result<F: FnOnce() -> Result<Self::Item, Self::Error>>(
        self,
        f: F,
    ) -> Result<Self::Item, Self::Error>;
}

impl<T> OrElseResult for Option<T> {
    type Error = std::io::Error;
    type Item = Option<T>;

    fn or_else_result<F: FnOnce() -> Result<Self::Item, Self::Error>>(
        self,
        f: F,
    ) -> Result<Self::Item, Self::Error> {
        match self {
            None => f(),
            Some(v) => Ok(Some(v)),
        }
    }
}

trait MapToInteger {
    fn map_to_integer<I: FromStr<Err = ParseIntError>>(
        s: &str,
    ) -> Result<Option<I>, std::io::Error> {
        debug!("map_to_integer: {s}");
        match I::from_str(s) {
            Ok(v) => Ok(Some(v)),
            Err(e) => {
                if *e.kind() == IntErrorKind::InvalidDigit {
                    Ok(None)
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                }
            }
        }
    }
    fn map_to_i64(self) -> Result<Option<i64>, std::io::Error>;
    fn map_to_u64(self) -> Result<Option<u64>, std::io::Error>;
    fn map_to_u32(self) -> Result<Option<u32>, std::io::Error>;
}

impl MapToInteger for Option<String> {
    fn map_to_i64(self) -> Result<Option<i64>, std::io::Error> {
        match self {
            None => Ok(None),
            Some(s) => Self::map_to_integer(s.as_str()),
        }
    }

    fn map_to_u64(self) -> Result<Option<u64>, std::io::Error> {
        match self {
            None => Ok(None),
            Some(s) => Self::map_to_integer(s.as_str()),
        }
    }

    fn map_to_u32(self) -> Result<Option<u32>, std::io::Error> {
        match self {
            None => Ok(None),
            Some(s) => Self::map_to_integer(s.as_str()),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct MemcachedCodec {
    command: Option<String>,
    key: Option<String>,

    // write
    flags: Option<u32>,
    expire: Option<u64>,
    number_of_bytes: Option<u64>,

    // diff
    diff: Option<i64>,
}

impl MemcachedCodec {
    fn encode_data(
        item: Result<MemcachedResponse, MemcachedError>,
        dst: &mut BytesMut,
    ) -> Result<(), std::fmt::Error> {
        match item {
            Ok(res) => match res {
                MemcachedResponse::Stored => dst.write_str("STORED\r\n"),
                MemcachedResponse::Deleted => dst.write_str("DELETED\r\n"),
                MemcachedResponse::NoValue => dst.write_str("END\r\n"),
                MemcachedResponse::Value {
                    key,
                    value,
                    expire,
                    flags,
                } => {
                    write!(
                        dst,
                        "VALUE {key} {flags} {}\r\n{value}\r\nEND\r\n",
                        expire.as_secs()
                    )
                }
                MemcachedResponse::Statistics(stats) => {
                    for (key, value) in stats {
                        let msg = format!("STAT {key} {value}\r\n");
                        dst.write_str(msg.as_str())?;
                    }
                    Ok(())
                }
                MemcachedResponse::Version(version) => {
                    let msg = format!("VERSION {version}\r\n");
                    dst.write_str(msg.as_str())
                }
            },
            Err(err) => match err {
                MemcachedError::NoExistenceCommand => dst.write_str("ERROR\r\n"),
                MemcachedError::Client(message) => {
                    let msg = format!("CLIENT_ERROR {message}\r\n");
                    dst.write_str(msg.as_str())
                }
                MemcachedError::Server(message) => {
                    let msg = format!("SERVER_ERROR {message}\r\n");
                    dst.write_str(msg.as_str())
                }
                MemcachedError::NotFound => dst.write_str("CLIENT_ERROR Not found\r\n"),
                MemcachedError::AlreadyExists => dst.write_str("CLIENT_ERROR Already exists\r\n"),
                MemcachedError::FailedToParseInteger => {
                    dst.write_str("CLIENT_ERROR Failed to parse integer\r\n")
                }
            },
        }
    }
}

impl Encoder<Result<MemcachedResponse, MemcachedError>> for MemcachedCodec {
    type Error = std::io::Error;

    fn encode(
        &mut self,
        item: Result<MemcachedResponse, MemcachedError>,
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        Self::encode_data(item, dst)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
    }
}

trait PositionSpace {
    fn position_space(&self) -> Option<usize>;
    fn substring_spaced(&mut self) -> std::io::Result<Option<String>>;
    fn position_newline(&self) -> Option<usize>;
    fn substring_newlined(&mut self) -> std::io::Result<Option<String>>;
}

impl PositionSpace for BytesMut {
    fn position_space(&self) -> Option<usize> {
        self.iter().position(|v| *v == b' ')
    }

    fn substring_spaced(&mut self) -> std::io::Result<Option<String>> {
        let index_of_space = self.position_space();
        let data = match index_of_space {
            None => return Ok(None),
            Some(index) => self.split_to(index),
        };
        self.advance(1);

        let value = String::from_utf8(data.to_vec())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(Some(value.trim_end().to_string()))
    }

    fn position_newline(&self) -> Option<usize> {
        self.iter().position(|v| *v == b'\n')
    }

    fn substring_newlined(&mut self) -> std::io::Result<Option<String>> {
        let index_of_space = self.position_newline();
        let data = match index_of_space {
            None => return Ok(None),
            Some(index) => self.split_to(index),
        };
        self.advance(1);

        let value = String::from_utf8(data.to_vec())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(Some(value.trim_end().to_string()))
    }
}

impl MemcachedCodec {
    fn decode_key_request(&mut self, src: &mut BytesMut) -> std::io::Result<Option<String>> {
        self.key.clone().or_else_result(|| src.substring_newlined())
    }

    fn decode_diff_request(
        &mut self,
        src: &mut BytesMut,
    ) -> std::io::Result<Option<(String, i64)>> {
        let Some(key) = self.key.clone().or_else_result(|| src.substring_spaced())? else {
            return Ok(None);
        };
        self.key = Some(key.to_string());

        let Some(diff) = self
            .diff
            .or_else_result(|| src.substring_newlined()?.map_to_i64())?
        else {
            return Ok(None);
        };
        self.diff = Some(diff);

        Ok(Some((key, diff)))
    }

    fn decode_write_request(
        &mut self,
        src: &mut BytesMut,
    ) -> std::io::Result<Option<(String, WriteOptions, String)>> {
        let Some(key) = self.key.clone().or_else_result(|| src.substring_spaced())? else {
            return Ok(None);
        };
        self.key = Some(key.to_string());

        let options = {
            let Some(flags) = self
                .flags
                .or_else_result(|| src.substring_spaced()?.map_to_u32())?
            else {
                return Ok(None);
            };
            self.flags = Some(flags);

            let Some(expire_in_seconds) = self
                .expire
                .or_else_result(|| src.substring_spaced()?.map_to_u64())?
            else {
                return Ok(None);
            };
            self.expire = Some(expire_in_seconds);

            WriteOptions {
                flags,
                expire: Duration::from_secs(expire_in_seconds),
            }
        };

        let Some(number_of_bytes) = self
            .number_of_bytes
            .or_else_result(|| src.substring_newlined()?.map_to_u64())?
        else {
            return Ok(None);
        };
        self.number_of_bytes = Some(number_of_bytes);
        let number_of_bytes = number_of_bytes as usize;

        debug!("length: src: {}, count: {number_of_bytes}", src.len());
        if src.len() < number_of_bytes {
            return Ok(None);
        }
        let bytes = src.split_to(number_of_bytes);
        let value = String::from_utf8(bytes.to_vec())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(Some((key, options, value)))
    }

    fn decode_impl(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<Option<MemcachedRequest>, std::io::Error> {
        debug!("decode: {}", src.len());
        let Some(cmd) = self
            .command
            .clone()
            .or_else_result(|| src.substring_spaced())?
        else {
            return Ok(None);
        };
        self.command = Some(cmd.to_string());

        match cmd.trim() {
            "set" => match self.decode_write_request(src) {
                Ok(v) => match v {
                    Some((key, options, value)) => Ok(Some(MemcachedRequest::Set {
                        key,
                        options,
                        value,
                    })),
                    None => Ok(None),
                },
                Err(e) => Err(e),
            },
            "add" => match self.decode_write_request(src) {
                Ok(v) => match v {
                    Some((key, options, value)) => Ok(Some(MemcachedRequest::Add {
                        key,
                        options,
                        value,
                    })),
                    None => Ok(None),
                },
                Err(e) => Err(e),
            },
            "replace" => match self.decode_write_request(src) {
                Ok(v) => match v {
                    Some((key, options, value)) => Ok(Some(MemcachedRequest::Replace {
                        key,
                        options,
                        value,
                    })),
                    None => Ok(None),
                },
                Err(e) => Err(e),
            },
            "append" => match self.decode_write_request(src) {
                Ok(v) => match v {
                    Some((key, options, value)) => Ok(Some(MemcachedRequest::Append {
                        key,
                        options,
                        value,
                    })),
                    None => Ok(None),
                },
                Err(e) => Err(e),
            },
            "prepend" => match self.decode_write_request(src) {
                Ok(v) => match v {
                    Some((key, options, value)) => Ok(Some(MemcachedRequest::Prepend {
                        key,
                        options,
                        value,
                    })),
                    None => Ok(None),
                },
                Err(e) => Err(e),
            },
            "get" => match self.decode_key_request(src) {
                Ok(v) => match v {
                    Some(key) => Ok(Some(MemcachedRequest::Get { key })),
                    None => Ok(None),
                },
                Err(e) => Err(e),
            },
            "delete" => match self.decode_key_request(src) {
                Ok(v) => match v {
                    Some(key) => Ok(Some(MemcachedRequest::Delete { key })),
                    None => Ok(None),
                },
                Err(e) => Err(e),
            },
            "incr" => match self.decode_diff_request(src) {
                Ok(v) => match v {
                    Some((key, diff)) => Ok(Some(MemcachedRequest::Incr { key, diff })),
                    None => Ok(None),
                },
                Err(e) => Err(e),
            },
            "decr" => match self.decode_diff_request(src) {
                Ok(v) => match v {
                    Some((key, diff)) => Ok(Some(MemcachedRequest::Decr { key, diff })),
                    None => Ok(None),
                },
                Err(e) => Err(e),
            },
            "stats" => Ok(Some(MemcachedRequest::Stats)),
            "version" => Ok(Some(MemcachedRequest::Version)),
            c => {
                warn!("Unsupported command: {c}");
                Ok(Some(MemcachedRequest::Unsupported))
            }
        }
    }
}

impl Decoder for MemcachedCodec {
    type Item = MemcachedRequest;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let value = self.decode_impl(src)?;
        if value.is_some() {
            self.command = None;
            self.key = None;
            self.flags = None;
            self.expire = None;
            self.number_of_bytes = None;
            self.diff = None;
        }
        Ok(value)
    }
}
