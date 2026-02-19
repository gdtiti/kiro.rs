# 请求流程 (Request Flow)

## Identity

**What**: Anthropic API 兼容代理的请求处理流水线
**Purpose**: 将 Anthropic 格式请求转换为 Kiro API 调用，并返回兼容响应

## 请求生命周期概览

```
Client → Router → Middleware → Handler → Converter → Provider → Parser → Stream → SSE Response
```

| 层 | 文件:行号 | 入口函数 | 职责 |
|----|-----------|----------|------|
| Router | `router.rs:37` | `create_router_with_provider()` | Axum 路由 |
| Middleware | `middleware.rs:54` | `auth_middleware()` | API Key 验证 |
| Handler | `handlers.rs:137` | `post_messages()` | 请求分发 |
| Converter | `converter.rs:83` | `convert_request()` | 格式转换 |
| Provider | `provider.rs:258` | `call_api_stream()` | API 调用+重试 |
| Parser | `decoder.rs:54` | `EventStreamDecoder` | Event Stream 解码 |
| Stream | `stream.rs` | `StreamContext` | SSE 生成 |

## 详细流程

```
POST /v1/messages
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│ 1. Router (router.rs:37)                                        │
│    └─ create_router_with_provider() 匹配路由                    │
└─────────────────────────┬───────────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. Middleware (middleware.rs:54)                                │
│    └─ auth_middleware() 验证 x-api-key / Bearer                 │
└─────────────────────────┬───────────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. Handler (handlers.rs:137)                                    │
│    ├─ thinking 后缀检测 → override_thinking_from_model_name()   │
│    ├─ WebSearch? → websearch::handle_websearch_request()        │
│    └─ 常规 → convert_request() → KiroRequest                    │
└─────────────────────────┬───────────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. Converter (converter.rs)                                     │
│    ├─ map_model(): sonnet/opus/haiku → Kiro ID                  │
│    └─ convert_request(): 消息/工具/系统提示词转换               │
└─────────────────────────┬───────────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. Provider (provider.rs:258-277)                               │
│    ├─ Token 过期? → token_manager.refresh_token()               │
│    └─ 故障转移: 401/403/402 切换凭据，最多重试 9 次             │
└─────────────────────────┬───────────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│ 6. Parser (decoder.rs)                                          │
│    └─ EventStreamDecoder: Ready→Parsing→[Recovering]→Stopped    │
└─────────────────────────┬───────────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│ 7. Stream (stream.rs)                                           │
│    ├─ StreamContext → process_kiro_event() → SSE                │
│    └─ BufferedStreamContext → 等待 contextUsageEvent 后返回     │
└─────────────────────────────────────────────────────────────────┘
```

## 核心函数索引

### 路由与认证
| 函数 | 文件:行号 | 用途 |
|------|-----------|------|
| `create_router_with_provider()` | `router.rs:37` | 创建 Router |
| `auth_middleware()` | `middleware.rs:54` | API Key 验证 |
| `AppState` | `middleware.rs:19` | 共享状态 |

### 请求处理
| 函数 | 文件:行号 | 用途 |
|------|-----------|------|
| `post_messages()` | `handlers.rs:137` | /v1/messages 入口 |
| `post_messages_cc()` | `handlers.rs:636` | /cc/v1 缓冲模式 |
| `handle_stream_request()` | `handlers.rs:258` | 流式响应 |
| `handle_non_stream_request()` | `handlers.rs:404` | 非流式响应 |

### 转换与调用
| 函数 | 文件:行号 | 用途 |
|------|-----------|------|
| `map_model()` | `converter.rs:83` | 模型名映射 |
| `convert_request()` | `converter.rs` | 格式转换 |
| `call_api()` | `provider.rs:258` | 非流式调用 |
| `call_api_stream()` | `provider.rs:275` | 流式调用 |

## 扩展点

| 场景 | 位置 | 步骤 |
|------|------|------|
| 新端点 | `router.rs` | handlers.rs 添加函数 → 注册路由 |
| 新模型 | `converter.rs:83` | 修改 map_model() |
| 新事件 | `kiro/model/events/` | 定义结构体 → Event 枚举 → stream.rs 处理 |
| 自定义认证 | `middleware.rs:54` | 修改 auth_middleware() |

## 数据流

### 流式 (stream=true)
```
post_messages() → handle_stream_request() → call_api_stream()
  → EventStreamDecoder → StreamContext.process_kiro_event() → SSE
```

### 缓冲 (/cc/v1/messages)
```
post_messages_cc() → handle_stream_request_buffered()
  → 等待流完成 → finish_and_get_all_events()
```

### 非流式 (stream=false)
```
post_messages() → handle_non_stream_request() → call_api()
  → EventStreamDecoder → JSON 响应
```

## 相关文档

- `architecture/token-management.md` - Token 刷新与多凭据
- `architecture/event-stream.md` - AWS Event Stream 解析
- `reference/api-endpoints.md` - API 端点规格
