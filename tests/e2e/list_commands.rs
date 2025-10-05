// E2E tests for List commands
// Translated from Redis test suite: tests/unit/type/list.tcl

mod common;

#[tokio::test]
#[ignore]
async fn test_lpush_llen() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    // LPUSH
    let len: i32 = redis::cmd("LPUSH")
        .arg("mylist")
        .arg("World")
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(len, 1);

    let len: i32 = redis::cmd("LPUSH")
        .arg("mylist")
        .arg("Hello")
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(len, 2);

    // LLEN
    let len: i32 = redis::cmd("LLEN")
        .arg("mylist")
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(len, 2);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_rpush_lrange() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    // RPUSH
    let _: i32 = redis::cmd("RPUSH")
        .arg("mylist")
        .arg("Hello")
        .arg("World")
        .query_async(&mut conn)
        .await
        .unwrap();

    // LRANGE
    let values: Vec<String> = redis::cmd("LRANGE")
        .arg("mylist")
        .arg(0)
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(values, vec!["Hello", "World"]);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_lpop_rpop() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: i32 = redis::cmd("RPUSH")
        .arg("mylist")
        .arg("one")
        .arg("two")
        .arg("three")
        .query_async(&mut conn)
        .await
        .unwrap();

    // LPOP
    let value: String = redis::cmd("LPOP")
        .arg("mylist")
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(value, "one");

    // RPOP
    let value: String = redis::cmd("RPOP")
        .arg("mylist")
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(value, "three");

    // Verify remaining
    let values: Vec<String> = redis::cmd("LRANGE")
        .arg("mylist")
        .arg(0)
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(values, vec!["two"]);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_lindex() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: i32 = redis::cmd("RPUSH")
        .arg("mylist")
        .arg("World")
        .arg("Hello")
        .query_async(&mut conn)
        .await
        .unwrap();

    // LINDEX
    let value: String = redis::cmd("LINDEX")
        .arg("mylist")
        .arg(0)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(value, "World");

    let value: String = redis::cmd("LINDEX")
        .arg("mylist")
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(value, "Hello");

    let value: Option<String> = redis::cmd("LINDEX")
        .arg("mylist")
        .arg(3)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(value, None);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_lset() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: i32 = redis::cmd("RPUSH")
        .arg("mylist")
        .arg("one")
        .arg("two")
        .arg("three")
        .query_async(&mut conn)
        .await
        .unwrap();

    // LSET
    let _: () = redis::cmd("LSET")
        .arg("mylist")
        .arg(0)
        .arg("four")
        .query_async(&mut conn)
        .await
        .unwrap();

    let _: () = redis::cmd("LSET")
        .arg("mylist")
        .arg(-2)
        .arg("five")
        .query_async(&mut conn)
        .await
        .unwrap();

    let values: Vec<String> = redis::cmd("LRANGE")
        .arg("mylist")
        .arg(0)
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(values, vec!["four", "five", "three"]);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_ltrim() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: i32 = redis::cmd("RPUSH")
        .arg("mylist")
        .arg("one")
        .arg("two")
        .arg("three")
        .arg("four")
        .arg("five")
        .query_async(&mut conn)
        .await
        .unwrap();

    // LTRIM
    let _: () = redis::cmd("LTRIM")
        .arg("mylist")
        .arg(1)
        .arg(3)
        .query_async(&mut conn)
        .await
        .unwrap();

    let values: Vec<String> = redis::cmd("LRANGE")
        .arg("mylist")
        .arg(0)
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(values, vec!["two", "three", "four"]);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_linsert() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: i32 = redis::cmd("RPUSH")
        .arg("mylist")
        .arg("Hello")
        .arg("World")
        .query_async(&mut conn)
        .await
        .unwrap();

    // LINSERT BEFORE
    let len: i32 = redis::cmd("LINSERT")
        .arg("mylist")
        .arg("BEFORE")
        .arg("World")
        .arg("There")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(len, 3);

    let values: Vec<String> = redis::cmd("LRANGE")
        .arg("mylist")
        .arg(0)
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(values, vec!["Hello", "There", "World"]);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_lrem() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: i32 = redis::cmd("RPUSH")
        .arg("mylist")
        .arg("hello")
        .arg("hello")
        .arg("foo")
        .arg("hello")
        .query_async(&mut conn)
        .await
        .unwrap();

    // LREM - remove 2 occurrences of "hello"
    let count: i32 = redis::cmd("LREM")
        .arg("mylist")
        .arg(2)
        .arg("hello")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(count, 2);

    let values: Vec<String> = redis::cmd("LRANGE")
        .arg("mylist")
        .arg(0)
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(values, vec!["foo", "hello"]);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_rpoplpush() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: i32 = redis::cmd("RPUSH")
        .arg("mylist")
        .arg("one")
        .arg("two")
        .arg("three")
        .query_async(&mut conn)
        .await
        .unwrap();

    // RPOPLPUSH
    let value: String = redis::cmd("RPOPLPUSH")
        .arg("mylist")
        .arg("myotherlist")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(value, "three");

    let values: Vec<String> = redis::cmd("LRANGE")
        .arg("mylist")
        .arg(0)
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(values, vec!["one", "two"]);

    let values: Vec<String> = redis::cmd("LRANGE")
        .arg("myotherlist")
        .arg(0)
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(values, vec!["three"]);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_blpop() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: i32 = redis::cmd("RPUSH")
        .arg("mylist")
        .arg("one")
        .arg("two")
        .query_async(&mut conn)
        .await
        .unwrap();

    // BLPOP with existing list
    let result: Vec<String> = redis::cmd("BLPOP")
        .arg("mylist")
        .arg(1)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(result, vec!["mylist", "one"]);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_brpop() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    let _: i32 = redis::cmd("RPUSH")
        .arg("mylist")
        .arg("one")
        .arg("two")
        .query_async(&mut conn)
        .await
        .unwrap();

    // BRPOP with existing list
    let result: Vec<String> = redis::cmd("BRPOP")
        .arg("mylist")
        .arg(1)
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(result, vec!["mylist", "two"]);

    server.stop().await;
}

#[tokio::test]
#[ignore]
async fn test_large_list() {
    let server = common::TestRedisServer::start().await;
    let mut conn = server.get_async_connection().await.unwrap();

    // Push 1000 elements
    for i in 0..1000 {
        let _: i32 = redis::cmd("RPUSH")
            .arg("biglist")
            .arg(format!("element{}", i))
            .query_async(&mut conn)
            .await
            .unwrap();
    }

    let len: i32 = redis::cmd("LLEN")
        .arg("biglist")
        .query_async(&mut conn)
        .await
        .unwrap();

    assert_eq!(len, 1000);

    // Verify first and last elements
    let first: String = redis::cmd("LINDEX")
        .arg("biglist")
        .arg(0)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(first, "element0");

    let last: String = redis::cmd("LINDEX")
        .arg("biglist")
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(last, "element999");

    server.stop().await;
}
