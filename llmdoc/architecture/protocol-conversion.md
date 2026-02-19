# 协议转换架构

## Identity

**What**: Anthropic API 格式到 Kiro API 格式的双向转换层
**Purpose**: 让使用 Anthropic API 的客户端能够无缝访问 Kiro 后端服务

## 核心组件

| 文件 | 符号 | 职责 |
| ---- | ---- | ---- |
| `src/anthropic/converter.rs:189` | `convert_request` | 请求转换主入口 |
| `src/anthropic/stream.rs:225` | `SseStateManager` | SSE 事件状态管理 |
| `src/anthropic/stream.rs:460` | `StreamContext` | 流式响应处理上下文 |
| `src/anthropic/websearch.rs` | 模块 | WebSearch 特殊处理 |
| `src/kiro/model/requests/conversation.rs` | 类型 | Kiro 请求结构 |
| `src/kiro/model/events/mod.rs` | 类型 | Kiro 响应事件 |

## 请求转换流程

```
Anthropic Request
       │
       ▼
┌─────────────────────┐
│  1. 模型名映射       │  converter.rs:83
│  sonnet → claude-sonnet-4.5/4.6
│  opus → claude-opus-4.5/4.6
│  haiku → claude-haiku-4.5
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  2. 系统消息注入     │  converter.rs:567-604
│  - 拼接分块写入策略
│  - 注入 thinking 标签
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  3. 历史消息构建     │  converter.rs:560-656
│  - user/assistant 配对
│  - tool_use/tool_result 配对验证
│  - 孤立项过滤
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  4. 工具定义转换     │  converter.rs:494-530
│  - JSON Schema 规范化
│  - Write/Edit 描述增强
│  - 历史工具占位符
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  5. 当前消息构建     │  converter.rs:255-277
│  - 文本 + 图片 + 工具结果
│  - 会话 ID 提取
└─────────────────────┘
       │
       ▼
Kiro ConversationState
```

### 关键转换点

| 源结构 (Anthropic) | 目标结构 (Kiro) | 代码位置 |
| ------------------ | --------------- | -------- |
| `messages[]` | `history[]` + `currentMessage` | `converter.rs:606-656` |
| `tools[].input_schema` | `tools[].toolSpecification.inputSchema.json` | `converter.rs:521-529` |
| `thinking.budget_tokens` | 系统消息 `<thinking_mode>` 标签 | `converter.rs:533-553` |
| `metadata.user_id` | `conversationId` (session UUID) | `converter.rs:130-149` |
| `content[].image` | `images[].format + source.bytes` | `converter.rs:308-312` |

## 响应转换流程

```
Kiro Event Stream
       │
       ▼
┌─────────────────────┐
│  事件类型分发        │  stream.rs:577-620
│  - assistantResponse
│  - toolUseEvent
│  - contextUsageEvent
│  - error/exception
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  Thinking 标签解析   │  stream.rs:68-172
│  - 跳过被引用的标签
│  - 缓冲区边界处理
│  - 双换行符验证
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  SSE 状态机          │  stream.rs:248-454
│  - message_start (一次)
│  - content_block_start/delta/stop
│  - message_delta (一次)
│  - message_stop
└─────────────────────┘
       │
       ▼
Anthropic SSE Stream
```

### SSE 事件序列

```
event: message_start       # 消息开始
event: content_block_start # thinking 块 (如启用)
event: content_block_delta # thinking_delta
event: content_block_stop
event: content_block_start # text 块
event: content_block_delta # text_delta (多次)
event: content_block_stop
event: content_block_start # tool_use 块 (如有)
event: content_block_delta # input_json_delta
event: content_block_stop
event: message_delta       # 消息结束信息
event: message_stop        # 消息结束
```

## 特殊场景处理

### Thinking 模式

**请求侧** - `converter.rs:533-604`:
- 将 `thinking.budget_tokens` 转换为系统消息前缀
- 格式: `<thinking_mode>enabled</thinking_mode><max_thinking_length>N</max_thinking_length>`
- Adaptive 模式: `<thinking_effort>high</thinking_effort>`

**响应侧** - `stream.rs:642-785`:
- 解析 `<thinking>` 标签
- 跳过被反引号/引号包裹的标签（避免误判）
- 生成独立的 thinking_delta 事件

### 工具调用

**配对验证** - `converter.rs:384-455`:
```
validate_tool_pairing():
  1. 收集历史中的 tool_use_id
  2. 收集已有 tool_result 的 id
  3. 过滤孤立的 tool_result
  4. 返回孤立的 tool_use_id
```

**历史工具占位** - `converter.rs:170-186`:
- 为历史中使用但未定义的工具生成占位符
- 避免 Kiro API 报错 "tool not found"

### WebSearch

**检测条件** - `websearch.rs:104-108`:
- `tools` 有且仅有一个
- 工具名称为 `web_search`

**转换流程** - `websearch.rs:442-488`:
```
1. 提取搜索查询
2. 创建 MCP 请求 (tools/call)
3. 调用 Kiro MCP API
4. 生成 SSE 响应序列:
   - server_tool_use 块
   - web_search_tool_result 块
   - text 块 (摘要)
```

## 数据结构映射

### 请求结构

```
Anthropic MessagesRequest
├── model          → model_id (映射)
├── max_tokens     → (用于 token 估算)
├── messages[]     → history[] + currentMessage
│   ├── role: "user"
│   │   └── content → userInputMessage.content
│   └── role: "assistant"
│       └── content → assistantResponseMessage.content
├── system         → history[0] (user+assistant 配对)
├── tools[]        → userInputMessageContext.tools[]
└── thinking       → 系统消息前缀标签
```

### 响应事件

| Kiro 事件 | Anthropic SSE 事件 |
| --------- | ------------------ |
| `assistantResponseEvent` | `content_block_delta` (text_delta) |
| `toolUseEvent` | `content_block_start/delta/stop` (tool_use) |
| `contextUsageEvent` | (内部使用，计算 input_tokens) |

## 错误处理

| 错误类型 | 处理方式 | 代码位置 |
| -------- | -------- | -------- |
| 不支持的模型 | 返回 `UnsupportedModel` | `converter.rs:191-192` |
| 空消息列表 | 返回 `EmptyMessages` | `converter.rs:195-197` |
| 孤立 tool_use | 从历史中移除 | `converter.rs:465-491` |
| 孤立 tool_result | 静默过滤 + 警告日志 | `converter.rs:439-444` |
| 上下文超限 | 设置 stop_reason | `stream.rs:589-592` |

## 设计决策

1. **系统消息作为 user+assistant 配对** - Kiro API 不支持独立系统消息，使用模拟配对
2. **MANUAL 触发类型** - 避免 AUTO 模式导致的 400 错误
3. **Thinking 标签解析** - 使用缓冲区 + 双换行验证，避免误判
4. **工具配对验证** - 确保 Kiro API 不会因孤立项报错
5. **历史工具占位符** - 向前兼容，支持对话中工具变化

## Related

- `architecture/event-stream.md` - AWS Event Stream 解析
- `architecture/api-layer.md` - API 分层架构
- `reference/api-endpoints.md` - API 端点参考
