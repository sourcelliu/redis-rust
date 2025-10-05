// Pub/Sub (Publish/Subscribe) implementation

use crate::protocol::RespValue;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Channel for broadcasting messages
type Channel = broadcast::Sender<Vec<u8>>;

/// Pub/Sub manager for handling subscriptions and publishing
pub struct PubSub {
    /// Channels for exact name subscriptions
    channels: DashMap<String, Channel>,
    /// Pattern-based channels (for PSUBSCRIBE)
    patterns: DashMap<String, Channel>,
}

impl PubSub {
    pub fn new() -> Self {
        Self {
            channels: DashMap::new(),
            patterns: DashMap::new(),
        }
    }

    /// Publish a message to a channel
    pub fn publish(&self, channel: &str, message: Vec<u8>) -> usize {
        let mut subscriber_count = 0;

        // Publish to exact channel subscribers
        if let Some(ch) = self.channels.get(channel) {
            subscriber_count += ch.receiver_count();
            let _ = ch.send(message.clone());
        }

        // Publish to pattern subscribers
        for entry in self.patterns.iter() {
            if Self::match_pattern(channel, entry.key()) {
                subscriber_count += entry.value().receiver_count();
                let _ = entry.value().send(message.clone());
            }
        }

        subscriber_count
    }

    /// Get or create a channel for subscription
    pub fn get_or_create_channel(&self, channel_name: &str) -> broadcast::Receiver<Vec<u8>> {
        let entry = self.channels.entry(channel_name.to_string()).or_insert_with(|| {
            let (tx, _) = broadcast::channel(1024);
            tx
        });
        entry.subscribe()
    }

    /// Get or create a pattern channel for PSUBSCRIBE
    pub fn get_or_create_pattern(&self, pattern: &str) -> broadcast::Receiver<Vec<u8>> {
        let entry = self.patterns.entry(pattern.to_string()).or_insert_with(|| {
            let (tx, _) = broadcast::channel(1024);
            tx
        });
        entry.subscribe()
    }

    /// Remove a channel if it has no subscribers
    pub fn cleanup_channel(&self, channel_name: &str) {
        if let Some(entry) = self.channels.get(channel_name) {
            if entry.receiver_count() == 0 {
                drop(entry);
                self.channels.remove(channel_name);
            }
        }
    }

    /// Remove a pattern if it has no subscribers
    pub fn cleanup_pattern(&self, pattern: &str) {
        if let Some(entry) = self.patterns.get(pattern) {
            if entry.receiver_count() == 0 {
                drop(entry);
                self.patterns.remove(pattern);
            }
        }
    }

    /// Get list of active channels (with at least one subscriber)
    pub fn active_channels(&self) -> Vec<String> {
        self.channels
            .iter()
            .filter(|entry| entry.value().receiver_count() > 0)
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get count of subscribers for a channel
    pub fn channel_subscribers(&self, channel: &str) -> usize {
        self.channels
            .get(channel)
            .map(|ch| ch.receiver_count())
            .unwrap_or(0)
    }

    /// Simple pattern matching for PSUBSCRIBE
    /// Supports * (match any) and ? (match one character)
    fn match_pattern(channel: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if !pattern.contains('*') && !pattern.contains('?') {
            return channel == pattern;
        }

        // Simple glob matching
        let pattern_chars: Vec<char> = pattern.chars().collect();
        let channel_chars: Vec<char> = channel.chars().collect();

        Self::glob_match(&channel_chars, &pattern_chars, 0, 0)
    }

    fn glob_match(text: &[char], pattern: &[char], ti: usize, pi: usize) -> bool {
        if pi == pattern.len() {
            return ti == text.len();
        }

        if pattern[pi] == '*' {
            // Try matching zero or more characters
            for i in ti..=text.len() {
                if Self::glob_match(text, pattern, i, pi + 1) {
                    return true;
                }
            }
            false
        } else if ti < text.len() && (pattern[pi] == '?' || pattern[pi] == text[ti]) {
            Self::glob_match(text, pattern, ti + 1, pi + 1)
        } else {
            false
        }
    }
}

impl Default for PubSub {
    fn default() -> Self {
        Self::new()
    }
}

/// PUBLISH command handler
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

/// Subscribe state for a connection
#[derive(Debug)]
pub struct SubscriptionState {
    pub channels: Vec<String>,
    pub patterns: Vec<String>,
}

impl SubscriptionState {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            patterns: Vec::new(),
        }
    }

    pub fn is_subscribed(&self) -> bool {
        !self.channels.is_empty() || !self.patterns.is_empty()
    }

    pub fn add_channel(&mut self, channel: String) {
        if !self.channels.contains(&channel) {
            self.channels.push(channel);
        }
    }

    pub fn remove_channel(&mut self, channel: &str) -> bool {
        if let Some(pos) = self.channels.iter().position(|c| c == channel) {
            self.channels.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn add_pattern(&mut self, pattern: String) {
        if !self.patterns.contains(&pattern) {
            self.patterns.push(pattern);
        }
    }

    pub fn remove_pattern(&mut self, pattern: &str) -> bool {
        if let Some(pos) = self.patterns.iter().position(|p| p == pattern) {
            self.patterns.remove(pos);
            true
        } else {
            false
        }
    }
}

impl Default for SubscriptionState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        assert!(PubSub::match_pattern("test", "*"));
        assert!(PubSub::match_pattern("test", "test"));
        assert!(PubSub::match_pattern("test", "t*t"));
        assert!(PubSub::match_pattern("test", "t?st"));
        assert!(PubSub::match_pattern("news.sports", "news.*"));
        assert!(PubSub::match_pattern("news.sports.football", "news.*"));
        assert!(!PubSub::match_pattern("test", "best"));
        assert!(!PubSub::match_pattern("test", "t?t"));
    }

    #[tokio::test]
    async fn test_publish_subscribe() {
        let pubsub = Arc::new(PubSub::new());

        // Create a subscriber
        let mut rx = pubsub.get_or_create_channel("test_channel");

        // Publish a message
        let result = publish(
            &pubsub,
            vec![b"test_channel".to_vec(), b"hello".to_vec()],
        )
        .await;

        assert_eq!(result, RespValue::Integer(1));

        // Receive the message
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg, b"hello".to_vec());
    }

    #[test]
    fn test_subscription_state() {
        let mut state = SubscriptionState::new();

        assert!(!state.is_subscribed());

        state.add_channel("ch1".to_string());
        assert!(state.is_subscribed());
        assert_eq!(state.channels.len(), 1);

        state.add_channel("ch1".to_string()); // Duplicate
        assert_eq!(state.channels.len(), 1);

        state.remove_channel("ch1");
        assert!(!state.is_subscribed());
    }
}
