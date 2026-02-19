# Token 刷新架构

## Identity

**What**: OAuth Token 自动刷新与多凭据管理系统
**Purpose**: 确保 API 请求始终使用有效的访问令牌，支持 Social 和 IdC 两种认证方式

## Core Components

| File | Symbol | Purpose |
| ---- | ------ | ------- |
| `src/kiro/token_manager.rs:30` | TokenManager | 单凭据 Token 管理 |
| `src/kiro/token_manager.rs:482` | MultiTokenManager | 多凭据管理，支持故障转移 |
| `src/kiro/token_manager.rs:512` | CallContext | API 调用上下文，绑定凭据 ID/Token |
| `src/kiro/model/token_refresh.rs:6` | RefreshRequest | Social 认证刷新请求体 |
| `src/kiro/model/token_refresh.rs:23` | IdcRefreshRequest | IdC 认证刷新请求体 |
| `src/kiro/model/credentials.rs:16` | KiroCredentials | 凭据数据模型 |

## Token 生命周期

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│   有效期    │ ──► │ 即将过期(10m)│ ──► │  已过期(5m) │
│  > 10 min   │     │   5-10 min   │     │   < 5 min   │
└─────────────┘     └──────────────┘     └─────────────┘
      │                    │                    │
      ▼                    ▼                    ▼
   直接使用            触发刷新              触发刷新
```

### 过期判断函数

| 函数 | 位置 | 判断逻辑 |
| ---- | ---- | -------- |
| `is_token_expired` | `token_manager.rs:98` | expires_at ≤ now + 5min |
| `is_token_expiring_soon` | `token_manager.rs:103` | expires_at ≤ now + 10min |
| `is_token_expiring_within` | `token_manager.rs:86` | 通用判断，参数化分钟数 |

## 刷新流程

### 单凭据刷新

```
ensure_valid_token() [token_manager.rs:59]
  │
  ├─► is_token_expired() || is_token_expiring_soon()
  │         │
  │         ▼
  │    refresh_token() [token_manager.rs:138]
  │         │
  │         ├─► validate_refresh_token() - 验证 refreshToken 完整性
  │         │
  │         ├─► auth_method 判断
  │         │      ├─ "idc" / "builder-id" / "iam" → refresh_idc_token()
  │         │      └─ 其他 → refresh_social_token()
  │         │
  │         └─► 返回新凭据（含新 access_token/refresh_token/expires_at）
  │
  └─► 返回 access_token
```

### 多凭据刷新（双重检查锁定）

```
acquire_context() [token_manager.rs:719]
  │
  ├─► select_next_credential() - 按优先级/负载均衡选择凭据
  │
  └─► try_ensure_token() [token_manager.rs:860]
         │
         ├─► 第一次检查（无锁）: needs_refresh?
         │         │
         │         ▼ 是
         │    ┌────────────────────────────────┐
         │    │ refresh_lock.lock().await      │ ◄─ 获取刷新锁
         │    │                                │
         │    │ 第二次检查: 仍需刷新?           │
         │    │   ├─ 是 → 执行刷新             │
         │    │   └─ 否 → 其他请求已完成刷新   │
         │    └────────────────────────────────┘
         │
         └─► 返回 CallContext { id, credentials, token }
```

## 认证方式

### Social 认证

- **端点**: `https://prod.{region}.auth.desktop.kiro.dev/refreshToken`
- **请求体**: `{ refreshToken }`
- **位置**: `token_manager.rs:166-235`

### IdC 认证（AWS SSO OIDC）

- **端点**: `https://oidc.{region}.amazonaws.com/token`
- **请求体**: `{ clientId, clientSecret, refreshToken, grantType }`
- **位置**: `token_manager.rs:241-313`

### 认证方式判断逻辑

```rust
// token_manager.rs:147-163
let auth_method = credentials.auth_method.as_deref().unwrap_or_else(|| {
    if credentials.client_id.is_some() && credentials.client_secret.is_some() {
        "idc"  // 有 clientId/clientSecret → IdC
    } else {
        "social"
    }
});
```

## Region 配置优先级

### Auth Region（Token 刷新）

```
凭据.auth_region > 凭据.region > config.auth_region > config.region
```

- 实现: `credentials.rs:204` (`effective_auth_region`)

### API Region（API 请求）

```
凭据.api_region > config.api_region > config.region
```

- 实现: `credentials.rs:213` (`effective_api_region`)

## 回写机制

### 触发条件

| 场景 | 是否回写 | 说明 |
| ---- | -------- | ---- |
| 多凭据格式（数组） | ✅ | 刷新后写入 `credentials.json` |
| 单凭据格式（对象） | ❌ | 仅内存更新 |

### 回写流程

```
persist_credentials() [token_manager.rs:937]
  │
  ├─► 检查 is_multiple_format
  │
  ├─► 收集所有凭据 entries → Vec<KiroCredentials>
  │
  └─► 序列化并写入文件
```

### 统计数据持久化

- **文件**: `{cache_dir}/kiro_stats.json`
- **内容**: `{ id: { success_count, last_used_at } }`
- **策略**: Debounce 30 秒
- **位置**: `token_manager.rs:1024-1058`

## 多凭据故障转移

### 禁用触发条件

| 条件 | 函数 | 行为 |
| ---- | ---- | ---- |
| 连续 3 次 API 调用失败 | `report_failure()` | 自动禁用，切换下一凭据 |
| 额度用尽（402 + MONTHLY_REQUEST_COUNT） | `report_quota_exhausted()` | 立即禁用 |
| Admin API 手动禁用 | `set_disabled()` | 标记 `disabled_reason: Manual` |

### 自愈机制

当所有凭据均被自动禁用（`TooManyFailures`）时：

```
acquire_context() [token_manager.rs:756-772]
  │
  └─► 检测所有禁用凭据均为 TooManyFailures
         │
         └─► 重置 failure_count = 0, disabled = false
```

## 代理配置优先级

```
凭据.proxy_url > config.proxy_url > 无代理
```

- **特殊值**: `direct` 表示显式不使用代理
- **实现**: `credentials.rs:222` (`effective_proxy`)

## Related

- `architecture/credentials-management.md` - 凭据管理架构
- `guides/adding-credentials.md` - 添加凭据指南
- `reference/auth-methods.md` - 认证方式参考
