// Protocol module - RESP parser and serializer

use bytes::{Buf, BytesMut};
use std::io::Cursor;
use thiserror::Error;

pub mod parser;
pub mod serializer;

pub use parser::RespParser;
pub use serializer::RespSerializer;

/// RESP (REdis Serialization Protocol) value types
#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    /// Simple string: +OK\r\n
    SimpleString(String),
    /// Error: -ERR unknown command\r\n
    Error(String),
    /// Integer: :1000\r\n
    Integer(i64),
    /// Bulk string: $6\r\nfoobar\r\n (None for null bulk string)
    BulkString(Option<Vec<u8>>),
    /// Array: *2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n (None for null array)
    Array(Option<Vec<RespValue>>),
    /// Null (RESP3)
    Null,
    /// Boolean (RESP3)
    Boolean(bool),
    /// Double (RESP3)
    Double(f64),
}

impl RespValue {
    /// Convert to a simple string if possible
    pub fn as_simple_string(&self) -> Option<&str> {
        match self {
            RespValue::SimpleString(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to bulk string if possible
    pub fn as_bulk_string(&self) -> Option<&[u8]> {
        match self {
            RespValue::BulkString(Some(s)) => Some(s),
            _ => None,
        }
    }

    /// Convert to integer if possible
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            RespValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Convert to array if possible
    pub fn as_array(&self) -> Option<&[RespValue]> {
        match self {
            RespValue::Array(Some(arr)) => Some(arr),
            _ => None,
        }
    }

    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(
            self,
            RespValue::Null | RespValue::BulkString(None) | RespValue::Array(None)
        )
    }
}

#[derive(Error, Debug)]
pub enum RespError {
    #[error("Incomplete data")]
    Incomplete,

    #[error("Invalid protocol: {0}")]
    InvalidProtocol(String),

    #[error("Invalid integer: {0}")]
    InvalidInteger(String),

    #[error("Invalid bulk string length")]
    InvalidBulkStringLength,

    #[error("Invalid array length")]
    InvalidArrayLength,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("UTF-8 str error: {0}")]
    Utf8Str(#[from] std::str::Utf8Error),
}

pub type Result<T> = std::result::Result<T, RespError>;

/// Helper function to find CRLF in buffer
pub(crate) fn find_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\r\n")
}

/// Helper function to read a line from cursor
pub(crate) fn read_line<'a>(cursor: &mut Cursor<&'a [u8]>) -> Result<&'a [u8]> {
    let start = cursor.position() as usize;
    let slice = &cursor.get_ref()[start..];

    let end = find_crlf(slice).ok_or(RespError::Incomplete)?;

    cursor.set_position((start + end + 2) as u64);
    Ok(&slice[..end])
}

/// Helper function to parse integer from bytes
pub(crate) fn parse_integer(buf: &[u8]) -> Result<i64> {
    let s = std::str::from_utf8(buf)
        .map_err(|_| RespError::InvalidInteger("Invalid UTF-8".to_string()))?;
    s.parse::<i64>()
        .map_err(|_| RespError::InvalidInteger(s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_crlf() {
        assert_eq!(find_crlf(b"hello\r\n"), Some(5));
        assert_eq!(find_crlf(b"hello"), None);
        assert_eq!(find_crlf(b"\r\n"), Some(0));
    }

    #[test]
    fn test_parse_integer() {
        assert_eq!(parse_integer(b"123").unwrap(), 123);
        assert_eq!(parse_integer(b"-456").unwrap(), -456);
        assert_eq!(parse_integer(b"0").unwrap(), 0);
        assert!(parse_integer(b"abc").is_err());
    }

    #[test]
    fn test_resp_value_conversions() {
        let val = RespValue::SimpleString("OK".to_string());
        assert_eq!(val.as_simple_string(), Some("OK"));
        assert_eq!(val.as_integer(), None);

        let val = RespValue::Integer(42);
        assert_eq!(val.as_integer(), Some(42));

        let val = RespValue::BulkString(None);
        assert!(val.is_null());
    }
}
