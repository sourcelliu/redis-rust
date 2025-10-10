// ACL command implementation
// Provides access control list management similar to Redis ACL commands

use crate::acl::{Acl, User, UserFlags, Permission, CommandCategory};
use crate::protocol::RespValue;
use std::sync::Arc;

pub struct AclCommands {
    acl: Arc<Acl>,
}

impl AclCommands {
    pub fn new(acl: Arc<Acl>) -> Self {
        Self { acl }
    }

    /// Execute an ACL command
    pub fn execute(&self, args: &[String]) -> Result<RespValue, String> {
        if args.is_empty() {
            return Err("ERR wrong number of arguments for 'acl' command".to_string());
        }

        let subcommand = args[0].to_uppercase();

        match subcommand.as_str() {
            "LIST" => self.acl_list(),
            "USERS" => self.acl_users(),
            "GETUSER" => self.acl_getuser(&args[1..]),
            "SETUSER" => self.acl_setuser(&args[1..]),
            "DELUSER" => self.acl_deluser(&args[1..]),
            "CAT" => self.acl_cat(&args[1..]),
            "WHOAMI" => self.acl_whoami(),
            "LOAD" => self.acl_load(),
            "SAVE" => self.acl_save(),
            "HELP" => Ok(self.acl_help()),
            _ => Err(format!("ERR unknown ACL subcommand '{}'", subcommand)),
        }
    }

    /// ACL LIST
    /// List all ACL rules for all users
    fn acl_list(&self) -> Result<RespValue, String> {
        let usernames = self.acl.list_users();
        let mut result = Vec::new();

        for username in usernames {
            if let Some(user) = self.acl.get_user(&username) {
                result.push(RespValue::BulkString(Self::format_user_rules(&user)));
            }
        }

        Ok(RespValue::Array(result))
    }

    /// ACL USERS
    /// List all usernames
    fn acl_users(&self) -> Result<RespValue, String> {
        let usernames = self.acl.list_users();
        let result: Vec<RespValue> = usernames
            .into_iter()
            .map(RespValue::BulkString)
            .collect();

        Ok(RespValue::Array(result))
    }

    /// ACL GETUSER username
    /// Get detailed information about a user
    fn acl_getuser(&self, args: &[String]) -> Result<RespValue, String> {
        if args.is_empty() {
            return Err("ERR wrong number of arguments for 'acl getuser' command".to_string());
        }

        let username = &args[0];
        let user = self
            .acl
            .get_user(username)
            .ok_or_else(|| format!("ERR User '{}' not found", username))?;

        let mut result = Vec::new();

        // Flags
        result.push(RespValue::BulkString("flags".to_string()));
        result.push(RespValue::Array(
            Self::format_user_flags(&user)
                .into_iter()
                .map(RespValue::BulkString)
                .collect(),
        ));

        // Passwords
        result.push(RespValue::BulkString("passwords".to_string()));
        result.push(RespValue::Array(vec![
            RespValue::BulkString(format!("{} password(s) set", user.passwords.len()))
        ]));

        // Commands
        result.push(RespValue::BulkString("commands".to_string()));
        result.push(RespValue::BulkString(
            if user.flags.contains(UserFlags::ALL_COMMANDS) {
                "+@all".to_string()
            } else {
                format!("{} permission(s)", user.permissions.len())
            },
        ));

        // Keys
        result.push(RespValue::BulkString("keys".to_string()));
        result.push(RespValue::Array(
            if user.flags.contains(UserFlags::ALL_KEYS) {
                vec![RespValue::BulkString("*".to_string())]
            } else {
                user.key_patterns
                    .iter()
                    .map(|p| RespValue::BulkString(p.clone()))
                    .collect()
            },
        ));

        Ok(RespValue::Array(result))
    }

    /// ACL SETUSER username [rules...]
    /// Create or modify a user
    fn acl_setuser(&self, args: &[String]) -> Result<RespValue, String> {
        if args.is_empty() {
            return Err("ERR wrong number of arguments for 'acl setuser' command".to_string());
        }

        let username = &args[0];
        let rules = &args[1..];

        // Get existing user or create new one
        let mut user = self
            .acl
            .get_user(username)
            .map(|u| (*u).clone())
            .unwrap_or_else(|| User::new(username));

        // Apply rules
        for rule in rules {
            Self::apply_rule(&mut user, rule)?;
        }

        // Add or update user
        if self.acl.get_user(username).is_some() {
            self.acl
                .manager()
                .update_user(user)
                .map_err(|e| format!("ERR {}", e))?;
        } else {
            self.acl
                .add_user(user)
                .map_err(|e| format!("ERR {}", e))?;
        }

        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// ACL DELUSER username [username ...]
    /// Delete users
    fn acl_deluser(&self, args: &[String]) -> Result<RespValue, String> {
        if args.is_empty() {
            return Err("ERR wrong number of arguments for 'acl deluser' command".to_string());
        }

        let mut deleted = 0;
        for username in args {
            if self.acl.delete_user(username).is_ok() {
                deleted += 1;
            }
        }

        Ok(RespValue::Integer(deleted))
    }

    /// ACL CAT [category]
    /// List command categories or commands in a category
    fn acl_cat(&self, args: &[String]) -> Result<RespValue, String> {
        if args.is_empty() {
            // List all categories
            let categories = vec![
                "@keyspace", "@read", "@write", "@set", "@sortedset", "@list",
                "@hash", "@string", "@bitmap", "@hyperloglog", "@geo", "@stream",
                "@pubsub", "@admin", "@fast", "@slow", "@dangerous", "@connection",
                "@transaction", "@scripting", "@all",
            ];

            let result: Vec<RespValue> = categories
                .into_iter()
                .map(|c| RespValue::BulkString(c.to_string()))
                .collect();

            Ok(RespValue::Array(result))
        } else {
            // List commands in category
            let category_name = &args[0];
            let category = CommandCategory::from_str(category_name)
                .ok_or_else(|| format!("ERR Unknown category '{}'", category_name))?;

            let commands: Vec<RespValue> = category
                .commands()
                .into_iter()
                .map(|c| RespValue::BulkString(c.to_string()))
                .collect();

            Ok(RespValue::Array(commands))
        }
    }

    /// ACL WHOAMI
    /// Return the current username
    fn acl_whoami(&self) -> Result<RespValue, String> {
        // In a real implementation, this would return the authenticated user
        Ok(RespValue::BulkString("default".to_string()))
    }

    /// ACL LOAD
    /// Reload ACL configuration from file
    fn acl_load(&self) -> Result<RespValue, String> {
        // Placeholder - would reload from ACL file
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// ACL SAVE
    /// Save ACL configuration to file
    fn acl_save(&self) -> Result<RespValue, String> {
        // In a real implementation, this would save to the ACL file
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// ACL HELP
    /// Show help for ACL command
    fn acl_help(&self) -> RespValue {
        let help_messages = vec![
            "ACL <subcommand> [<arg> [value] [opt] ...]. Subcommands are:",
            "LIST",
            "    List all users and their rules.",
            "USERS",
            "    List all usernames.",
            "GETUSER <username>",
            "    Get details about a specific user.",
            "SETUSER <username> [rules...]",
            "    Create or modify a user with ACL rules.",
            "DELUSER <username> [username ...]",
            "    Delete one or more users.",
            "CAT [category]",
            "    List command categories or commands in a category.",
            "WHOAMI",
            "    Return the current username.",
            "LOAD",
            "    Reload ACL rules from the configured ACL file.",
            "SAVE",
            "    Save the current ACL rules to the configured file.",
            "HELP",
            "    Print this help.",
        ];

        RespValue::Array(
            help_messages
                .iter()
                .map(|s| RespValue::BulkString(s.to_string()))
                .collect(),
        )
    }

    /// Format user rules for ACL LIST
    fn format_user_rules(user: &User) -> String {
        let mut parts = vec![format!("user {}", user.username)];

        // Flags
        if user.is_enabled() {
            parts.push("on".to_string());
        } else {
            parts.push("off".to_string());
        }

        if user.flags.contains(UserFlags::NO_PASS) {
            parts.push("nopass".to_string());
        }

        // Commands
        if user.flags.contains(UserFlags::ALL_COMMANDS) {
            parts.push("+@all".to_string());
        }

        // Keys
        if user.flags.contains(UserFlags::ALL_KEYS) {
            parts.push("~*".to_string());
        } else {
            for pattern in &user.key_patterns {
                parts.push(format!("~{}", pattern));
            }
        }

        parts.join(" ")
    }

    /// Format user flags
    fn format_user_flags(user: &User) -> Vec<String> {
        let mut flags = Vec::new();

        if user.is_enabled() {
            flags.push("on".to_string());
        } else {
            flags.push("off".to_string());
        }

        if user.flags.contains(UserFlags::ALL_COMMANDS) {
            flags.push("allcommands".to_string());
        }

        if user.flags.contains(UserFlags::ALL_KEYS) {
            flags.push("allkeys".to_string());
        }

        if user.flags.contains(UserFlags::ALL_CHANNELS) {
            flags.push("allchannels".to_string());
        }

        if user.flags.contains(UserFlags::NO_PASS) {
            flags.push("nopass".to_string());
        }

        flags
    }

    /// Apply an ACL rule to a user
    fn apply_rule(user: &mut User, rule: &str) -> Result<(), String> {
        match rule {
            "on" => user.enable(),
            "off" => user.disable(),
            "nopass" => user.remove_all_passwords(),
            "allcommands" | "+@all" => user.grant_all_commands(),
            "allkeys" => user.grant_all_keys(),
            "allchannels" => user.grant_all_channels(),
            rule if rule.starts_with('>') => {
                // Add password
                user.add_password(&rule[1..]);
            }
            rule if rule.starts_with('~') => {
                // Add key pattern
                user.add_key_pattern(&rule[1..]);
            }
            rule if rule.starts_with('+') => {
                // Add command or category permission
                if rule.starts_with("+@") {
                    if let Some(cat) = CommandCategory::from_str(&rule[1..]) {
                        user.add_permission(Permission::AllowCategory(cat));
                    }
                } else {
                    user.add_permission(Permission::AllowCommand(rule[1..].to_string()));
                }
            }
            rule if rule.starts_with('-') => {
                // Deny command or category
                if rule.starts_with("-@") {
                    if let Some(cat) = CommandCategory::from_str(&rule[1..]) {
                        user.add_permission(Permission::DenyCategory(cat));
                    }
                } else {
                    user.add_permission(Permission::DenyCommand(rule[1..].to_string()));
                }
            }
            _ => return Err(format!("ERR Invalid ACL rule '{}'", rule)),
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_users() {
        let acl = Arc::new(Acl::new());
        let cmd = AclCommands::new(acl.clone());

        let result = cmd.acl_users();
        assert!(result.is_ok());
    }

    #[test]
    fn test_acl_setuser() {
        let acl = Arc::new(Acl::new());
        let cmd = AclCommands::new(acl.clone());

        let result = cmd.acl_setuser(&[
            "alice".to_string(),
            "on".to_string(),
            ">password".to_string(),
            "allkeys".to_string(),
            "+@all".to_string(),
        ]);

        assert!(result.is_ok());
        assert!(acl.get_user("alice").is_some());
    }

    #[test]
    fn test_acl_cat() {
        let acl = Arc::new(Acl::new());
        let cmd = AclCommands::new(acl.clone());

        let result = cmd.acl_cat(&[]);
        assert!(result.is_ok());

        let result = cmd.acl_cat(&["@read".to_string()]);
        assert!(result.is_ok());
    }
}
