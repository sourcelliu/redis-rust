// Integration tests for ACL functionality

use redis_rust::acl::{Acl, User, Permission, CommandCategory, UserFlags, AclManager};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_acl_manager_default() {
    let manager = AclManager::new();

    assert_eq!(manager.user_count(), 1);
    assert!(manager.get_user("default").is_some());
}

#[test]
fn test_acl_add_user() {
    let manager = AclManager::new();

    let mut user = User::new("alice");
    user.enable();
    user.add_password("secret");

    manager.add_user(user).unwrap();
    assert_eq!(manager.user_count(), 2);

    let alice = manager.get_user("alice").unwrap();
    assert_eq!(alice.username, "alice");
    assert!(alice.is_enabled());
}

#[test]
fn test_acl_authenticate() {
    let manager = AclManager::new();

    let mut user = User::new("bob");
    user.enable();
    user.add_password("password123");
    manager.add_user(user).unwrap();

    let result = manager.authenticate("bob", "password123");
    assert!(result.is_ok());

    let result = manager.authenticate("bob", "wrongpassword");
    assert!(result.is_err());
}

#[test]
fn test_acl_delete_user() {
    let manager = AclManager::new();

    let mut user = User::new("temp");
    user.enable();
    manager.add_user(user).unwrap();

    assert_eq!(manager.user_count(), 2);

    manager.delete_user("temp").unwrap();
    assert_eq!(manager.user_count(), 1);
    assert!(manager.get_user("temp").is_none());
}

#[test]
fn test_acl_cannot_delete_default() {
    let manager = AclManager::new();

    let result = manager.delete_user("default");
    assert!(result.is_err());
}

#[test]
fn test_user_password_management() {
    let mut user = User::new("alice");

    user.add_password("password1");
    assert!(user.verify_password("password1"));
    assert!(!user.verify_password("password2"));

    user.add_password("password2");
    assert!(user.verify_password("password1"));
    assert!(user.verify_password("password2"));

    user.remove_password("password1");
    assert!(!user.verify_password("password1"));
    assert!(user.verify_password("password2"));
}

#[test]
fn test_user_flags() {
    let mut user = User::new("bob");

    assert!(!user.is_enabled());

    user.enable();
    assert!(user.is_enabled());

    user.disable();
    assert!(!user.is_enabled());

    user.grant_all_commands();
    assert!(user.flags.contains(UserFlags::ALL_COMMANDS));

    user.grant_all_keys();
    assert!(user.flags.contains(UserFlags::ALL_KEYS));
}

#[test]
fn test_user_key_patterns() {
    let mut user = User::new("alice");

    user.add_key_pattern("user:*");
    user.add_key_pattern("session:123");

    assert!(user.can_access_key("user:alice"));
    assert!(user.can_access_key("user:bob"));
    assert!(user.can_access_key("session:123"));
    assert!(!user.can_access_key("admin:data"));

    user.grant_all_keys();
    assert!(user.can_access_key("admin:data"));
}

#[test]
fn test_user_permissions() {
    let mut user = User::new("alice");
    user.enable();
    user.grant_all_keys();

    user.add_permission(Permission::AllowCategory(CommandCategory::Read));
    user.add_permission(Permission::DenyCommand("FLUSHDB".to_string()));

    let manager = AclManager::new();
    manager.add_user(user.clone()).unwrap();

    // Should allow read commands
    let result = manager.check_permission(&user, "GET", &["key1".to_string()]);
    assert!(result.is_ok());

    // Should deny FLUSHDB
    let result = manager.check_permission(&user, "FLUSHDB", &[]);
    assert!(result.is_err());
}

#[test]
fn test_command_category() {
    let cat = CommandCategory::Read;

    assert!(cat.contains_command("GET"));
    assert!(cat.contains_command("MGET"));
    assert!(!cat.contains_command("SET"));

    let cat = CommandCategory::Write;
    assert!(cat.contains_command("SET"));
    assert!(cat.contains_command("DEL"));
    assert!(!cat.contains_command("GET"));
}

#[test]
fn test_permission_allows() {
    let perm = Permission::AllowCommand("GET".to_string());
    assert_eq!(perm.allows("GET"), Some(true));
    assert_eq!(perm.allows("SET"), None);

    let perm = Permission::DenyCommand("FLUSHDB".to_string());
    assert_eq!(perm.allows("FLUSHDB"), Some(false));
    assert_eq!(perm.allows("GET"), None);

    let perm = Permission::AllowCategory(CommandCategory::Read);
    assert_eq!(perm.allows("GET"), Some(true));
    assert_eq!(perm.allows("SET"), None);
}

#[test]
fn test_acl_save_load() {
    let temp_dir = TempDir::new().unwrap();
    let acl_path = temp_dir.path().join("users.acl");

    let manager = AclManager::new();

    let mut user = User::new("alice");
    user.enable();
    user.add_password("secret");
    user.grant_all_keys();
    user.grant_all_commands();
    manager.add_user(user).unwrap();

    // Save to file
    manager.save_to_file(acl_path.to_str().unwrap()).unwrap();

    // Load from file
    let loaded_manager = AclManager::from_file(acl_path.to_str().unwrap()).unwrap();

    assert_eq!(loaded_manager.user_count(), 2);
    assert!(loaded_manager.get_user("alice").is_some());
}

#[test]
fn test_acl_check_permission_with_keys() {
    let manager = AclManager::new();

    let mut user = User::new("restricted");
    user.enable();
    user.grant_all_commands();
    user.add_key_pattern("user:*");
    manager.add_user(user.clone()).unwrap();

    // Should allow access to keys matching pattern
    let result = manager.check_permission(&user, "GET", &["user:123".to_string()]);
    assert!(result.is_ok());

    // Should deny access to keys not matching pattern
    let result = manager.check_permission(&user, "GET", &["admin:123".to_string()]);
    assert!(result.is_err());
}

#[test]
fn test_acl_disabled_user() {
    let manager = AclManager::new();

    let mut user = User::new("disabled");
    user.add_password("password");
    // User is not enabled
    manager.add_user(user.clone()).unwrap();

    // Should fail to authenticate disabled user
    let result = manager.authenticate("disabled", "password");
    assert!(result.is_err());

    // Should fail to check permission for disabled user
    let result = manager.check_permission(&user, "GET", &["key".to_string()]);
    assert!(result.is_err());
}

#[test]
fn test_acl_update_user() {
    let manager = AclManager::new();

    let mut user = User::new("updatable");
    user.enable();
    user.add_password("password");
    manager.add_user(user.clone()).unwrap();

    // Update user
    user.add_password("newpassword");
    user.grant_all_keys();
    manager.update_user(user).unwrap();

    // Verify update
    let updated = manager.get_user("updatable").unwrap();
    assert!(updated.verify_password("newpassword"));
    assert!(updated.flags.contains(UserFlags::ALL_KEYS));
}

#[test]
fn test_default_user() {
    let user = User::default_user();

    assert_eq!(user.username, "default");
    assert!(user.is_enabled());
    assert!(user.flags.contains(UserFlags::ALL_COMMANDS));
    assert!(user.flags.contains(UserFlags::ALL_KEYS));
    assert!(user.flags.contains(UserFlags::ALL_CHANNELS));
    assert!(user.flags.contains(UserFlags::NO_PASS));
}

#[test]
fn test_acl_list_users() {
    let manager = AclManager::new();

    let mut user1 = User::new("alice");
    user1.enable();
    manager.add_user(user1).unwrap();

    let mut user2 = User::new("bob");
    user2.enable();
    manager.add_user(user2).unwrap();

    let users = manager.list_users();
    assert_eq!(users.len(), 3); // default + alice + bob
    assert!(users.contains(&"default".to_string()));
    assert!(users.contains(&"alice".to_string()));
    assert!(users.contains(&"bob".to_string()));
}

#[test]
fn test_user_nopass() {
    let mut user = User::new("nopass_user");
    user.enable();

    // Initially has passwords
    user.add_password("temp");
    assert!(!user.flags.contains(UserFlags::NO_PASS));

    // Remove all passwords
    user.remove_all_passwords();
    assert!(user.flags.contains(UserFlags::NO_PASS));

    // Should verify any password when NO_PASS is set
    assert!(user.verify_password("anything"));
    assert!(user.verify_password(""));
}
