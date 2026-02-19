# Admin API Architecture

## Identity

**What**: 可选的 Web 管理界面和 API 系统
**Purpose**: 提供凭据管理、余额查询、负载均衡配置等功能

## Core Components

| File | Symbol | Purpose |
| ---- | ------ | ------- |
| `src/admin/router.rs:34` | `create_admin_router` | 创建 Admin API 路由 |
| `src/admin/handlers.rs:*` | `get_all_credentials` 等 | HTTP 请求处理器 |
| `src/admin/service.rs:35` | `AdminService` | 业务逻辑服务 |
| `src/admin/middleware.rs:18` | `AdminState` | 共享状态和 API Key |
| `src/admin/middleware.rs:36` | `admin_auth_middleware` | 认证中间件 |
| `src/admin/types.rs:*` | `*Request` / `*Response` | API 类型定义 |
| `src/admin/import.rs:*` | `KiroAccountsExport` / `ImportResult` | 导入类型和格式转换 |
| `src/admin/error.rs:11` | `AdminServiceError` | 错误类型枚举 |
| `src/admin_ui/router.rs:18` | `create_admin_ui_router` | 静态文件路由 |

## API Endpoints

### 凭据管理

| Method | Path | Handler | 描述 |
| ------ | ---- | ------- | ---- |
| GET | `/api/admin/credentials` | `get_all_credentials` | 获取所有凭据状态 |
| POST | `/api/admin/credentials` | `add_credential` | 添加新凭据 |
| DELETE | `/api/admin/credentials/{id}` | `delete_credential` | 删除凭据 |
| POST | `/api/admin/credentials/{id}/disabled` | `set_credential_disabled` | 启用/禁用凭据 |
| POST | `/api/admin/credentials/{id}/priority` | `set_credential_priority` | 设置优先级 |
| POST | `/api/admin/credentials/{id}/reset` | `reset_failure_count` | 重置失败计数 |
| GET | `/api/admin/credentials/{id}/balance` | `get_credential_balance` | 查询余额 |
| POST | `/api/admin/credentials/import` | `import_credentials` | 导入凭据（JSON Body） |
| POST | `/api/admin/credentials/import/file` | `import_credentials_file` | 导入凭据（文件上传） |

### 凭据导入

**请求格式**:
- JSON Body: 直接传递 kiro-accounts 导出 JSON
- 文件上传: multipart/form-data，字段名为 `file`

**响应格式**:
```json
{
  "success": true,
  "message": "导入完成: 新增 5, 更新 3, 跳过 0, 共处理 8",
  "imported": 5,
  "updated": 3,
  "skipped": 0,
  "total": 8
}
```

**导入逻辑**:
- 根据 `clientId` 匹配现有凭据，存在则更新，不存在则新增
- 更新时保留 `priority`、`disabled`、`failure_count` 等状态

**核心文件**:
- `src/admin/import.rs` - 导入类型定义和格式转换
- `src/admin/service.rs:421-443` - `import_credentials` 方法
- `src/kiro/token_manager.rs:1660-1715` - 批量导入逻辑

### 配置管理

| Method | Path | Handler | 描述 |
| ------ | ---- | ------- | ---- |
| GET | `/api/admin/config/load-balancing` | `get_load_balancing_mode` | 获取负载均衡模式 |
| PUT | `/api/admin/config/load-balancing` | `set_load_balancing_mode` | 设置负载均衡模式 |

### Web UI

| Method | Path | Handler | 描述 |
| ------ | ---- | ------- | ---- |
| GET | `/admin` | `static_handler` | Web 管理界面（SPA） |
| GET | `/admin/{*file}` | `static_handler` | 静态资源文件 |

## Authentication Flow

```
Request → admin_auth_middleware → 验证 API Key → Handler
                    ↓
         [x-api-key header] ──┐
         [Authorization: Bearer] ─┴─→ constant_time_eq() 验证
```

**认证方式**（`src/admin/middleware.rs:36-50`）:
- `x-api-key` Header
- `Authorization: Bearer <token>` Header
- 使用常量时间比较防止时序攻击

**启用条件**: `config.json` 中配置非空 `adminApiKey`

## Service Layer

### AdminService 核心方法

| 方法 | 位置 | 功能 |
| ---- | ---- | ---- |
| `get_all_credentials` | `service.rs:57` | 获取凭据快照并排序 |
| `set_disabled` | `service.rs:93` | 设置禁用状态，当前凭据被禁用时自动切换 |
| `set_priority` | `service.rs:110` | 修改优先级 |
| `reset_and_enable` | `service.rs:117` | 重置失败计数并启用 |
| `get_balance` | `service.rs:124` | 余额查询（带 5 分钟缓存） |
| `add_credential` | `service.rs:185` | 添加凭据并获取订阅信息 |
| `delete_credential` | `service.rs:234` | 删除凭据（需先禁用） |
| `import_credentials` | `service.rs:421` | 批量导入凭据（支持 kiro-accounts 格式） |

### 余额缓存机制

```
get_balance() → 查缓存 → [命中] → 返回缓存数据
                  ↓
              [未命中] → fetch_balance() → 更新缓存 → 持久化
```

- TTL: 300 秒（`service.rs:21`）
- 持久化: `{cache_dir}/kiro_balance_cache.json`

## Error Handling

### 错误类型映射

| AdminServiceError | HTTP Status | 场景 |
| ----------------- | ----------- | ---- |
| `NotFound` | 404 | 凭据 ID 不存在 |
| `UpstreamError` | 502 | 网络/API 调用失败 |
| `InternalError` | 500 | 内部状态错误 |
| `InvalidCredential` | 400 | 验证失败、重复凭据 |

### 错误分类方法

- `classify_error`: 简单操作错误分类
- `classify_balance_error`: 余额查询错误（区分上游/本地）
- `classify_add_error`: 添加凭据错误
- `classify_delete_error`: 删除凭据错误

## UI Integration

### 静态文件嵌入

- 构建时嵌入 `admin-ui/dist/` 目录
- 使用 `rust_embed::Embed` 宏（`admin_ui/router.rs:13-15`）

### 缓存策略

| 文件类型 | Cache-Control |
| -------- | ------------- |
| HTML | `no-cache` |
| assets/*（带哈希） | `public, max-age=31536000, immutable` |
| 其他 | `public, max-age=3600` |

### SPA 路由处理

- 非资源路径返回 `index.html`（`admin_ui/router.rs:59`）
- 安全检查：拒绝 `..` 路径（`admin_ui/router.rs:34`）

## Request Flow

```
HTTP Request
    │
    ▼
admin_auth_middleware ─── 验证 adminApiKey
    │
    ▼
handlers.rs handlers ──── 参数解析
    │
    ▼
AdminService ──────────── 业务逻辑
    │
    ├─── MultiTokenManager ─── 凭据状态管理
    │
    └─── HTTP Client ───────── 余额查询
    │
    ▼
JSON Response
```

## Related

- `architecture/credential-management.md` - 多凭据管理架构
- `architecture/token-refresh.md` - Token 刷新机制
- `reference/api-endpoints.md` - API 端点详细规范
