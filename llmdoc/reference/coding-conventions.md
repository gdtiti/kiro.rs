# 编码规范

## Identity

**What**: kiro-rs 项目的代码风格和最佳实践
**Purpose**: 帮助贡献者遵循一致的编码模式

## Core Summary

1. **模块化组织** - 按功能划分子模块，每模块独立 types/handlers/router
2. **分层错误处理** - anyhow 通用错误 + 领域错误类型
3. **Serde 序列化** - camelCase JSON 输出，跳过 None 值
4. **内联测试** - `#[cfg(test)]` 模块与实现同文件

## 模块组织

### 目录结构

```
src/
├── main.rs              # 程序入口
├── http_client.rs       # 公共模块（单文件）
├── model/               # 数据模型
│   ├── mod.rs
│   ├── config.rs
│   └── arg.rs
├── anthropic/           # API 模块
│   ├── mod.rs
│   ├── router.rs
│   ├── handlers.rs
│   ├── middleware.rs
│   └── types.rs
└── kiro/                # 复杂模块（嵌套子模块）
    ├── mod.rs
    ├── provider.rs
    ├── model/
    │   ├── mod.rs
    │   └── credentials.rs
    └── parser/
        ├── mod.rs
        └── decoder.rs
```

### 模块入口 (mod.rs)

```rust
// 单层模块 - src/kiro/mod.rs:1-8
pub mod machine_id;
pub mod model;
pub mod parser;
pub mod provider;
pub mod token_manager;
```

### 引用规则

- **同模块引用**: `use super::{handlers, types};`
- **跨模块引用**: `use crate::kiro::provider::KiroProvider;`
- **外部 crate**: `use axum::{Router, Json};`

## 错误处理

### 通用错误

使用 `anyhow::Result` 作为函数返回类型：

```rust
// src/model/config.rs:181
pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self>
```

### 领域错误类型

为特定领域定义错误枚举：

```rust
// src/admin/error.rs:10-23
#[derive(Debug)]
pub enum AdminServiceError {
    NotFound { id: u64 },
    UpstreamError(String),
    InternalError(String),
    InvalidCredential(String),
}
```

**必须实现**:
- `impl fmt::Display`
- `impl std::error::Error`
- `fn status_code(&self) -> StatusCode` - HTTP 状态码映射
- `fn into_response(self) -> Response` - API 错误响应转换

### 错误传播

```rust
// 使用 ? 操作符
let content = fs::read_to_string(path)?;
let config: Config = serde_json::from_str(&content)?;
```

## 异步约定

### 运行时

使用 Tokio 作为异步运行时：

```rust
// src/main.rs:19
#[tokio::main]
async fn main() {
    // ...
}
```

### Handler 函数

所有 Axum handler 使用 async：

```rust
// src/anthropic/handlers.rs:32
pub async fn get_models() -> impl IntoResponse {
    // ...
}

// src/anthropic/handlers.rs:137
pub async fn post_messages(
    State(state): State<AppState>,
    JsonExtractor(payload): JsonExtractor<MessagesRequest>,
) -> Response {
    // ...
}
```

### 流式处理

使用 `tokio::select!` 处理并发流：

```rust
// src/anthropic/handlers.rs:332-392
tokio::select! {
    chunk_result = body_stream.next() => {
        // 处理数据流
    }
    _ = ping_interval.tick() => {
        // 发送保活信号
    }
}
```

## 序列化规范

### JSON 命名

使用 `#[serde(rename_all = "camelCase")]` 输出 camelCase：

```rust
// src/admin/types.rs:8-10
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialsStatusResponse {
    pub total: usize,
    pub available: usize,
    // ...
}
```

### 可选字段

跳过 `None` 值的序列化：

```rust
// src/kiro/model/credentials.rs:18-27
pub struct KiroCredentials {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    // ...
}
```

### 默认值

使用 `#[serde(default)]` 提供默认值：

```rust
// src/anthropic/types.rs:120-121
#[serde(default)]
pub stream: bool,
```

### 自定义反序列化

复杂场景使用自定义 visitor：

```rust
// src/anthropic/types.rs:133-189
fn deserialize_system<'de, D>(deserializer: D) -> Result<Option<Vec<SystemMessage>>, D::Error>
where D: serde::Deserializer<'de> {
    // 支持 string 或 array 两种格式
}
```

## 命名约定

| 类型 | 风格 | 示例 |
|------|------|------|
| 文件名 | snake_case | `token_manager.rs` |
| 模块名 | snake_case | `pub mod token_manager;` |
| 结构体 | PascalCase | `struct CredentialsConfig` |
| 枚举 | PascalCase | `enum AdminServiceError` |
| 函数 | snake_case | `fn get_all_credentials()` |
| 变量 | snake_case | `let config_path = ...` |
| 常量 | SCREAMING_SNAKE_CASE | `const MAX_BODY_SIZE: usize = 50 * 1024 * 1024;` |
| 静态 | SCREAMING_SNAKE_CASE | `pub static INSTANCE: OnceCell<...>` |

### 布尔字段命名

使用 `is_`, `has_` 前缀表示布尔语义：

```rust
// src/admin/types.rs:40-50
pub is_current: bool,
pub has_profile_arn: bool,
pub has_proxy: bool,
```

## 测试规范

### 测试位置

测试代码放在同文件的 `#[cfg(test)]` 模块中：

```rust
// src/kiro/model/credentials.rs:286-287
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_json() {
        // ...
    }
}
```

### 测试命名

- 测试函数以 `test_` 前缀
- 描述性名称说明测试场景：`test_priority_default`, `test_credentials_config_priority_sorting`

### 测试组织

按功能分组测试：

```rust
// src/kiro/model/credentials.rs:409
// ============ Region 字段测试 ============

// src/kiro/model/credentials.rs:496
// ============ MachineId 字段测试 ============
```

## 文档注释

### 模块级文档

使用 `//!` 注释：

```rust
// src/kiro/mod.rs:1
//! Kiro API 客户端模块
```

### 公共 API 文档

使用 `///` 注释：

```rust
// src/admin/error.rs:9-23
/// Admin 服务错误类型
#[derive(Debug)]
pub enum AdminServiceError {
    /// 凭据不存在
    NotFound { id: u64 },
    /// 上游服务调用失败（网络、API 错误等）
    UpstreamError(String),
    // ...
}
```

### 函数文档

描述功能、参数和返回值：

```rust
// src/kiro/model/credentials.rs:129-133
/// 从文件加载凭据配置
///
/// - 如果文件不存在，返回空数组
/// - 如果文件内容为空，返回空数组
/// - 支持单对象或数组格式
pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self>
```

## HTTP 路由

### Router 定义

每个 API 模块提供独立的 `create_*_router` 函数：

```rust
// src/anthropic/router.rs:37
pub fn create_router_with_provider(
    api_key: impl Into<String>,
    kiro_provider: Option<KiroProvider>,
    profile_arn: Option<String>,
) -> Router

// src/admin/router.rs:34
pub fn create_admin_router(state: AdminState) -> Router
```

### 路由分组

按功能分组路由，使用中间件保护：

```rust
// src/anthropic/router.rs:51-58
let v1_routes = Router::new()
    .route("/models", get(get_models))
    .route("/messages", post(post_messages))
    .route("/messages/count_tokens", post(count_tokens))
    .layer(middleware::from_fn_with_state(
        state.clone(),
        auth_middleware,
    ));
```

## 相关架构

- `architecture/api-layer.md` - API 分层架构
- `architecture/token-management.md` - Token 管理流程
