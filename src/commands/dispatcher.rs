// Command dispatcher

use crate::config::Config;
use crate::persistence::aof::AofManager;
use crate::protocol::RespValue;
use crate::pubsub::PubSub;
use crate::replication::{ReplicationInfo, ReplicationBacklog, CommandPropagator};
use crate::scripting::ScriptCache;
use crate::server::client_info::ClientRegistry;
use crate::server::slowlog::SlowLog;
use crate::storage::db::Database;
use crate::transaction::Transaction;
use std::sync::Arc;

pub struct CommandDispatcher;

impl CommandDispatcher {
    pub fn new() -> Self {
        Self
    }

    pub async fn dispatch(
        &self,
        db_index: &mut usize,
        db: &Arc<Database>,
        pubsub: &Arc<PubSub>,
        aof: &Arc<AofManager>,
        script_cache: &Arc<ScriptCache>,
        repl_info: &Arc<ReplicationInfo>,
        repl_backlog: &Arc<ReplicationBacklog>,
        propagator: &Arc<CommandPropagator>,
        client_registry: &Arc<ClientRegistry>,
        client_id: u64,
        slowlog: &Arc<SlowLog>,
        config: &Arc<Config>,
        tx: &mut Transaction,
        mut args: Vec<Vec<u8>>,
    ) -> RespValue {
        if args.is_empty() {
            return RespValue::Error("ERR empty command".to_string());
        }

        // Extract command name (case-insensitive)
        let cmd_bytes = args.remove(0);
        let cmd = match std::str::from_utf8(&cmd_bytes) {
            Ok(s) => s.to_uppercase(),
            Err(_) => return RespValue::Error("ERR invalid command name".to_string()),
        };

        // Dispatch to appropriate handler
        match cmd.as_str() {
            // Transaction commands
            "MULTI" => super::transaction_cmds::multi(tx).await,
            "EXEC" => super::transaction_cmds::exec(tx).await,
            "DISCARD" => super::transaction_cmds::discard(tx).await,
            "WATCH" => super::transaction_cmds::watch(tx, args).await,
            "UNWATCH" => super::transaction_cmds::unwatch(tx).await,

            // String commands
            "SET" => super::string::set(db, *db_index, args).await,
            "GET" => super::string::get(db, *db_index, args).await,
            "GETEX" => super::string::getex(db, *db_index, args).await,
            "GETDEL" => super::string::getdel(db, *db_index, args).await,
            "SETEX" => super::string::setex(db, *db_index, args).await,
            "SETNX" => super::string::setnx(db, *db_index, args).await,
            "DEL" => super::string::del(db, *db_index, args).await,
            "EXISTS" => super::string::exists(db, *db_index, args).await,
            "APPEND" => super::string::append(db, *db_index, args).await,
            "STRLEN" => super::string::strlen(db, *db_index, args).await,
            "INCR" => super::string::incr(db, *db_index, args).await,
            "DECR" => super::string::decr(db, *db_index, args).await,
            "INCRBY" => super::string::incrby(db, *db_index, args).await,
            "DECRBY" => super::string::decrby(db, *db_index, args).await,
            "INCRBYFLOAT" => super::string::incrbyfloat(db, *db_index, args).await,
            "PSETEX" => super::string::psetex(db, *db_index, args).await,
            "GETRANGE" => super::string::getrange(db, *db_index, args).await,
            "SETRANGE" => super::string::setrange(db, *db_index, args).await,
            "MGET" => super::string::mget(db, *db_index, args).await,
            "MSET" => super::string::mset(db, *db_index, args).await,
            "MSETNX" => super::string::msetnx(db, *db_index, args).await,

            // Bitmap commands
            "SETBIT" => super::bitmap::setbit(db, *db_index, args).await,
            "GETBIT" => super::bitmap::getbit(db, *db_index, args).await,
            "BITCOUNT" => super::bitmap::bitcount(db, *db_index, args).await,
            "BITPOS" => super::bitmap::bitpos(db, *db_index, args).await,
            "BITOP" => super::bitmap::bitop(db, *db_index, args).await,

            // HyperLogLog commands
            "PFADD" => super::hyperloglog::pfadd(db, *db_index, args).await,
            "PFCOUNT" => super::hyperloglog::pfcount(db, *db_index, args).await,
            "PFMERGE" => super::hyperloglog::pfmerge(db, *db_index, args).await,

            // Server commands
            "PING" => super::server_cmds::ping(args).await,
            "ECHO" => super::server_cmds::echo(args).await,
            "SELECT" => super::server_cmds::select(db_index, args).await,
            "FLUSHDB" => super::server_cmds::flushdb(db, *db_index).await,
            "FLUSHALL" => super::server_cmds::flushall(db).await,
            "DBSIZE" => super::server_cmds::dbsize(db, *db_index).await,
            "KEYS" => super::server_cmds::keys(db, *db_index, args).await,
            "SAVE" => super::server_cmds::save(db).await,
            "BGSAVE" => super::server_cmds::bgsave(db).await,
            "BGREWRITEAOF" => super::server_cmds::bgrewriteaof(db, aof).await,
            "INFO" => super::info_cmd::info(db, repl_info, args).await,
            "CLIENT" => super::admin_cmds::client(client_registry, client_id, args).await,
            "SLOWLOG" => super::admin_cmds::slowlog(slowlog, args).await,
            "COMMAND" => super::admin_cmds::command(args).await,
            "TIME" => super::server_cmds::time().await,
            "LASTSAVE" => super::server_cmds::lastsave().await,
            "TYPE" => super::server_cmds::key_type(db, *db_index, args).await,
            "RANDOMKEY" => super::server_cmds::randomkey(db, *db_index).await,
            "SHUTDOWN" => super::server_cmds::shutdown(db).await,
            "CONFIG" => {
                // Handle CONFIG subcommands
                if args.is_empty() {
                    RespValue::Error("ERR wrong number of arguments for 'config' command".to_string())
                } else {
                    let subcmd = match std::str::from_utf8(&args[0]) {
                        Ok(s) => s.to_uppercase(),
                        Err(_) => return RespValue::Error("ERR invalid subcommand".to_string()),
                    };
                    let rest_args = args[1..].to_vec();
                    match subcmd.as_str() {
                        "GET" => super::server_cmds::config_get(config, rest_args).await,
                        "SET" => super::server_cmds::config_set(config, rest_args).await,
                        _ => RespValue::Error(format!("ERR Unknown CONFIG subcommand '{}'", subcmd)),
                    }
                }
            }

            // List commands
            "LPUSH" => super::list::lpush(db, *db_index, args).await,
            "RPUSH" => super::list::rpush(db, *db_index, args).await,
            "LPOP" => super::list::lpop(db, *db_index, args).await,
            "RPOP" => super::list::rpop(db, *db_index, args).await,
            "LLEN" => super::list::llen(db, *db_index, args).await,
            "LRANGE" => super::list::lrange(db, *db_index, args).await,
            "LINDEX" => super::list::lindex(db, *db_index, args).await,
            "LSET" => super::list::lset(db, *db_index, args).await,
            "LTRIM" => super::list::ltrim(db, *db_index, args).await,
            "LREM" => super::list::lrem(db, *db_index, args).await,
            "LPUSHX" => super::list::lpushx(db, *db_index, args).await,
            "RPUSHX" => super::list::rpushx(db, *db_index, args).await,
            "RPOPLPUSH" => super::list::rpoplpush(db, *db_index, args).await,
            "BLPOP" => super::list::blpop(db, *db_index, args).await,
            "BRPOP" => super::list::brpop(db, *db_index, args).await,
            "BLMOVE" => super::list::blmove(db, *db_index, args).await,
            "LPOS" => super::list::lpos(db, *db_index, args).await,
            "LMOVE" => super::list::lmove(db, *db_index, args).await,

            // Hash commands
            "HSET" => super::hash::hset(db, *db_index, args).await,
            "HGET" => super::hash::hget(db, *db_index, args).await,
            "HDEL" => super::hash::hdel(db, *db_index, args).await,
            "HEXISTS" => super::hash::hexists(db, *db_index, args).await,
            "HGETALL" => super::hash::hgetall(db, *db_index, args).await,
            "HKEYS" => super::hash::hkeys(db, *db_index, args).await,
            "HVALS" => super::hash::hvals(db, *db_index, args).await,
            "HLEN" => super::hash::hlen(db, *db_index, args).await,
            "HMGET" => super::hash::hmget(db, *db_index, args).await,
            "HMSET" => super::hash::hmset(db, *db_index, args).await,
            "HSETNX" => super::hash::hsetnx(db, *db_index, args).await,
            "HINCRBY" => super::hash::hincrby(db, *db_index, args).await,
            "HINCRBYFLOAT" => super::hash::hincrbyfloat(db, *db_index, args).await,
            "HSTRLEN" => super::hash::hstrlen(db, *db_index, args).await,
            "HSCAN" => super::hash::hscan(db, *db_index, args).await,
            "HRANDFIELD" => super::hash::hrandfield(db, *db_index, args).await,

            // Set commands
            "SADD" => super::set::sadd(db, *db_index, args).await,
            "SREM" => super::set::srem(db, *db_index, args).await,
            "SMEMBERS" => super::set::smembers(db, *db_index, args).await,
            "SISMEMBER" => super::set::sismember(db, *db_index, args).await,
            "SCARD" => super::set::scard(db, *db_index, args).await,
            "SPOP" => super::set::spop(db, *db_index, args).await,
            "SRANDMEMBER" => super::set::srandmember(db, *db_index, args).await,
            "SINTER" => super::set::sinter(db, *db_index, args).await,
            "SUNION" => super::set::sunion(db, *db_index, args).await,
            "SDIFF" => super::set::sdiff(db, *db_index, args).await,
            "SINTERSTORE" => super::set::sinterstore(db, *db_index, args).await,
            "SUNIONSTORE" => super::set::sunionstore(db, *db_index, args).await,
            "SDIFFSTORE" => super::set::sdiffstore(db, *db_index, args).await,
            "SMOVE" => super::set::smove(db, *db_index, args).await,
            "SMISMEMBER" => super::set::smismember(db, *db_index, args).await,
            "SSCAN" => super::set::sscan(db, *db_index, args).await,

            // ZSet commands
            "ZADD" => super::zset::zadd(db, *db_index, args).await,
            "ZREM" => super::zset::zrem(db, *db_index, args).await,
            "ZSCORE" => super::zset::zscore(db, *db_index, args).await,
            "ZCARD" => super::zset::zcard(db, *db_index, args).await,
            "ZCOUNT" => super::zset::zcount(db, *db_index, args).await,
            "ZRANGE" => super::zset::zrange(db, *db_index, args).await,
            "ZREVRANGE" => super::zset::zrevrange(db, *db_index, args).await,
            "ZRANGEBYSCORE" => super::zset::zrangebyscore(db, *db_index, args).await,
            "ZRANK" => super::zset::zrank(db, *db_index, args).await,
            "ZREVRANK" => super::zset::zrevrank(db, *db_index, args).await,
            "ZINCRBY" => super::zset::zincrby(db, *db_index, args).await,
            "ZPOPMIN" => super::zset::zpopmin(db, *db_index, args).await,
            "ZPOPMAX" => super::zset::zpopmax(db, *db_index, args).await,
            "ZREMRANGEBYRANK" => super::zset::zremrangebyrank(db, *db_index, args).await,
            "ZREMRANGEBYSCORE" => super::zset::zremrangebyscore(db, *db_index, args).await,
            "BZPOPMIN" => super::zset::bzpopmin(db, *db_index, args).await,
            "BZPOPMAX" => super::zset::bzpopmax(db, *db_index, args).await,
            "ZMSCORE" => super::zset::zmscore(db, *db_index, args).await,
            "ZDIFF" => super::zset::zdiff(db, *db_index, args).await,
            "ZDIFFSTORE" => super::zset::zdiffstore(db, *db_index, args).await,
            "ZUNIONSTORE" => super::zset::zunionstore(db, *db_index, args).await,
            "ZINTERSTORE" => super::zset::zinterstore(db, *db_index, args).await,
            "ZREVRANGEBYSCORE" => super::zset::zrevrangebyscore(db, *db_index, args).await,
            "ZLEXCOUNT" => super::zset::zlexcount(db, *db_index, args).await,
            "ZRANGEBYLEX" => super::zset::zrangebylex(db, *db_index, args).await,
            "ZREVRANGEBYLEX" => super::zset::zrevrangebylex(db, *db_index, args).await,
            "ZREMRANGEBYLEX" => super::zset::zremrangebylex(db, *db_index, args).await,
            "ZSCAN" => super::zset::zscan(db, *db_index, args).await,

            // Geo commands
            "GEOADD" => super::geo::geoadd(db, *db_index, args).await,
            "GEOPOS" => super::geo::geopos(db, *db_index, args).await,
            "GEODIST" => super::geo::geodist(db, *db_index, args).await,
            "GEOHASH" => super::geo::geohash(db, *db_index, args).await,

            // Stream commands
            "XADD" => super::stream::xadd(db, *db_index, args).await,
            "XLEN" => super::stream::xlen(db, *db_index, args).await,
            "XRANGE" => super::stream::xrange(db, *db_index, args).await,
            "XREVRANGE" => super::stream::xrevrange(db, *db_index, args).await,
            "XDEL" => super::stream::xdel(db, *db_index, args).await,
            "XREAD" => super::stream::xread(db, *db_index, args).await,
            "XTRIM" => super::stream::xtrim(db, *db_index, args).await,

            // Expiration commands
            "EXPIRE" => super::expiration::expire(db, *db_index, args).await,
            "EXPIREAT" => super::expiration::expireat(db, *db_index, args).await,
            "PEXPIRE" => super::expiration::pexpire(db, *db_index, args).await,
            "PEXPIREAT" => super::expiration::pexpireat(db, *db_index, args).await,
            "TTL" => super::expiration::ttl(db, *db_index, args).await,
            "PTTL" => super::expiration::pttl(db, *db_index, args).await,
            "PERSIST" => super::expiration::persist(db, *db_index, args).await,

            // Pub/Sub commands (PUBLISH only - SUBSCRIBE handled separately)
            "PUBLISH" => super::pubsub_cmds::publish(pubsub, args).await,

            // Script commands
            "EVAL" => super::script_cmds::eval(db, *db_index, script_cache, args).await,
            "EVALSHA" => super::script_cmds::evalsha(db, *db_index, script_cache, args).await,
            "SCRIPT" => super::script_cmds::script(db, *db_index, script_cache, args).await,

            // Replication commands
            "REPLICAOF" | "SLAVEOF" => super::replication_cmds::replicaof(repl_info, repl_backlog, db, args).await,
            "ROLE" => super::replication_cmds::role(repl_info).await,
            "PSYNC" => super::replication_cmds::psync(repl_info, repl_backlog, args).await,
            "REPLCONF" => super::replication_cmds::replconf(propagator, args).await,
            "WAIT" => super::replication_cmds::wait(repl_info, propagator, args).await,

            // Key management commands
            "RENAME" => super::key_mgmt::rename(db, *db_index, args).await,
            "RENAMENX" => super::key_mgmt::renamenx(db, *db_index, args).await,
            "COPY" => super::key_mgmt::copy(db, *db_index, args).await,
            "MOVE" => super::key_mgmt::move_key(db, *db_index, args).await,
            "DUMP" => super::key_mgmt::dump(db, *db_index, args).await,
            "RESTORE" => super::key_mgmt::restore(db, *db_index, args).await,
            "SCAN" => super::key_mgmt::scan(db, *db_index, args).await,
            "TOUCH" => super::key_mgmt::touch(db, *db_index, args).await,
            "UNLINK" => super::key_mgmt::unlink(db, *db_index, args).await,
            "OBJECT" => super::key_mgmt::object(db, *db_index, args).await,

            // Cluster commands (Placeholder - requires cluster state integration)
            "CLUSTER" => {
                if args.is_empty() {
                    return RespValue::Error("ERR wrong number of arguments for 'cluster' command".to_string());
                }

                let subcommand_bytes = args.remove(0);
                let subcommand = match std::str::from_utf8(&subcommand_bytes) {
                    Ok(s) => s.to_uppercase(),
                    Err(_) => return RespValue::Error("ERR invalid subcommand".to_string()),
                };

                match subcommand.as_str() {
                    "KEYSLOT" => {
                        if args.len() != 1 {
                            return RespValue::Error("ERR wrong number of arguments for 'cluster keyslot'".to_string());
                        }
                        super::cluster::cluster_keyslot(&args[0])
                    }
                    _ => RespValue::Error(format!("ERR Unknown CLUSTER subcommand '{}'", subcommand)),
                }
            }

            _ => RespValue::Error(format!("ERR unknown command '{}'", cmd)),
        }
    }
}

impl Default for CommandDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
