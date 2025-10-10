// Permission system for ACL commands

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Command categories for ACL permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandCategory {
    /// Key space commands (GET, SET, DEL, etc.)
    Keyspace,
    /// Read commands
    Read,
    /// Write commands
    Write,
    /// Set commands
    Set,
    /// Sorted set commands
    SortedSet,
    /// List commands
    List,
    /// Hash commands
    Hash,
    /// String commands
    String,
    /// Bitmap commands
    Bitmap,
    /// HyperLogLog commands
    HyperLogLog,
    /// Geo commands
    Geo,
    /// Stream commands
    Stream,
    /// Pub/Sub commands
    PubSub,
    /// Transaction commands
    Transaction,
    /// Scripting commands
    Scripting,
    /// Server management commands
    Admin,
    /// Dangerous commands (FLUSHDB, FLUSHALL, etc.)
    Dangerous,
    /// Connection commands
    Connection,
    /// Fast commands (O(1) complexity)
    Fast,
    /// Slow commands
    Slow,
    /// All commands
    All,
}

impl CommandCategory {
    /// Get the category name as used in ACL rules
    pub fn name(&self) -> &'static str {
        match self {
            CommandCategory::Keyspace => "@keyspace",
            CommandCategory::Read => "@read",
            CommandCategory::Write => "@write",
            CommandCategory::Set => "@set",
            CommandCategory::SortedSet => "@sortedset",
            CommandCategory::List => "@list",
            CommandCategory::Hash => "@hash",
            CommandCategory::String => "@string",
            CommandCategory::Bitmap => "@bitmap",
            CommandCategory::HyperLogLog => "@hyperloglog",
            CommandCategory::Geo => "@geo",
            CommandCategory::Stream => "@stream",
            CommandCategory::PubSub => "@pubsub",
            CommandCategory::Transaction => "@transaction",
            CommandCategory::Scripting => "@scripting",
            CommandCategory::Admin => "@admin",
            CommandCategory::Dangerous => "@dangerous",
            CommandCategory::Connection => "@connection",
            CommandCategory::Fast => "@fast",
            CommandCategory::Slow => "@slow",
            CommandCategory::All => "@all",
        }
    }

    /// Parse a category from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "@keyspace" => Some(CommandCategory::Keyspace),
            "@read" => Some(CommandCategory::Read),
            "@write" => Some(CommandCategory::Write),
            "@set" => Some(CommandCategory::Set),
            "@sortedset" => Some(CommandCategory::SortedSet),
            "@list" => Some(CommandCategory::List),
            "@hash" => Some(CommandCategory::Hash),
            "@string" => Some(CommandCategory::String),
            "@bitmap" => Some(CommandCategory::Bitmap),
            "@hyperloglog" => Some(CommandCategory::HyperLogLog),
            "@geo" => Some(CommandCategory::Geo),
            "@stream" => Some(CommandCategory::Stream),
            "@pubsub" => Some(CommandCategory::PubSub),
            "@transaction" => Some(CommandCategory::Transaction),
            "@scripting" => Some(CommandCategory::Scripting),
            "@admin" => Some(CommandCategory::Admin),
            "@dangerous" => Some(CommandCategory::Dangerous),
            "@connection" => Some(CommandCategory::Connection),
            "@fast" => Some(CommandCategory::Fast),
            "@slow" => Some(CommandCategory::Slow),
            "@all" => Some(CommandCategory::All),
            _ => None,
        }
    }

    /// Get the commands in this category
    pub fn commands(&self) -> HashSet<&'static str> {
        match self {
            CommandCategory::Read => {
                ["GET", "MGET", "STRLEN", "EXISTS", "TYPE", "KEYS", "SCAN",
                 "HGET", "HMGET", "HGETALL", "HKEYS", "HVALS", "LRANGE", "LLEN",
                 "SMEMBERS", "SCARD", "ZRANGE", "ZCARD"].iter().copied().collect()
            }
            CommandCategory::Write => {
                ["SET", "MSET", "DEL", "SETEX", "SETNX", "APPEND", "INCR", "DECR",
                 "HSET", "HDEL", "LPUSH", "RPUSH", "LPOP", "RPOP", "SADD", "SREM",
                 "ZADD", "ZREM", "EXPIRE", "PERSIST"].iter().copied().collect()
            }
            CommandCategory::String => {
                ["GET", "SET", "MGET", "MSET", "STRLEN", "APPEND", "INCR", "DECR",
                 "SETEX", "SETNX", "GETSET", "GETRANGE", "SETRANGE"].iter().copied().collect()
            }
            CommandCategory::List => {
                ["LPUSH", "RPUSH", "LPOP", "RPOP", "LRANGE", "LLEN", "LINDEX",
                 "LSET", "LREM", "LTRIM", "BLPOP", "BRPOP"].iter().copied().collect()
            }
            CommandCategory::Set => {
                ["SADD", "SREM", "SMEMBERS", "SISMEMBER", "SCARD", "SPOP",
                 "SRANDMEMBER", "SUNION", "SINTER", "SDIFF"].iter().copied().collect()
            }
            CommandCategory::Hash => {
                ["HSET", "HGET", "HMSET", "HMGET", "HDEL", "HEXISTS", "HKEYS",
                 "HVALS", "HGETALL", "HLEN", "HINCRBY"].iter().copied().collect()
            }
            CommandCategory::SortedSet => {
                ["ZADD", "ZREM", "ZRANGE", "ZCARD", "ZSCORE", "ZRANK", "ZINCRBY",
                 "ZCOUNT", "ZRANGEBYSCORE", "ZREMRANGEBYRANK"].iter().copied().collect()
            }
            CommandCategory::PubSub => {
                ["PUBLISH", "SUBSCRIBE", "UNSUBSCRIBE", "PSUBSCRIBE",
                 "PUNSUBSCRIBE", "PUBSUB"].iter().copied().collect()
            }
            CommandCategory::Transaction => {
                ["MULTI", "EXEC", "DISCARD", "WATCH", "UNWATCH"].iter().copied().collect()
            }
            CommandCategory::Scripting => {
                ["EVAL", "EVALSHA", "SCRIPT"].iter().copied().collect()
            }
            CommandCategory::Admin => {
                ["CONFIG", "INFO", "DBSIZE", "SAVE", "BGSAVE", "LASTSAVE",
                 "SHUTDOWN", "CLIENT", "COMMAND", "SLOWLOG", "MONITOR"].iter().copied().collect()
            }
            CommandCategory::Dangerous => {
                ["FLUSHDB", "FLUSHALL", "SHUTDOWN", "CONFIG", "DEBUG",
                 "BGREWRITEAOF", "BGSAVE", "SAVE", "KEYS"].iter().copied().collect()
            }
            CommandCategory::Connection => {
                ["AUTH", "PING", "ECHO", "SELECT", "QUIT"].iter().copied().collect()
            }
            CommandCategory::All => HashSet::new(), // Special case
            _ => HashSet::new(),
        }
    }

    /// Check if a command belongs to this category
    pub fn contains_command(&self, command: &str) -> bool {
        let command_upper = command.to_uppercase();
        if *self == CommandCategory::All {
            return true;
        }
        self.commands().contains(command_upper.as_str())
    }
}

/// Represents a permission rule for ACL
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    /// Allow a specific command
    AllowCommand(String),
    /// Deny a specific command
    DenyCommand(String),
    /// Allow all commands in a category
    AllowCategory(CommandCategory),
    /// Deny all commands in a category
    DenyCategory(CommandCategory),
    /// Allow all commands
    AllowAllCommands,
    /// Deny all commands
    DenyAllCommands,
}

impl Permission {
    /// Check if this permission allows a command
    pub fn allows(&self, command: &str) -> Option<bool> {
        let command_upper = command.to_uppercase();
        match self {
            Permission::AllowCommand(cmd) => {
                if cmd.to_uppercase() == command_upper {
                    Some(true)
                } else {
                    None
                }
            }
            Permission::DenyCommand(cmd) => {
                if cmd.to_uppercase() == command_upper {
                    Some(false)
                } else {
                    None
                }
            }
            Permission::AllowCategory(cat) => {
                if cat.contains_command(&command_upper) {
                    Some(true)
                } else {
                    None
                }
            }
            Permission::DenyCategory(cat) => {
                if cat.contains_command(&command_upper) {
                    Some(false)
                } else {
                    None
                }
            }
            Permission::AllowAllCommands => Some(true),
            Permission::DenyAllCommands => Some(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_category() {
        let cat = CommandCategory::Read;
        assert!(cat.contains_command("GET"));
        assert!(cat.contains_command("get"));
        assert!(!cat.contains_command("SET"));
    }

    #[test]
    fn test_permission_allow_command() {
        let perm = Permission::AllowCommand("GET".to_string());
        assert_eq!(perm.allows("GET"), Some(true));
        assert_eq!(perm.allows("SET"), None);
    }

    #[test]
    fn test_permission_allow_category() {
        let perm = Permission::AllowCategory(CommandCategory::Read);
        assert_eq!(perm.allows("GET"), Some(true));
        assert_eq!(perm.allows("SET"), None);
    }

    #[test]
    fn test_permission_deny() {
        let perm = Permission::DenyCommand("FLUSHDB".to_string());
        assert_eq!(perm.allows("FLUSHDB"), Some(false));
        assert_eq!(perm.allows("GET"), None);
    }
}
