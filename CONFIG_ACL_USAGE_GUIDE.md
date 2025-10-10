# Redis-Rust 配置和 ACL 使用指南

本指南介绍如何使用 redis-rust 的配置管理和访问控制列表（ACL）功能。

## 快速开始

### 1. 使用配置文件启动

创建配置文件 `my_redis.conf`:

```conf
# 基本配置
port 6379
bind 127.0.0.1
databases 16

# 持久化
appendonly yes
appendfilename "appendonly.aof"
appendfsync everysec

# 内存限制
maxmemory 1gb
maxmemory-policy allkeys-lru

# 安全
requirepass mypassword

# ACL 配置文件
aclfile users.acl
```

### 2. 运行测试

```bash
# 运行所有配置测试
cargo test --test config_integration_test

# 运行所有 ACL 测试
cargo test --test acl_integration_test

# 运行特定测试
cargo test test_config_get
cargo test test_acl_authenticate
```

## CONFIG 命令使用

### CONFIG GET - 获取配置参数

```redis
# 获取单个配置
CONFIG GET port
> 1) "port"
> 2) "6379"

# 使用通配符获取多个配置
CONFIG GET max*
> 1) "maxclients"
> 2) "10000"
> 3) "maxmemory"
> 4) "1073741824"
> 5) "maxmemory-policy"
> 6) "allkeys-lru"

# 获取所有配置
CONFIG GET *
```

### CONFIG SET - 设置配置参数

```redis
# 设置超时时间
CONFIG SET timeout 300
> OK

# 设置最大内存
CONFIG SET maxmemory 2000000
> OK

# 设置内存驱逐策略
CONFIG SET maxmemory-policy volatile-lru
> OK

# 尝试设置只读参数（会失败）
CONFIG SET port 6380
> (error) ERR Configuration parameter 'port' cannot be changed at runtime
```

### CONFIG REWRITE - 持久化配置

```redis
# 将当前配置写入配置文件
CONFIG REWRITE
> OK
```

### CONFIG RESETSTAT - 重置统计

```redis
CONFIG RESETSTAT
> OK
```

## ACL 命令使用

### ACL LIST - 查看所有用户

```redis
ACL LIST
> 1) "user default on allkeys allcommands allchannels nopass"
```

### ACL USERS - 列出用户名

```redis
ACL USERS
> 1) "default"
```

### ACL SETUSER - 创建/修改用户

```redis
# 创建一个只读用户
ACL SETUSER readonly on >password allkeys +@read -@write
> OK

# 创建一个受限用户（只能访问特定键模式）
ACL SETUSER app_user on >secret ~app:* +@all
> OK

# 创建管理员用户
ACL SETUSER admin on >adminpass allkeys +@all
> OK

# 禁用用户
ACL SETUSER guest off
> OK

# 创建无密码用户（不推荐）
ACL SETUSER test on nopass allkeys +@read
> OK
```

### ACL 规则语法

| 规则 | 说明 | 示例 |
|------|------|------|
| `on` | 启用用户 | `ACL SETUSER alice on` |
| `off` | 禁用用户 | `ACL SETUSER alice off` |
| `>password` | 添加密码 | `ACL SETUSER alice >secret123` |
| `nopass` | 无需密码 | `ACL SETUSER alice nopass` |
| `allkeys` | 访问所有键 | `ACL SETUSER alice allkeys` |
| `~pattern` | 允许访问匹配模式的键 | `ACL SETUSER alice ~user:*` |
| `+@category` | 允许命令类别 | `ACL SETUSER alice +@read` |
| `-@category` | 拒绝命令类别 | `ACL SETUSER alice -@dangerous` |
| `+command` | 允许特定命令 | `ACL SETUSER alice +get` |
| `-command` | 拒绝特定命令 | `ACL SETUSER alice -flushdb` |
| `allcommands` | 允许所有命令 | `ACL SETUSER alice allcommands` |
| `allchannels` | 访问所有 Pub/Sub 频道 | `ACL SETUSER alice allchannels` |

### ACL GETUSER - 查看用户详情

```redis
ACL GETUSER alice
> 1) "flags"
> 2) 1) "on"
>    2) "allkeys"
> 3) "passwords"
> 4) 1) "1 password(s) set"
> 5) "commands"
> 6) "+@all"
> 7) "keys"
> 8) 1) "*"
```

### ACL DELUSER - 删除用户

```redis
# 删除单个用户
ACL DELUSER alice
> (integer) 1

# 删除多个用户
ACL DELUSER user1 user2 user3
> (integer) 3

# 注意：不能删除 default 用户
ACL DELUSER default
> (error) ERR Cannot delete default user
```

### ACL CAT - 查看命令类别

```redis
# 列出所有类别
ACL CAT
> 1) "@keyspace"
> 2) "@read"
> 3) "@write"
> 4) "@set"
> 5) "@sortedset"
> ...

# 查看某个类别包含的命令
ACL CAT @read
> 1) "GET"
> 2) "MGET"
> 3) "STRLEN"
> 4) "EXISTS"
> ...
```

### ACL WHOAMI - 查看当前用户

```redis
ACL WHOAMI
> "default"
```

### ACL SAVE/LOAD - 持久化 ACL

```redis
# 保存 ACL 到文件
ACL SAVE
> OK

# 从文件重新加载 ACL
ACL LOAD
> OK
```

## 命令类别参考

| 类别 | 说明 |
|------|------|
| `@keyspace` | 键空间操作 |
| `@read` | 只读命令（GET, MGET等） |
| `@write` | 写入命令（SET, DEL等） |
| `@string` | 字符串命令 |
| `@list` | 列表命令 |
| `@set` | 集合命令 |
| `@sortedset` | 有序集合命令 |
| `@hash` | 哈希表命令 |
| `@bitmap` | 位图命令 |
| `@hyperloglog` | HyperLogLog 命令 |
| `@geo` | 地理位置命令 |
| `@stream` | 流命令 |
| `@pubsub` | 发布/订阅命令 |
| `@transaction` | 事务命令 |
| `@scripting` | 脚本命令 |
| `@admin` | 管理命令 |
| `@dangerous` | 危险命令（FLUSHDB, FLUSHALL等） |
| `@connection` | 连接命令 |
| `@fast` | 快速命令（O(1)复杂度） |
| `@slow` | 慢速命令 |
| `@all` | 所有命令 |

## 实际使用场景

### 场景 1: Web 应用后端

```redis
# 创建应用用户，只能访问应用相关的键
ACL SETUSER webapp on >webapp_secret ~app:* ~session:* ~cache:* +@read +@write -@dangerous
```

### 场景 2: 只读分析服务

```redis
# 创建只读用户用于数据分析
ACL SETUSER analyst on >analyst_pass allkeys +@read -@write -@admin
```

### 场景 3: 监控服务

```redis
# 创建监控用户，只能执行 INFO 和 PING 命令
ACL SETUSER monitor on >monitor_pass allkeys +info +ping +config
```

### 场景 4: 缓存服务

```redis
# 创建缓存用户，只能访问缓存键并有过期时间控制
ACL SETUSER cache_user on >cache_secret ~cache:* +get +set +del +expire +ttl
```

## 配置文件示例

### 完整的 redis.conf

查看项目根目录的 `redis.conf` 文件，包含所有可用配置及详细说明。

### ACL 配置文件 (users.acl)

```json
{
  "users": [
    {
      "username": "default",
      "passwords": [],
      "flags": {
        "bits": 31
      },
      "permissions": [],
      "key_patterns": [],
      "channel_patterns": []
    },
    {
      "username": "admin",
      "passwords": ["5baa61e4c9b93f3f0682250b6cf8331b7ee68fd8"],
      "flags": {
        "bits": 15
      },
      "permissions": [],
      "key_patterns": [],
      "channel_patterns": []
    }
  ]
}
```

## 编程接口

### Rust API 示例

```rust
use redis_rust::config::ConfigManager;
use redis_rust::acl::{Acl, User, Permission, CommandCategory};

// 加载配置
let config = ConfigManager::from_file("redis.conf")?;

// 动态修改配置
config.set("timeout".to_string(), "300".to_string())?;
config.rewrite()?;

// 创建 ACL 管理器
let acl = Acl::new();

// 创建用户
let mut user = User::new("alice");
user.enable();
user.add_password("secret");
user.add_key_pattern("user:*");
user.add_permission(Permission::AllowCategory(CommandCategory::Read));

// 添加用户
acl.add_user(user)?;

// 认证
let authenticated_user = acl.authenticate("alice", "secret")?;

// 检查权限
acl.check_permission(&authenticated_user, "GET", &["user:123".to_string()])?;
```

## 安全最佳实践

1. **不要使用 default 用户**: 为每个应用创建专用用户
2. **使用强密码**: 密码应该足够复杂且唯一
3. **最小权限原则**: 只授予必要的权限
4. **定期审查**: 定期检查和更新用户权限
5. **禁用不用的用户**: 使用 `off` 标志禁用不再使用的用户
6. **限制键访问**: 使用键模式限制用户只能访问特定数据
7. **避免危险命令**: 对大多数用户禁用 `@dangerous` 类别
8. **持久化 ACL**: 定期使用 `ACL SAVE` 保存配置

## 故障排查

### 配置问题

```redis
# 查看当前配置
CONFIG GET *

# 重置到默认值
CONFIG SET <parameter> <default_value>

# 重新加载配置文件
# (需要重启服务器)
```

### ACL 问题

```redis
# 检查用户状态
ACL GETUSER <username>

# 检查当前用户
ACL WHOAMI

# 查看所有用户
ACL LIST

# 重新加载 ACL
ACL LOAD
```

### 常见错误

1. **"Configuration parameter cannot be changed"**: 尝试修改只读配置
2. **"User not found"**: 用户不存在
3. **"Authentication failed"**: 密码错误
4. **"User is disabled"**: 用户被禁用
5. **"Permission denied"**: 没有执行该命令的权限
6. **"Cannot access key"**: 没有访问该键的权限

## 性能考虑

1. **配置读取**: 配置读取是线程安全且高性能的
2. **ACL 检查开销**: 每个命令都会进行 ACL 检查，建议合理设置权限规则数量
3. **密码哈希**: 使用 SHA1 哈希，认证性能良好
4. **键模式匹配**: 简单的通配符匹配，性能开销小

## 更多信息

- [实现总结文档](CONFIG_ACL_IMPLEMENTATION.md)
- [Redis 官方 ACL 文档](https://redis.io/docs/management/security/acl/)
- [Redis 官方配置文档](https://redis.io/docs/management/config/)
