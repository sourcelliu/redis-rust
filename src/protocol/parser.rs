// RESP Protocol Parser

use super::{parse_integer, read_line, RespError, RespValue, Result};
use bytes::{Buf, BytesMut};
use std::io::Cursor;

pub struct RespParser;

impl RespParser {
    /// Parse a complete RESP value from a byte buffer
    pub fn parse(buf: &[u8]) -> Result<RespValue> {
        let mut cursor = Cursor::new(buf);
        Self::parse_value(&mut cursor)
    }

    /// Parse RESP value from cursor
    fn parse_value(cursor: &mut Cursor<&[u8]>) -> Result<RespValue> {
        if cursor.position() >= cursor.get_ref().len() as u64 {
            return Err(RespError::Incomplete);
        }

        let type_byte = cursor.get_ref()[cursor.position() as usize];
        cursor.set_position(cursor.position() + 1);

        match type_byte {
            b'+' => Self::parse_simple_string(cursor),
            b'-' => Self::parse_error(cursor),
            b':' => Self::parse_integer(cursor),
            b'$' => Self::parse_bulk_string(cursor),
            b'*' => Self::parse_array(cursor),
            b'_' => {
                // RESP3 null
                let _ = read_line(cursor)?;
                Ok(RespValue::Null)
            }
            b'#' => {
                // RESP3 boolean
                let line = read_line(cursor)?;
                match line {
                    b"t" => Ok(RespValue::Boolean(true)),
                    b"f" => Ok(RespValue::Boolean(false)),
                    _ => Err(RespError::InvalidProtocol(
                        "Invalid boolean value".to_string(),
                    )),
                }
            }
            b',' => {
                // RESP3 double
                let line = read_line(cursor)?;
                let s = std::str::from_utf8(line)?;
                let val = s
                    .parse::<f64>()
                    .map_err(|_| RespError::InvalidProtocol("Invalid double".to_string()))?;
                Ok(RespValue::Double(val))
            }
            _ => Err(RespError::InvalidProtocol(format!(
                "Unknown type byte: {}",
                type_byte as char
            ))),
        }
    }

    /// Parse simple string: +OK\r\n
    fn parse_simple_string(cursor: &mut Cursor<&[u8]>) -> Result<RespValue> {
        let line = read_line(cursor)?;
        let s = String::from_utf8(line.to_vec())?;
        Ok(RespValue::SimpleString(s))
    }

    /// Parse error: -ERR message\r\n
    fn parse_error(cursor: &mut Cursor<&[u8]>) -> Result<RespValue> {
        let line = read_line(cursor)?;
        let s = String::from_utf8(line.to_vec())?;
        Ok(RespValue::Error(s))
    }

    /// Parse integer: :1000\r\n
    fn parse_integer(cursor: &mut Cursor<&[u8]>) -> Result<RespValue> {
        let line = read_line(cursor)?;
        let i = parse_integer(line)?;
        Ok(RespValue::Integer(i))
    }

    /// Parse bulk string: $6\r\nfoobar\r\n or $-1\r\n (null)
    fn parse_bulk_string(cursor: &mut Cursor<&[u8]>) -> Result<RespValue> {
        let line = read_line(cursor)?;
        let len = parse_integer(line)?;

        if len == -1 {
            return Ok(RespValue::BulkString(None));
        }

        if len < -1 {
            return Err(RespError::InvalidBulkStringLength);
        }

        let len = len as usize;
        let start = cursor.position() as usize;
        let end = start + len;

        if end + 2 > cursor.get_ref().len() {
            return Err(RespError::Incomplete);
        }

        let data = cursor.get_ref()[start..end].to_vec();
        cursor.set_position((end + 2) as u64); // Skip data and \r\n

        // Verify CRLF
        if cursor.get_ref()[end..end + 2] != *b"\r\n" {
            return Err(RespError::InvalidProtocol(
                "Missing CRLF after bulk string".to_string(),
            ));
        }

        Ok(RespValue::BulkString(Some(data)))
    }

    /// Parse array: *2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n or *-1\r\n (null)
    fn parse_array(cursor: &mut Cursor<&[u8]>) -> Result<RespValue> {
        let line = read_line(cursor)?;
        let len = parse_integer(line)?;

        if len == -1 {
            return Ok(RespValue::Array(None));
        }

        if len < -1 {
            return Err(RespError::InvalidArrayLength);
        }

        let len = len as usize;
        let mut arr = Vec::with_capacity(len);

        for _ in 0..len {
            let value = Self::parse_value(cursor)?;
            arr.push(value);
        }

        Ok(RespValue::Array(Some(arr)))
    }

    /// Check if buffer contains a complete RESP value
    pub fn check_complete(buf: &BytesMut) -> Result<Option<usize>> {
        let mut cursor = Cursor::new(&buf[..]);
        match Self::parse_value(&mut cursor) {
            Ok(_) => Ok(Some(cursor.position() as usize)),
            Err(RespError::Incomplete) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_string() {
        let result = RespParser::parse(b"+OK\r\n").unwrap();
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
    }

    #[test]
    fn test_parse_error() {
        let result = RespParser::parse(b"-ERR unknown command\r\n").unwrap();
        assert_eq!(result, RespValue::Error("ERR unknown command".to_string()));
    }

    #[test]
    fn test_parse_integer() {
        let result = RespParser::parse(b":1000\r\n").unwrap();
        assert_eq!(result, RespValue::Integer(1000));

        let result = RespParser::parse(b":-456\r\n").unwrap();
        assert_eq!(result, RespValue::Integer(-456));
    }

    #[test]
    fn test_parse_bulk_string() {
        let result = RespParser::parse(b"$6\r\nfoobar\r\n").unwrap();
        assert_eq!(
            result,
            RespValue::BulkString(Some(b"foobar".to_vec()))
        );

        // Null bulk string
        let result = RespParser::parse(b"$-1\r\n").unwrap();
        assert_eq!(result, RespValue::BulkString(None));

        // Empty bulk string
        let result = RespParser::parse(b"$0\r\n\r\n").unwrap();
        assert_eq!(result, RespValue::BulkString(Some(vec![])));
    }

    #[test]
    fn test_parse_array() {
        let result = RespParser::parse(b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n").unwrap();
        assert_eq!(
            result,
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"foo".to_vec())),
                RespValue::BulkString(Some(b"bar".to_vec()))
            ]))
        );

        // Null array
        let result = RespParser::parse(b"*-1\r\n").unwrap();
        assert_eq!(result, RespValue::Array(None));

        // Empty array
        let result = RespParser::parse(b"*0\r\n").unwrap();
        assert_eq!(result, RespValue::Array(Some(vec![])));
    }

    #[test]
    fn test_parse_nested_array() {
        let result = RespParser::parse(b"*2\r\n*2\r\n:1\r\n:2\r\n*1\r\n:3\r\n").unwrap();
        assert_eq!(
            result,
            RespValue::Array(Some(vec![
                RespValue::Array(Some(vec![
                    RespValue::Integer(1),
                    RespValue::Integer(2)
                ])),
                RespValue::Array(Some(vec![RespValue::Integer(3)]))
            ]))
        );
    }

    #[test]
    fn test_parse_mixed_array() {
        let result = RespParser::parse(b"*3\r\n+OK\r\n:42\r\n$5\r\nhello\r\n").unwrap();
        assert_eq!(
            result,
            RespValue::Array(Some(vec![
                RespValue::SimpleString("OK".to_string()),
                RespValue::Integer(42),
                RespValue::BulkString(Some(b"hello".to_vec()))
            ]))
        );
    }

    #[test]
    fn test_parse_binary_safe() {
        let data = b"$7\r\n\x00\x01\x02\xff\xfe\xfd\x03\r\n";
        let result = RespParser::parse(data).unwrap();
        assert_eq!(
            result,
            RespValue::BulkString(Some(vec![0x00, 0x01, 0x02, 0xff, 0xfe, 0xfd, 0x03]))
        );
    }

    #[test]
    fn test_parse_incomplete() {
        let result = RespParser::parse(b"+OK");
        assert!(matches!(result, Err(RespError::Incomplete)));

        let result = RespParser::parse(b"$6\r\nfoo");
        assert!(matches!(result, Err(RespError::Incomplete)));
    }

    #[test]
    fn test_parse_invalid() {
        let result = RespParser::parse(b"?invalid\r\n");
        assert!(matches!(result, Err(RespError::InvalidProtocol(_))));
    }

    #[test]
    fn test_resp3_null() {
        let result = RespParser::parse(b"_\r\n").unwrap();
        assert_eq!(result, RespValue::Null);
    }

    #[test]
    fn test_resp3_boolean() {
        let result = RespParser::parse(b"#t\r\n").unwrap();
        assert_eq!(result, RespValue::Boolean(true));

        let result = RespParser::parse(b"#f\r\n").unwrap();
        assert_eq!(result, RespValue::Boolean(false));
    }

    #[test]
    fn test_resp3_double() {
        let result = RespParser::parse(b",3.14159\r\n").unwrap();
        assert_eq!(result, RespValue::Double(3.14159));

        let result = RespParser::parse(b",-0.5\r\n").unwrap();
        assert_eq!(result, RespValue::Double(-0.5));
    }

    #[test]
    fn test_check_complete() {
        let mut buf = BytesMut::from(&b"+OK\r\n"[..]);
        let len = RespParser::check_complete(&buf).unwrap();
        assert_eq!(len, Some(5));

        let mut buf = BytesMut::from(&b"+OK"[..]);
        let len = RespParser::check_complete(&buf).unwrap();
        assert_eq!(len, None);
    }
}
