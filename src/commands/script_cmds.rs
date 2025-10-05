// Script commands (EVAL, EVALSHA, SCRIPT)

use crate::protocol::RespValue;
use crate::scripting::{LuaEngine, ScriptCache};
use crate::storage::db::Database;
use std::sync::Arc;
use tracing::debug;

/// EVAL - Execute a Lua script
pub async fn eval(
    db: &Arc<Database>,
    db_index: usize,
    script_cache: &Arc<ScriptCache>,
    args: Vec<Vec<u8>>,
) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'eval' command".to_string(),
        );
    }

    // Parse script
    let script = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid script".to_string()),
    };

    // Parse numkeys
    let numkeys = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<usize>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR invalid numkeys".to_string()),
        },
        Err(_) => return RespValue::Error("ERR invalid numkeys".to_string()),
    };

    // Extract KEYS and ARGV
    let remaining_args = &args[2..];
    if remaining_args.len() < numkeys {
        return RespValue::Error("ERR not enough arguments for KEYS".to_string());
    }

    let keys = remaining_args[..numkeys].to_vec();
    let argv = remaining_args[numkeys..].to_vec();

    debug!(
        "EVAL: script_len={}, numkeys={}, keys={}, args={}",
        script.len(),
        numkeys,
        keys.len(),
        argv.len()
    );

    // Cache the script
    let _sha1 = script_cache.load(script.to_string());

    // Execute script
    match LuaEngine::new() {
        Ok(engine) => match engine.execute(script, keys, argv, db, db_index).await {
            Ok(result) => result,
            Err(e) => RespValue::Error(format!("ERR Error running script: {}", e)),
        },
        Err(e) => RespValue::Error(format!("ERR Failed to create Lua engine: {}", e)),
    }
}

/// EVALSHA - Execute a cached Lua script by SHA1
pub async fn evalsha(
    db: &Arc<Database>,
    db_index: usize,
    script_cache: &Arc<ScriptCache>,
    args: Vec<Vec<u8>>,
) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'evalsha' command".to_string(),
        );
    }

    // Parse SHA1
    let sha1 = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid SHA1".to_string()),
    };

    // Get script from cache
    let script = match script_cache.get(sha1) {
        Some(s) => s,
        None => {
            return RespValue::Error(format!(
                "NOSCRIPT No matching script. Please use EVAL. SHA: {}",
                sha1
            ))
        }
    };

    // Parse numkeys
    let numkeys = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<usize>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR invalid numkeys".to_string()),
        },
        Err(_) => return RespValue::Error("ERR invalid numkeys".to_string()),
    };

    // Extract KEYS and ARGV
    let remaining_args = &args[2..];
    if remaining_args.len() < numkeys {
        return RespValue::Error("ERR not enough arguments for KEYS".to_string());
    }

    let keys = remaining_args[..numkeys].to_vec();
    let argv = remaining_args[numkeys..].to_vec();

    debug!(
        "EVALSHA: sha1={}, numkeys={}, keys={}, args={}",
        sha1,
        numkeys,
        keys.len(),
        argv.len()
    );

    // Execute script
    match LuaEngine::new() {
        Ok(engine) => match engine.execute(&script, keys, argv, db, db_index).await {
            Ok(result) => result,
            Err(e) => RespValue::Error(format!("ERR Error running script: {}", e)),
        },
        Err(e) => RespValue::Error(format!("ERR Failed to create Lua engine: {}", e)),
    }
}

/// SCRIPT LOAD - Load a script into the cache
pub async fn script_load(script_cache: &Arc<ScriptCache>, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'script load' command".to_string(),
        );
    }

    let script = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid script".to_string()),
    };

    let sha1 = script_cache.load(script.to_string());
    RespValue::BulkString(Some(sha1.into_bytes()))
}

/// SCRIPT EXISTS - Check if scripts exist in cache
pub async fn script_exists(script_cache: &Arc<ScriptCache>, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error(
            "ERR wrong number of arguments for 'script exists' command".to_string(),
        );
    }

    let mut sha1s = Vec::new();
    for arg in args {
        match std::str::from_utf8(&arg) {
            Ok(s) => sha1s.push(s.to_string()),
            Err(_) => return RespValue::Error("ERR invalid SHA1".to_string()),
        }
    }

    let results = script_cache.exists_multi(&sha1s);
    let resp_results: Vec<RespValue> = results
        .into_iter()
        .map(|exists| RespValue::Integer(if exists { 1 } else { 0 }))
        .collect();

    RespValue::Array(Some(resp_results))
}

/// SCRIPT FLUSH - Flush all scripts from cache
pub async fn script_flush(script_cache: &Arc<ScriptCache>) -> RespValue {
    script_cache.flush();
    RespValue::SimpleString("OK".to_string())
}

/// SCRIPT - Main SCRIPT command dispatcher
pub async fn script(
    _db: &Arc<Database>,
    _db_index: usize,
    script_cache: &Arc<ScriptCache>,
    args: Vec<Vec<u8>>,
) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'script' command".to_string());
    }

    let subcommand = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR invalid subcommand".to_string()),
    };

    let remaining_args = args[1..].to_vec();

    match subcommand.as_str() {
        "LOAD" => script_load(script_cache, remaining_args).await,
        "EXISTS" => script_exists(script_cache, remaining_args).await,
        "FLUSH" => script_flush(script_cache).await,
        _ => RespValue::Error(format!(
            "ERR Unknown SCRIPT subcommand '{}'",
            subcommand
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_script_load() {
        let cache = Arc::new(ScriptCache::new());
        let script = b"return 'hello'".to_vec();

        let result = script_load(&cache, vec![script.clone()]).await;

        match result {
            RespValue::BulkString(Some(sha1)) => {
                let sha1_str = String::from_utf8(sha1).unwrap();
                assert_eq!(sha1_str.len(), 40);
            }
            _ => panic!("Expected BulkString"),
        }
    }

    #[tokio::test]
    async fn test_script_exists() {
        let cache = Arc::new(ScriptCache::new());
        let script = "return 'hello'".to_string();
        let sha1 = cache.load(script);

        let result = script_exists(
            &cache,
            vec![sha1.as_bytes().to_vec(), b"nonexistent".to_vec()],
        )
        .await;

        match result {
            RespValue::Array(Some(arr)) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0], RespValue::Integer(1));
                assert_eq!(arr[1], RespValue::Integer(0));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[tokio::test]
    async fn test_script_flush() {
        let cache = Arc::new(ScriptCache::new());
        let script = "return 'hello'".to_string();
        cache.load(script);

        assert_eq!(cache.len(), 1);

        let result = script_flush(&cache).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert_eq!(cache.len(), 0);
    }
}
