// E2E tests for String commands
// Translated from Redis test suite: tests/unit/type/string.tcl

mod common;

use redis::RedisResult;

#[tokio::test]
#[ignore] // Remove this once the server is implemented
async fn test_set_and_get() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    // SET and GET basic test
    let _: () = redis::cmd("SET")
        .arg("mykey")
        .arg("Hello")
        .query_async(&mut conn)
        .await
        .unwrap();

    let value: String = redis::cmd("GET")
        .arg("mykey")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(value, "Hello");

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_append() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    // Set initial value
    let _: () = redis::cmd("SET")
        .arg("mykey")
        .arg("Hello")
        .query_async(&mut conn)
        .await
        .unwrap();

    // Append
    let len: i32 = redis::cmd("APPEND")
        .arg("mykey")
        .arg(" World")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(len, 11);

    let value: String = redis::cmd("GET")
        .arg("mykey")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(value, "Hello World");

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_getrange() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: () = redis::cmd("SET")
        .arg("mykey")
        .arg("This is a string")
        .query_async(&mut conn)
        .await
        .unwrap();

    // GETRANGE
    let substr: String = redis::cmd("GETRANGE")
        .arg("mykey")
        .arg(0)
        .arg(3)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(substr, "This");

    let substr: String = redis::cmd("GETRANGE")
        .arg("mykey")
        .arg(-3)
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(substr, "ing");

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_setrange() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: () = redis::cmd("SET")
        .arg("key1")
        .arg("Hello World")
        .query_async(&mut conn)
        .await
        .unwrap();

    // SETRANGE
    let len: i32 = redis::cmd("SETRANGE")
        .arg("key1")
        .arg(6)
        .arg("Redis")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(len, 11);

    let value: String = redis::cmd("GET")
        .arg("key1")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(value, "Hello Redis");

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_strlen() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: () = redis::cmd("SET")
        .arg("mykey")
        .arg("Hello World")
        .query_async(&mut conn)
        .await
        .unwrap();

    // STRLEN
    let len: i32 = redis::cmd("STRLEN")
        .arg("mykey")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(len, 11);

    // Non-existent key
    let len: i32 = redis::cmd("STRLEN")
        .arg("nonexisting")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(len, 0);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_incr_decr() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: () = redis::cmd("SET")
        .arg("mykey")
        .arg("10")
        .query_async(&mut conn)
        .await
        .unwrap();

    // INCR
    let val: i32 = redis::cmd("INCR")
        .arg("mykey")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(val, 11);

    // INCRBY
    let val: i32 = redis::cmd("INCRBY")
        .arg("mykey")
        .arg(5)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(val, 16);

    // DECR
    let val: i32 = redis::cmd("DECR")
        .arg("mykey")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(val, 15);

    // DECRBY
    let val: i32 = redis::cmd("DECRBY")
        .arg("mykey")
        .arg(10)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(val, 5);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_incrbyfloat() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: () = redis::cmd("SET")
        .arg("mykey")
        .arg("10.50")
        .query_async(&mut conn)
        .await
        .unwrap();

    // INCRBYFLOAT
    let val: String = redis::cmd("INCRBYFLOAT")
        .arg("mykey")
        .arg("0.1")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(val, "10.6");

    let val: String = redis::cmd("INCRBYFLOAT")
        .arg("mykey")
        .arg("-5")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(val, "5.6");

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_mset_mget() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    // MSET
    let _: () = redis::cmd("MSET")
        .arg("key1")
        .arg("Hello")
        .arg("key2")
        .arg("World")
        .query_async(&mut conn)
        .await
        .unwrap();

    // MGET
    let values: Vec<String> = redis::cmd("MGET")
        .arg("key1")
        .arg("key2")
        .arg("nonexisting")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(values.len(), 3);
    assert_eq!(values[0], "Hello");
    assert_eq!(values[1], "World");

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_setnx() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    // SETNX on non-existing key
    let result: i32 = redis::cmd("SETNX")
        .arg("mykey")
        .arg("Hello")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(result, 1);

    // SETNX on existing key
    let result: i32 = redis::cmd("SETNX")
        .arg("mykey")
        .arg("World")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(result, 0);

    let value: String = redis::cmd("GET")
        .arg("mykey")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(value, "Hello");

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_getset() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: () = redis::cmd("SET")
        .arg("mykey")
        .arg("Hello")
        .query_async(&mut conn)
        .await
        .unwrap();

    // GETSET
    let old_value: String = redis::cmd("GETSET")
        .arg("mykey")
        .arg("World")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(old_value, "Hello");

    let new_value: String = redis::cmd("GET")
        .arg("mykey")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(new_value, "World");

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_set_with_options() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    // SET with EX (expiration in seconds)
    let _: () = redis::cmd("SET")
        .arg("mykey")
        .arg("value")
        .arg("EX")
        .arg(10)
        .query_async(&mut conn)
        .await
        .unwrap();

    let ttl: i32 = redis::cmd("TTL")
        .arg("mykey")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert!(ttl > 0 && ttl <= 10);

    // SET with PX (expiration in milliseconds)
    let _: () = redis::cmd("SET")
        .arg("mykey2")
        .arg("value")
        .arg("PX")
        .arg(10000)
        .query_async(&mut conn)
        .await
        .unwrap();

    let pttl: i32 = redis::cmd("PTTL")
        .arg("mykey2")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert!(pttl > 0 && pttl <= 10000);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_binary_safety() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    // Binary data with null bytes
    let binary_data = vec![0u8, 1, 2, 3, 255, 254, 253];

    let _: () = redis::cmd("SET")
        .arg("binkey")
        .arg(&binary_data)
        .query_async(&mut conn)
        .await
        .unwrap();

    let result: Vec<u8> = redis::cmd("GET")
        .arg("binkey")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(result, binary_data);

    server.stop().await;
}
