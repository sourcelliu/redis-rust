// ACL Manager - Core ACL management logic

use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::Arc;
use std::fs;
use anyhow::Result;
use thiserror::Error;
use serde::{Deserialize, Serialize};

use super::user::{User, UserFlags};
use super::permission::Permission;

#[derive(Error, Debug)]
pub enum AclError {
    #[error("User '{0}' not found")]
    UserNotFound(String),

    #[error("User '{0}' already exists")]
    UserAlreadyExists(String),

    #[error("Authentication failed for user '{0}'")]
    AuthenticationFailed(String),

    #[error("User '{0}' is disabled")]
    UserDisabled(String),

    #[error("Permission denied: cannot execute command '{0}'")]
    PermissionDenied(String),

    #[error("Permission denied: cannot access key '{0}'")]
    KeyAccessDenied(String),

    #[error("ACL file error: {0}")]
    FileError(String),
}

/// ACL configuration that can be serialized
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AclConfig {
    users: Vec<User>,
}

/// Manages users and permissions
pub struct AclManager {
    /// Map of username to user
    users: RwLock<HashMap<String, Arc<User>>>,
}

impl AclManager {
    /// Create a new ACL manager with default user
    pub fn new() -> Self {
        let mut users = HashMap::new();
        let default_user = User::default_user();
        users.insert(default_user.username.clone(), Arc::new(default_user));

        Self {
            users: RwLock::new(users),
        }
    }

    /// Load ACL configuration from a file
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read ACL file: {}", e))?;

        let config: AclConfig = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse ACL file: {}", e))?;

        let mut users = HashMap::new();
        for user in config.users {
            users.insert(user.username.clone(), Arc::new(user));
        }

        // Ensure default user exists
        if !users.contains_key("default") {
            users.insert("default".to_string(), Arc::new(User::default_user()));
        }

        Ok(Self {
            users: RwLock::new(users),
        })
    }

    /// Authenticate a user with username and password
    pub fn authenticate(&self, username: &str, password: &str) -> Result<Arc<User>, AclError> {
        let users = self.users.read().unwrap();

        let user = users
            .get(username)
            .ok_or_else(|| AclError::UserNotFound(username.to_string()))?;

        if !user.is_enabled() {
            return Err(AclError::UserDisabled(username.to_string()));
        }

        if !user.verify_password(password) {
            return Err(AclError::AuthenticationFailed(username.to_string()));
        }

        Ok(Arc::clone(user))
    }

    /// Check if a user has permission to execute a command on specific keys
    pub fn check_permission(&self, user: &User, command: &str, keys: &[String]) -> Result<(), AclError> {
        // Check if user is enabled
        if !user.is_enabled() {
            return Err(AclError::UserDisabled(user.username.clone()));
        }

        // Check command permission
        if !self.check_command_permission(user, command) {
            return Err(AclError::PermissionDenied(command.to_string()));
        }

        // Check key access
        for key in keys {
            if !user.can_access_key(key) {
                return Err(AclError::KeyAccessDenied(key.clone()));
            }
        }

        Ok(())
    }

    /// Check if user can execute a command
    fn check_command_permission(&self, user: &User, command: &str) -> bool {
        // If user has ALL_COMMANDS flag, allow everything
        if user.flags.contains(UserFlags::ALL_COMMANDS) {
            return true;
        }

        // Check permissions in order (deny takes precedence)
        let mut allowed = false;

        for permission in &user.permissions {
            match permission.allows(command) {
                Some(true) => allowed = true,
                Some(false) => return false, // Explicit deny
                None => continue,
            }
        }

        allowed
    }

    /// Add a new user
    pub fn add_user(&self, user: User) -> Result<(), AclError> {
        let mut users = self.users.write().unwrap();

        if users.contains_key(&user.username) {
            return Err(AclError::UserAlreadyExists(user.username.clone()));
        }

        users.insert(user.username.clone(), Arc::new(user));
        Ok(())
    }

    /// Delete a user
    pub fn delete_user(&self, username: &str) -> Result<(), AclError> {
        // Cannot delete default user
        if username == "default" {
            return Err(AclError::FileError("Cannot delete default user".to_string()));
        }

        let mut users = self.users.write().unwrap();

        if !users.contains_key(username) {
            return Err(AclError::UserNotFound(username.to_string()));
        }

        users.remove(username);
        Ok(())
    }

    /// Get a user by username
    pub fn get_user(&self, username: &str) -> Option<Arc<User>> {
        let users = self.users.read().unwrap();
        users.get(username).map(Arc::clone)
    }

    /// Update a user
    pub fn update_user(&self, user: User) -> Result<(), AclError> {
        let mut users = self.users.write().unwrap();

        if !users.contains_key(&user.username) {
            return Err(AclError::UserNotFound(user.username.clone()));
        }

        users.insert(user.username.clone(), Arc::new(user));
        Ok(())
    }

    /// List all usernames
    pub fn list_users(&self) -> Vec<String> {
        let users = self.users.read().unwrap();
        users.keys().cloned().collect()
    }

    /// Get the number of users
    pub fn user_count(&self) -> usize {
        let users = self.users.read().unwrap();
        users.len()
    }

    /// Save ACL configuration to a file
    pub fn save_to_file(&self, path: &str) -> Result<(), AclError> {
        let users = self.users.read().unwrap();

        let config = AclConfig {
            users: users.values().map(|u| (**u).clone()).collect(),
        };

        let json = serde_json::to_string_pretty(&config)
            .map_err(|e| AclError::FileError(format!("Failed to serialize ACL: {}", e)))?;

        fs::write(path, json)
            .map_err(|e| AclError::FileError(format!("Failed to write ACL file: {}", e)))?;

        Ok(())
    }
}

impl Default for AclManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_manager() {
        let manager = AclManager::new();
        assert_eq!(manager.user_count(), 1);
        assert!(manager.get_user("default").is_some());
    }

    #[test]
    fn test_add_user() {
        let manager = AclManager::new();
        let mut user = User::new("alice");
        user.enable();
        user.add_password("secret");

        manager.add_user(user).unwrap();
        assert_eq!(manager.user_count(), 2);
    }

    #[test]
    fn test_authenticate() {
        let manager = AclManager::new();
        let mut user = User::new("alice");
        user.enable();
        user.add_password("secret");
        manager.add_user(user).unwrap();

        let result = manager.authenticate("alice", "secret");
        assert!(result.is_ok());

        let result = manager.authenticate("alice", "wrong");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_user() {
        let manager = AclManager::new();
        let mut user = User::new("alice");
        user.enable();
        manager.add_user(user).unwrap();

        manager.delete_user("alice").unwrap();
        assert_eq!(manager.user_count(), 1);

        // Cannot delete default user
        let result = manager.delete_user("default");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_permission() {
        let manager = AclManager::new();
        let mut user = User::new("alice");
        user.enable();
        user.add_password("secret");
        user.grant_all_commands();
        user.add_key_pattern("user:*");

        let result = manager.check_permission(&user, "GET", &["user:alice".to_string()]);
        assert!(result.is_ok());

        let result = manager.check_permission(&user, "GET", &["admin:data".to_string()]);
        assert!(result.is_err());
    }
}
