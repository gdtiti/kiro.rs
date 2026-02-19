# 凭据管理架构

## Identity

**What**: 多凭据管理系统，支持优先级排序、故障转移、凭据级代理配置
**Purpose**: 实现高可用 API 请求，自动切换失效凭据，支持不同网络环境的凭据独立配置

## Core Components

| File | Symbol | Purpose |
| ---- | ------ | ------- |
| `src/kiro/model/credentials.rs:16` | KiroCredentials | 凭据数据结构，包含认证信息和配置 |
| `src/kiro/model/credentials.rs:121` | CredentialsConfig | 凭据配置解析（单对象/数组格式） |
| `src/kiro/token_manager.rs:482` | MultiTokenManager | 多凭据管理器，负责选择、切换、刷新 |
| `src/kiro/token_manager.rs:392` | CredentialEntry | 单个凭据运行时状态（失败计数、禁用原因） |
| `src/kiro/token_manager.rs:512` | CallContext | API 调用上下文，绑定凭据 ID、credentials、token |
| `src/admin/service.rs:35` | AdminService | Admin API 业务层，凭据 CRUD 和余额查询 |

## Credential Structure

### 核心字段

| 字段 | 类型 | 用途 |
| ---- | ---- | ---- |
| `id` | u64 | 唯一标识符（自动分配） |
| `refreshToken` | String | 刷新令牌（必需） |
| `accessToken` | String | 访问令牌（自动刷新） |
| `authMethod` | String | 认证方式：`social` / `idc` |
| `priority` | u32 | 优先级（数字越小越优先，默认 0） |
| `disabled` | bool | 禁用标志 |

### Region 配置字段

| 字段 | 用途 | 回退链 |
| ---- | ---- | ------ |
| `authRegion` | Token 刷新区域 | `authRegion` > `region` > `config.authRegion` > `config.region` |
| `apiRegion` | API 请求区域 | `apiRegion` > `config.apiRegion` > `config.region` |

### 代理配置字段

| 字段 | 用途 |
| ---- | ---- |
| `proxyUrl` | 代理地址（支持 `http`/`https`/`socks5`，特殊值 `direct` 表示直连） |
| `proxyUsername` | 代理认证用户名 |
| `proxyPassword` | 代理认证密码 |

**代理解析**: `src/kiro/model/credentials.rs:222-236` (effective_proxy)

## Selection Algorithm

### 优先级模式（priority）

```
1. 过滤：排除 disabled=true 的凭据
2. 过滤：Opus 模型排除 FREE 订阅
3. 排序：按 priority 升序（数字越小越优先）
4. 选择：取排序后第一个
```

**实现**: `src/kiro/token_manager.rs:661-707` (select_next_credential)

### 均衡模式（balanced）

```
1. 过滤：同优先级模式
2. 排序：按 (success_count, priority) 升序
3. 选择：取使用次数最少的
```

### 凭据选择流程

```
acquire_context(model) -> CallContext
    │
    ├─ balanced 模式？
    │   └─ 每次调用 select_next_credential
    │
    └─ priority 模式
        ├─ current_id 凭据可用？
        │   └─ 直接使用
        └─ 不可用 → select_next_credential
            │
            └─ 全部禁用？
                └─ 自愈：重置 TooManyFailures 状态
```

**实现**: `src/kiro/token_manager.rs:719-805` (acquire_context)

## Failover Logic

### 失败阈值

```rust
const MAX_FAILURES_PER_CREDENTIAL: u32 = 3;
```

### 失败处理流程

```
report_failure(id)
    │
    ├─ failure_count += 1
    │
    ├─ failure_count >= 3 ?
    │   ├─ disabled = true
    │   ├─ disabled_reason = TooManyFailures
    │   └─ 切换到下一个可用凭据
    │
    └─ 返回是否还有可用凭据
```

**实现**: `src/kiro/token_manager.rs:1107-1154` (report_failure)

### 额度耗尽处理

```
report_quota_exhausted(id)
    │
    ├─ disabled = true
    ├─ disabled_reason = QuotaExceeded
    ├─ failure_count = 3（标记为不可用）
    └─ 切换到下一个可用凭据
```

**实现**: `src/kiro/token_manager.rs:1162-1204` (report_quota_exhausted)

### 禁用原因枚举

| Reason | 触发条件 | 自愈支持 |
| ------ | -------- | -------- |
| Manual | Admin API 手动禁用 | 需手动启用 |
| TooManyFailures | 连续失败 3 次 | 自动自愈 |
| QuotaExceeded | MONTHLY_REQUEST_COUNT | 次月自动恢复 |

**定义**: `src/kiro/token_manager.rs:410-418` (DisabledReason)

## Token Refresh

### 刷新触发条件

```rust
// 已过期（提前 5 分钟判断）
is_token_expired(credentials) -> expires_at <= now + 5min

// 即将过期（10 分钟内）
is_token_expiring_soon(credentials) -> expires_at <= now + 10min
```

**实现**: `src/kiro/token_manager.rs:86-105`

### 刷新流程

```
try_ensure_token(id, credentials)
    │
    ├─ 双重检查锁定
    │   ├─ 第一次检查（无锁）：是否需要刷新
    │   └─ 第二次检查（有锁）：其他请求是否已刷新
    │
    ├─ 需要刷新 → refresh_token()
    │   ├─ Social 认证 → refresh_social_token()
    │   └─ IdC 认证 → refresh_idc_token()
    │
    ├─ 更新内存凭据
    ├─ 回写到文件（仅多凭据格式）
    └─ 返回 CallContext
```

**实现**: `src/kiro/token_manager.rs:860-925` (try_ensure_token)

### 认证方式判断

```
auth_method 显式指定 → 使用指定方式
未指定 + 有 clientId/clientSecret → IdC
未指定 + 无 clientId/clientSecret → Social
```

**实现**: `src/kiro/token_manager.rs:147-163` (refresh_token)

## Proxy Resolution

### 优先级链

```
凭据.proxyUrl = "direct" → 无代理
凭据.proxyUrl = 具体URL → 使用凭据代理（含认证）
凭据.proxyUrl = 未配置 → 使用全局代理
全局代理未配置 → 无代理
```

**实现**: `src/kiro/model/credentials.rs:222-236` (effective_proxy)

### 代理认证

```rust
// 代理 URL + 认证信息
if proxy_url && proxy_username && proxy_password {
    ProxyConfig::new(url).with_auth(username, password)
}
```

## Write-back Mechanism

### 触发条件

```
is_multiple_format = true  // 数组格式
credentials_path 已设置    // 有源文件路径
```

### 回写时机

1. Token 刷新后 → `persist_credentials()`
2. Admin API 修改后（禁用、优先级、添加、删除）
3. 新凭据自动分配 ID/machineId 后

**实现**: `src/kiro/token_manager.rs:937-978` (persist_credentials)

## Statistics Persistence

### 存储位置

```
{credentials_dir}/kiro_stats.json
```

### 存储内容

```json
{
  "1": { "success_count": 42, "last_used_at": "2025-01-15T10:30:00Z" },
  "2": { "success_count": 15, "last_used_at": "2025-01-15T09:00:00Z" }
}
```

### 防抖策略

```rust
const STATS_SAVE_DEBOUNCE: Duration = Duration::from_secs(30);
```

**实现**: `src/kiro/token_manager.rs:1060-1075` (save_stats_debounced)

## Design Rationale

### 为什么使用 CallContext？

并发请求时，`current_id` 可能在 Token 刷新后被其他请求修改，导致 `report_failure` 报告到错误的凭据。`CallContext` 绑定 ID、credentials、token 三者，确保一致性。

### 为什么失败阈值是 3？

平衡误判和可用性：
- 1 次：网络抖动即可触发切换，过于敏感
- 5 次：凭据真正失效时浪费过多请求
- 3 次：合理的中庸值

### 为什么区分 disabled_reason？

自愈机制需要区分：
- `TooManyFailures`：可能临时问题，可自动恢复
- `Manual` / `QuotaExceeded`：需要人工干预或等待周期重置
