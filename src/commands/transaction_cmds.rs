// Transaction command handlers

use crate::protocol::RespValue;
use crate::transaction::Transaction;

/// MULTI command
pub async fn multi(tx: &mut Transaction) -> RespValue {
    crate::transaction::multi(tx).await
}

/// EXEC command
pub async fn exec(tx: &mut Transaction) -> RespValue {
    if !tx.in_multi {
        return RespValue::Error("ERR EXEC without MULTI".to_string());
    }

    // Return marker that EXEC was called - actual execution happens in connection handler
    RespValue::SimpleString("__EXEC__".to_string())
}

/// DISCARD command
pub async fn discard(tx: &mut Transaction) -> RespValue {
    crate::transaction::discard(tx).await
}

/// WATCH command
pub async fn watch(tx: &mut Transaction, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'watch' command".to_string());
    }

    if tx.in_multi {
        return RespValue::Error("ERR WATCH inside MULTI is not allowed".to_string());
    }

    for key_bytes in args {
        let key = match std::str::from_utf8(&key_bytes) {
            Ok(s) => s.to_string(),
            Err(_) => return RespValue::Error("ERR invalid key".to_string()),
        };
        tx.watch_key(key);
    }

    RespValue::SimpleString("OK".to_string())
}

/// UNWATCH command
pub async fn unwatch(tx: &mut Transaction) -> RespValue {
    crate::transaction::unwatch(tx).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_multi_exec_discard() {
        let mut tx = Transaction::new();

        // Start transaction
        let result = multi(&mut tx).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));

        // Discard transaction
        let result = discard(&mut tx).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
    }

    #[tokio::test]
    async fn test_watch_unwatch() {
        let mut tx = Transaction::new();

        let result = watch(&mut tx, vec![b"key1".to_vec(), b"key2".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert_eq!(tx.watched_keys.len(), 2);

        let result = unwatch(&mut tx).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert_eq!(tx.watched_keys.len(), 0);
    }
}
