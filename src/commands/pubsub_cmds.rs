// Pub/Sub command handlers

use crate::protocol::RespValue;
use crate::pubsub::{PubSub, SubscriptionState};
use std::sync::Arc;

/// PUBLISH channel message
pub async fn publish(pubsub: &Arc<PubSub>, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'publish' command".to_string());
    }

    let channel = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid channel name".to_string()),
    };

    let message = args[1].clone();
    let count = pubsub.publish(channel, message);

    RespValue::Integer(count as i64)
}

/// SUBSCRIBE channel [channel ...]
pub async fn subscribe(
    pubsub: &Arc<PubSub>,
    state: &mut SubscriptionState,
    args: Vec<Vec<u8>>,
) -> Vec<RespValue> {
    if args.is_empty() {
        return vec![RespValue::Error(
            "ERR wrong number of arguments for 'subscribe' command".to_string(),
        )];
    }

    let mut responses = Vec::new();

    for channel_bytes in args {
        let channel = match std::str::from_utf8(&channel_bytes) {
            Ok(s) => s.to_string(),
            Err(_) => {
                responses.push(RespValue::Error("ERR invalid channel name".to_string()));
                continue;
            }
        };

        // Add to subscription state
        state.add_channel(channel.clone());

        // Subscribe to the channel
        let _rx = pubsub.get_or_create_channel(&channel);

        // Return subscription confirmation
        responses.push(RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"subscribe".to_vec())),
            RespValue::BulkString(Some(channel.into_bytes())),
            RespValue::Integer(state.channels.len() as i64),
        ])));
    }

    responses
}

/// UNSUBSCRIBE [channel [channel ...]]
pub async fn unsubscribe(
    pubsub: &Arc<PubSub>,
    state: &mut SubscriptionState,
    args: Vec<Vec<u8>>,
) -> Vec<RespValue> {
    let mut responses = Vec::new();

    if args.is_empty() {
        // Unsubscribe from all channels
        let channels: Vec<String> = state.channels.drain(..).collect();
        for channel in channels {
            pubsub.cleanup_channel(&channel);
            responses.push(RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"unsubscribe".to_vec())),
                RespValue::BulkString(Some(channel.into_bytes())),
                RespValue::Integer(state.channels.len() as i64),
            ])));
        }
    } else {
        for channel_bytes in args {
            let channel = match std::str::from_utf8(&channel_bytes) {
                Ok(s) => s,
                Err(_) => {
                    responses.push(RespValue::Error("ERR invalid channel name".to_string()));
                    continue;
                }
            };

            state.remove_channel(channel);
            pubsub.cleanup_channel(channel);

            responses.push(RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"unsubscribe".to_vec())),
                RespValue::BulkString(Some(channel.as_bytes().to_vec())),
                RespValue::Integer(state.channels.len() as i64),
            ])));
        }
    }

    responses
}

/// PSUBSCRIBE pattern [pattern ...]
pub async fn psubscribe(
    pubsub: &Arc<PubSub>,
    state: &mut SubscriptionState,
    args: Vec<Vec<u8>>,
) -> Vec<RespValue> {
    if args.is_empty() {
        return vec![RespValue::Error(
            "ERR wrong number of arguments for 'psubscribe' command".to_string(),
        )];
    }

    let mut responses = Vec::new();

    for pattern_bytes in args {
        let pattern = match std::str::from_utf8(&pattern_bytes) {
            Ok(s) => s.to_string(),
            Err(_) => {
                responses.push(RespValue::Error("ERR invalid pattern".to_string()));
                continue;
            }
        };

        // Add to subscription state
        state.add_pattern(pattern.clone());

        // Subscribe to the pattern
        let _rx = pubsub.get_or_create_pattern(&pattern);

        // Return subscription confirmation
        responses.push(RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"psubscribe".to_vec())),
            RespValue::BulkString(Some(pattern.into_bytes())),
            RespValue::Integer(state.patterns.len() as i64),
        ])));
    }

    responses
}

/// PUNSUBSCRIBE [pattern [pattern ...]]
pub async fn punsubscribe(
    pubsub: &Arc<PubSub>,
    state: &mut SubscriptionState,
    args: Vec<Vec<u8>>,
) -> Vec<RespValue> {
    let mut responses = Vec::new();

    if args.is_empty() {
        // Unsubscribe from all patterns
        let patterns: Vec<String> = state.patterns.drain(..).collect();
        for pattern in patterns {
            pubsub.cleanup_pattern(&pattern);
            responses.push(RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"punsubscribe".to_vec())),
                RespValue::BulkString(Some(pattern.into_bytes())),
                RespValue::Integer(state.patterns.len() as i64),
            ])));
        }
    } else {
        for pattern_bytes in args {
            let pattern = match std::str::from_utf8(&pattern_bytes) {
                Ok(s) => s,
                Err(_) => {
                    responses.push(RespValue::Error("ERR invalid pattern".to_string()));
                    continue;
                }
            };

            state.remove_pattern(pattern);
            pubsub.cleanup_pattern(pattern);

            responses.push(RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"punsubscribe".to_vec())),
                RespValue::BulkString(Some(pattern.as_bytes().to_vec())),
                RespValue::Integer(state.patterns.len() as i64),
            ])));
        }
    }

    responses
}

/// PUBSUB CHANNELS [pattern]
pub async fn pubsub_channels(pubsub: &Arc<PubSub>, args: Vec<Vec<u8>>) -> RespValue {
    let pattern = if args.is_empty() {
        "*"
    } else {
        match std::str::from_utf8(&args[0]) {
            Ok(s) => s,
            Err(_) => return RespValue::Error("ERR invalid pattern".to_string()),
        }
    };

    let channels = pubsub.active_channels();
    let filtered: Vec<RespValue> = channels
        .into_iter()
        .filter(|ch| pattern == "*" || ch.contains(pattern))
        .map(|ch| RespValue::BulkString(Some(ch.into_bytes())))
        .collect();

    RespValue::Array(Some(filtered))
}

/// PUBSUB NUMSUB [channel [channel ...]]
pub async fn pubsub_numsub(pubsub: &Arc<PubSub>, args: Vec<Vec<u8>>) -> RespValue {
    let mut result = Vec::new();

    for channel_bytes in args {
        let channel = match std::str::from_utf8(&channel_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        result.push(RespValue::BulkString(Some(channel.as_bytes().to_vec())));
        result.push(RespValue::Integer(
            pubsub.channel_subscribers(channel) as i64,
        ));
    }

    RespValue::Array(Some(result))
}

/// PUBSUB NUMPAT
pub async fn pubsub_numpat(pubsub: &Arc<PubSub>) -> RespValue {
    // Count active patterns
    let count = pubsub
        .active_channels()
        .iter()
        .filter(|_| true) // In full implementation, would count pattern subscriptions
        .count();

    RespValue::Integer(count as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscribe_unsubscribe() {
        let pubsub = Arc::new(PubSub::new());
        let mut state = SubscriptionState::new();

        // Subscribe to channels
        let responses = subscribe(
            &pubsub,
            &mut state,
            vec![b"ch1".to_vec(), b"ch2".to_vec()],
        )
        .await;

        assert_eq!(responses.len(), 2);
        assert_eq!(state.channels.len(), 2);

        // Unsubscribe from one channel
        let responses = unsubscribe(&pubsub, &mut state, vec![b"ch1".to_vec()]).await;

        assert_eq!(responses.len(), 1);
        assert_eq!(state.channels.len(), 1);

        // Unsubscribe from all
        let responses = unsubscribe(&pubsub, &mut state, vec![]).await;

        assert_eq!(responses.len(), 1);
        assert_eq!(state.channels.len(), 0);
    }

    #[tokio::test]
    async fn test_psubscribe() {
        let pubsub = Arc::new(PubSub::new());
        let mut state = SubscriptionState::new();

        let responses = psubscribe(&pubsub, &mut state, vec![b"news.*".to_vec()]).await;

        assert_eq!(responses.len(), 1);
        assert_eq!(state.patterns.len(), 1);
    }
}
