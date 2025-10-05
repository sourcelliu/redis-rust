// RESP Protocol Serializer

use super::{RespValue, Result};
use bytes::{BufMut, BytesMut};

pub struct RespSerializer;

impl RespSerializer {
    /// Serialize a RESP value to bytes
    pub fn serialize(value: &RespValue) -> Vec<u8> {
        let mut buf = BytesMut::new();
        Self::write_value(&mut buf, value);
        buf.to_vec()
    }

    /// Write RESP value to buffer
    fn write_value(buf: &mut BytesMut, value: &RespValue) {
        match value {
            RespValue::SimpleString(s) => {
                buf.put_u8(b'+');
                buf.put_slice(s.as_bytes());
                buf.put_slice(b"\r\n");
            }
            RespValue::Error(e) => {
                buf.put_u8(b'-');
                buf.put_slice(e.as_bytes());
                buf.put_slice(b"\r\n");
            }
            RespValue::Integer(i) => {
                buf.put_u8(b':');
                buf.put_slice(i.to_string().as_bytes());
                buf.put_slice(b"\r\n");
            }
            RespValue::BulkString(opt) => match opt {
                None => {
                    buf.put_slice(b"$-1\r\n");
                }
                Some(data) => {
                    buf.put_u8(b'$');
                    buf.put_slice(data.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    buf.put_slice(data);
                    buf.put_slice(b"\r\n");
                }
            },
            RespValue::Array(opt) => match opt {
                None => {
                    buf.put_slice(b"*-1\r\n");
                }
                Some(arr) => {
                    buf.put_u8(b'*');
                    buf.put_slice(arr.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    for item in arr {
                        Self::write_value(buf, item);
                    }
                }
            },
            RespValue::Null => {
                buf.put_slice(b"_\r\n");
            }
            RespValue::Boolean(b) => {
                buf.put_u8(b'#');
                buf.put_u8(if *b { b't' } else { b'f' });
                buf.put_slice(b"\r\n");
            }
            RespValue::Double(d) => {
                buf.put_u8(b',');
                buf.put_slice(d.to_string().as_bytes());
                buf.put_slice(b"\r\n");
            }
        }
    }

    /// Convenience method to create OK response
    pub fn ok() -> Vec<u8> {
        Self::serialize(&RespValue::SimpleString("OK".to_string()))
    }

    /// Convenience method to create error response
    pub fn error(msg: &str) -> Vec<u8> {
        Self::serialize(&RespValue::Error(msg.to_string()))
    }

    /// Convenience method to create null bulk string
    pub fn null_bulk_string() -> Vec<u8> {
        Self::serialize(&RespValue::BulkString(None))
    }

    /// Convenience method to create null array
    pub fn null_array() -> Vec<u8> {
        Self::serialize(&RespValue::Array(None))
    }

    /// Convenience method to create bulk string from bytes
    pub fn bulk_string(data: &[u8]) -> Vec<u8> {
        Self::serialize(&RespValue::BulkString(Some(data.to_vec())))
    }

    /// Convenience method to create integer response
    pub fn integer(i: i64) -> Vec<u8> {
        Self::serialize(&RespValue::Integer(i))
    }

    /// Convenience method to create array response
    pub fn array(items: Vec<RespValue>) -> Vec<u8> {
        Self::serialize(&RespValue::Array(Some(items)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_simple_string() {
        let result = RespSerializer::serialize(&RespValue::SimpleString("OK".to_string()));
        assert_eq!(result, b"+OK\r\n");
    }

    #[test]
    fn test_serialize_error() {
        let result = RespSerializer::serialize(&RespValue::Error("ERR unknown".to_string()));
        assert_eq!(result, b"-ERR unknown\r\n");
    }

    #[test]
    fn test_serialize_integer() {
        let result = RespSerializer::serialize(&RespValue::Integer(1000));
        assert_eq!(result, b":1000\r\n");

        let result = RespSerializer::serialize(&RespValue::Integer(-42));
        assert_eq!(result, b":-42\r\n");
    }

    #[test]
    fn test_serialize_bulk_string() {
        let result = RespSerializer::serialize(&RespValue::BulkString(Some(b"foobar".to_vec())));
        assert_eq!(result, b"$6\r\nfoobar\r\n");

        // Null bulk string
        let result = RespSerializer::serialize(&RespValue::BulkString(None));
        assert_eq!(result, b"$-1\r\n");

        // Empty bulk string
        let result = RespSerializer::serialize(&RespValue::BulkString(Some(vec![])));
        assert_eq!(result, b"$0\r\n\r\n");
    }

    #[test]
    fn test_serialize_array() {
        let result = RespSerializer::serialize(&RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"foo".to_vec())),
            RespValue::BulkString(Some(b"bar".to_vec())),
        ])));
        assert_eq!(result, b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n");

        // Null array
        let result = RespSerializer::serialize(&RespValue::Array(None));
        assert_eq!(result, b"*-1\r\n");

        // Empty array
        let result = RespSerializer::serialize(&RespValue::Array(Some(vec![])));
        assert_eq!(result, b"*0\r\n");
    }

    #[test]
    fn test_serialize_nested_array() {
        let result = RespSerializer::serialize(&RespValue::Array(Some(vec![
            RespValue::Array(Some(vec![
                RespValue::Integer(1),
                RespValue::Integer(2),
            ])),
            RespValue::Array(Some(vec![RespValue::Integer(3)])),
        ])));
        assert_eq!(result, b"*2\r\n*2\r\n:1\r\n:2\r\n*1\r\n:3\r\n");
    }

    #[test]
    fn test_serialize_binary_safe() {
        let data = vec![0x00, 0x01, 0x02, 0xff, 0xfe, 0xfd, 0x03];
        let result = RespSerializer::serialize(&RespValue::BulkString(Some(data.clone())));
        assert_eq!(result[0], b'$');
        assert_eq!(result[1], b'7');
        assert_eq!(&result[4..11], &data[..]);
    }

    #[test]
    fn test_serialize_resp3_null() {
        let result = RespSerializer::serialize(&RespValue::Null);
        assert_eq!(result, b"_\r\n");
    }

    #[test]
    fn test_serialize_resp3_boolean() {
        let result = RespSerializer::serialize(&RespValue::Boolean(true));
        assert_eq!(result, b"#t\r\n");

        let result = RespSerializer::serialize(&RespValue::Boolean(false));
        assert_eq!(result, b"#f\r\n");
    }

    #[test]
    fn test_serialize_resp3_double() {
        let result = RespSerializer::serialize(&RespValue::Double(3.14159));
        assert_eq!(result, b",3.14159\r\n");
    }

    #[test]
    fn test_convenience_methods() {
        assert_eq!(RespSerializer::ok(), b"+OK\r\n");
        assert_eq!(RespSerializer::error("test"), b"-test\r\n");
        assert_eq!(RespSerializer::null_bulk_string(), b"$-1\r\n");
        assert_eq!(RespSerializer::null_array(), b"*-1\r\n");
        assert_eq!(RespSerializer::bulk_string(b"test"), b"$4\r\ntest\r\n");
        assert_eq!(RespSerializer::integer(42), b":42\r\n");
    }

    #[test]
    fn test_roundtrip() {
        use super::super::parser::RespParser;

        let values = vec![
            RespValue::SimpleString("OK".to_string()),
            RespValue::Error("ERR".to_string()),
            RespValue::Integer(42),
            RespValue::BulkString(Some(b"test".to_vec())),
            RespValue::BulkString(None),
            RespValue::Array(Some(vec![
                RespValue::Integer(1),
                RespValue::BulkString(Some(b"foo".to_vec())),
            ])),
            RespValue::Null,
            RespValue::Boolean(true),
            RespValue::Double(3.14),
        ];

        for value in values {
            let serialized = RespSerializer::serialize(&value);
            let parsed = RespParser::parse(&serialized).unwrap();
            assert_eq!(parsed, value);
        }
    }
}
