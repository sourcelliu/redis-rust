// User definition and flags for ACL

use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use super::permission::Permission;

bitflags::bitflags! {
    /// User flags that control user state and behavior
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct UserFlags: u32 {
        /// User is enabled and can authenticate
        const ENABLED = 0b00000001;
        /// User has no password (dangerous!)
        const NO_PASS = 0b00000010;
        /// User can execute all commands
        const ALL_COMMANDS = 0b00000100;
        /// User can access all keys
        const ALL_KEYS = 0b00001000;
        /// User can access all channels (pub/sub)
        const ALL_CHANNELS = 0b00010000;
    }
}

// Manual Serialize/Deserialize implementation for UserFlags
impl serde::Serialize for UserFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(self.bits())
    }
}

impl<'de> serde::Deserialize<'de> for UserFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = u32::deserialize(deserializer)?;
        Ok(UserFlags::from_bits_truncate(bits))
    }
}

/// Represents a user in the ACL system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Username
    pub username: String,
    /// Hashed passwords (supports multiple passwords)
    pub passwords: HashSet<String>,
    /// User flags
    pub flags: UserFlags,
    /// Permissions for commands
    pub permissions: Vec<Permission>,
    /// Allowed key patterns (glob-style)
    pub key_patterns: Vec<String>,
    /// Allowed pub/sub channel patterns
    pub channel_patterns: Vec<String>,
}

impl User {
    /// Create a new user with default permissions (disabled, no access)
    pub fn new(username: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            passwords: HashSet::new(),
            flags: UserFlags::empty(),
            permissions: Vec::new(),
            key_patterns: Vec::new(),
            channel_patterns: Vec::new(),
        }
    }

    /// Create a default user with full permissions
    pub fn default_user() -> Self {
        let mut user = Self::new("default");
        user.flags = UserFlags::ENABLED | UserFlags::ALL_COMMANDS | UserFlags::ALL_KEYS | UserFlags::ALL_CHANNELS;
        user.flags |= UserFlags::NO_PASS; // Default user has no password initially
        user
    }

    /// Enable the user
    pub fn enable(&mut self) {
        self.flags |= UserFlags::ENABLED;
    }

    /// Disable the user
    pub fn disable(&mut self) {
        self.flags.remove(UserFlags::ENABLED);
    }

    /// Check if user is enabled
    pub fn is_enabled(&self) -> bool {
        self.flags.contains(UserFlags::ENABLED)
    }

    /// Add a password to the user (stored as SHA256 hash)
    pub fn add_password(&mut self, password: &str) {
        let hashed = Self::hash_password(password);
        self.passwords.insert(hashed);
        self.flags.remove(UserFlags::NO_PASS);
    }

    /// Remove a password from the user
    pub fn remove_password(&mut self, password: &str) {
        let hashed = Self::hash_password(password);
        self.passwords.remove(&hashed);
        if self.passwords.is_empty() {
            self.flags |= UserFlags::NO_PASS;
        }
    }

    /// Remove all passwords
    pub fn remove_all_passwords(&mut self) {
        self.passwords.clear();
        self.flags |= UserFlags::NO_PASS;
    }

    /// Verify a password
    pub fn verify_password(&self, password: &str) -> bool {
        if self.flags.contains(UserFlags::NO_PASS) {
            return true;
        }
        let hashed = Self::hash_password(password);
        self.passwords.contains(&hashed)
    }

    /// Hash a password using SHA256
    fn hash_password(password: &str) -> String {
        use sha1::{Sha1, Digest};
        let mut hasher = Sha1::new();
        hasher.update(password.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Grant permission to a command
    pub fn add_permission(&mut self, permission: Permission) {
        self.permissions.push(permission);
    }

    /// Grant access to all commands
    pub fn grant_all_commands(&mut self) {
        self.flags |= UserFlags::ALL_COMMANDS;
    }

    /// Revoke access to all commands
    pub fn revoke_all_commands(&mut self) {
        self.flags.remove(UserFlags::ALL_COMMANDS);
    }

    /// Add a key pattern
    pub fn add_key_pattern(&mut self, pattern: impl Into<String>) {
        self.key_patterns.push(pattern.into());
    }

    /// Grant access to all keys
    pub fn grant_all_keys(&mut self) {
        self.flags |= UserFlags::ALL_KEYS;
    }

    /// Revoke access to all keys
    pub fn revoke_all_keys(&mut self) {
        self.flags.remove(UserFlags::ALL_KEYS);
        self.key_patterns.clear();
    }

    /// Add a channel pattern
    pub fn add_channel_pattern(&mut self, pattern: impl Into<String>) {
        self.channel_patterns.push(pattern.into());
    }

    /// Grant access to all channels
    pub fn grant_all_channels(&mut self) {
        self.flags |= UserFlags::ALL_CHANNELS;
    }

    /// Check if user can access a specific key
    pub fn can_access_key(&self, key: &str) -> bool {
        if self.flags.contains(UserFlags::ALL_KEYS) {
            return true;
        }

        // Check against key patterns
        for pattern in &self.key_patterns {
            if Self::matches_pattern(key, pattern) {
                return true;
            }
        }

        false
    }

    /// Check if user can access a specific channel
    pub fn can_access_channel(&self, channel: &str) -> bool {
        if self.flags.contains(UserFlags::ALL_CHANNELS) {
            return true;
        }

        // Check against channel patterns
        for pattern in &self.channel_patterns {
            if Self::matches_pattern(channel, pattern) {
                return true;
            }
        }

        false
    }

    /// Simple glob-style pattern matching
    fn matches_pattern(text: &str, pattern: &str) -> bool {
        // Support * wildcard
        if pattern == "*" {
            return true;
        }

        if !pattern.contains('*') {
            return text == pattern;
        }

        // Simple implementation: support * at the end
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            return text.starts_with(prefix);
        }

        // Support * at the beginning
        if pattern.starts_with('*') {
            let suffix = &pattern[1..];
            return text.ends_with(suffix);
        }

        // For more complex patterns, use exact match for now
        text == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_user() {
        let user = User::new("alice");
        assert_eq!(user.username, "alice");
        assert!(!user.is_enabled());
        assert!(user.passwords.is_empty());
    }

    #[test]
    fn test_default_user() {
        let user = User::default_user();
        assert_eq!(user.username, "default");
        assert!(user.is_enabled());
        assert!(user.flags.contains(UserFlags::ALL_COMMANDS));
        assert!(user.flags.contains(UserFlags::ALL_KEYS));
    }

    #[test]
    fn test_password() {
        let mut user = User::new("alice");
        user.add_password("secret");
        assert!(user.verify_password("secret"));
        assert!(!user.verify_password("wrong"));
    }

    #[test]
    fn test_key_patterns() {
        let mut user = User::new("alice");
        user.add_key_pattern("user:*");
        user.add_key_pattern("session:123");

        assert!(user.can_access_key("user:alice"));
        assert!(user.can_access_key("user:bob"));
        assert!(user.can_access_key("session:123"));
        assert!(!user.can_access_key("admin:data"));
    }

    #[test]
    fn test_enable_disable() {
        let mut user = User::new("alice");
        assert!(!user.is_enabled());

        user.enable();
        assert!(user.is_enabled());

        user.disable();
        assert!(!user.is_enabled());
    }
}
