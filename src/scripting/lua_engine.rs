// Lua script execution engine
// NOTE: Full Lua support requires compiling with the 'lua' feature
//       This is a stub implementation that returns errors when Lua is not available

use crate::protocol::RespValue;
use crate::storage::db::Database;
use std::sync::Arc;

/// Lua script execution engine (stub without mlua)
pub struct LuaEngine;

impl LuaEngine {
    /// Create a new Lua engine
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }

    /// Execute a Lua script
    pub async fn execute(
        &self,
        _script: &str,
        _keys: Vec<Vec<u8>>,
        _args: Vec<Vec<u8>>,
        _db: &Arc<Database>,
        _db_index: usize,
    ) -> anyhow::Result<RespValue> {
        Ok(RespValue::Error(
            "ERR Lua scripting support not enabled in this build. Please recompile with Lua runtime.".to_string(),
        ))
    }
}

impl Default for LuaEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create Lua engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lua_not_available() {
        let engine = LuaEngine::new().unwrap();
        let db = Arc::new(Database::new(16));

        let script = "return 'hello'";
        let result = engine
            .execute(script, vec![], vec![], &db, 0)
            .await
            .unwrap();

        match result {
            RespValue::Error(msg) => {
                assert!(msg.contains("not enabled"));
            }
            _ => panic!("Expected Error"),
        }
    }
}

// ==============================================================================
// FULL LUA IMPLEMENTATION (for reference - requires mlua dependency)
// ==============================================================================
// To enable Lua scripting:
// 1. Install Lua 5.4 or LuaJIT on your system
// 2. Uncomment mlua in Cargo.toml
// 3. Replace this stub implementation with the full implementation below
//
// Full implementation features:
// - redis.call() and redis.pcall() support
// - KEYS and ARGV table access
// - Proper type conversion between Lua and RESP
// - Error handling and status replies
// - Support for all Redis commands within scripts
//
// Example full implementation structure:
//
// pub struct LuaEngine {
//     lua: Lua,
// }
//
// impl LuaEngine {
//     pub fn new() -> anyhow::Result<Self> {
//         let lua = Lua::new();
//         // Set up redis table with call/pcall functions
//         Ok(Self { lua })
//     }
//
//     pub async fn execute(...) -> anyhow::Result<RespValue> {
//         // Set up KEYS and ARGV
//         // Execute script
//         // Convert result to RESP
//     }
// }
