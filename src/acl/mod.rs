// Access Control List (ACL) implementation
// Provides user authentication and authorization similar to Redis ACL

mod user;
mod permission;
mod acl_manager;

pub use user::{User, UserFlags};
pub use permission::{Permission, CommandCategory};
pub use acl_manager::{AclManager, AclError};

use std::sync::Arc;
use anyhow::Result;

/// ACL system for managing users and permissions
pub struct Acl {
    manager: Arc<AclManager>,
}

impl Acl {
    /// Create a new ACL system
    pub fn new() -> Self {
        Self {
            manager: Arc::new(AclManager::new()),
        }
    }

    /// Load ACL configuration from a file
    pub fn from_file(path: &str) -> Result<Self> {
        let manager = AclManager::from_file(path)?;
        Ok(Self {
            manager: Arc::new(manager),
        })
    }

    /// Authenticate a user
    pub fn authenticate(&self, username: &str, password: &str) -> Result<Arc<User>, AclError> {
        self.manager.authenticate(username, password)
    }

    /// Check if a user can execute a command
    pub fn check_permission(&self, user: &User, command: &str, keys: &[String]) -> Result<(), AclError> {
        self.manager.check_permission(user, command, keys)
    }

    /// Add a new user
    pub fn add_user(&self, user: User) -> Result<(), AclError> {
        self.manager.add_user(user)
    }

    /// Delete a user
    pub fn delete_user(&self, username: &str) -> Result<(), AclError> {
        self.manager.delete_user(username)
    }

    /// Get a user by username
    pub fn get_user(&self, username: &str) -> Option<Arc<User>> {
        self.manager.get_user(username)
    }

    /// List all users
    pub fn list_users(&self) -> Vec<String> {
        self.manager.list_users()
    }

    /// Save ACL configuration to a file
    pub fn save_to_file(&self, path: &str) -> Result<(), AclError> {
        self.manager.save_to_file(path)
    }

    /// Get the manager for direct access
    pub fn manager(&self) -> Arc<AclManager> {
        Arc::clone(&self.manager)
    }
}

impl Default for Acl {
    fn default() -> Self {
        Self::new()
    }
}
