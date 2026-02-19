# 流式响应架构

## Identity

**What**: Kiro → Anthropic 流式响应转换系统
**Purpose**: 将 AWS Event Stream 二进制格式转换为 Anthropic SSE 格式

## 高层架构

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Kiro API      │───▶│  Event Stream   │───▶│   SSE Output    │
│  (二进制流)      │    │    Parser       │    │   (文本流)       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                              │
                    ┌─────────┼─────────┐
                    ▼         ▼         ▼
                Decoder    Frame    Header/CRC
```

## 核心组件

| 文件 | 符号 | 职责 |
| ---- | ---- | ---- |
| `src/kiro/parser/decoder.rs:87` | `EventStreamDecoder` | 状态机解码器，管理缓冲区和错误恢复 |
| `src/kiro/parser/frame.rs:33` | `Frame` | 消息帧结构，包含 headers 和 payload |
| `src/kiro/parser/header.rs:73` | `Headers` | 头部解析，提取 event-type 等元信息 |
| `src/kiro/parser/crc.rs:17` | `crc32` | CRC32 校验（ISO-HDLC 标准） |
| `src/kiro/model/events/base.rs:65` | `Event` | 统一事件枚举 |
| `src/anthropic/stream.rs:225` | `SseStateManager` | SSE 事件序列状态管理 |
| `src/anthropic/stream.rs:460` | `StreamContext` | 实时流处理上下文 |
| `src/anthropic/stream.rs:1078` | `BufferedStreamContext` | 缓冲流处理上下文 |

## 执行流程

### 1. 二进制帧解析

```
输入: Kiro API 原始字节流
      │
      ▼
┌─────────────────────────────────────────────────────┐
│  EventStreamDecoder (decoder.rs:173)                │
│                                                     │
│  feed() ──▶ buffer ──▶ decode() ──▶ Frame          │
└─────────────────────────────────────────────────────┘
      │
      ▼ parse_frame() (frame.rs:75)
┌─────────────────────────────────────────────────────┐
│  帧结构 (frame.rs:6-17)                             │
│  ┌──────────┬──────────┬───────────┬─────┬─────┐   │
│  │Total Len │Header Len│Prelude CRC│Headers│Payload│Msg CRC│
│  │ 4 bytes  │ 4 bytes  │ 4 bytes  │变长  │变长  │4 bytes│
│  └──────────┴──────────┴───────────┴─────┴─────┘   │
└─────────────────────────────────────────────────────┘
      │
      ▼ CRC 校验 (crc.rs:17)
      │ - Prelude CRC: 前 8 字节
      │ - Message CRC: 整帧（不含最后 4 字节）
      ▼
┌─────────────────────────────────────────────────────┐
│  Frame.headers ──▶ event_type() (header.rs:107)    │
│  Frame.payload ──▶ JSON 反序列化                   │
└─────────────────────────────────────────────────────┘
```

### 2. 事件类型分发

| event-type | Event 变体 | 处理位置 |
| ---------- | ---------- | -------- |
| `assistantResponseEvent` | `Event::AssistantResponse` | `base.rs:111` |
| `toolUseEvent` | `Event::ToolUse` | `base.rs:115` |
| `contextUsageEvent` | `Event::ContextUsage` | `base.rs:120` |
| `error` | `Event::Error` | `base.rs:129` |
| `exception` | `Event::Exception` | `base.rs:144` |

### 3. SSE 事件生成

```
Event ──▶ StreamContext.process_kiro_event() (stream.rs:577)
              │
              ├── AssistantResponse ──▶ process_content_with_thinking()
              │                              │
              │                              ├── thinking 块: thinking_delta
              │                              └── 文本块: text_delta
              │
              ├── ToolUse ──▶ process_tool_use()
              │                   │
              │                   ├── content_block_start (tool_use)
              │                   ├── input_json_delta
              │                   └── content_block_stop
              │
              └── ContextUsage ──▶ 计算 input_tokens
```

### 4. SSE 状态管理

`SseStateManager` (stream.rs:225) 确保事件序列符合 Claude API 规范：

```
┌───────────────┐     ┌───────────────────┐     ┌───────────────┐
│ message_start │ ──▶ │ content_block_*   │ ──▶ │ message_delta │
│  (仅一次)      │     │ start/delta/stop  │     │  (仅一次)      │
└───────────────┘     └───────────────────┘     └───────────────┘
                                                      │
                                                      ▼
                                              ┌───────────────┐
                                              │ message_stop  │
                                              └───────────────┘
```

## 两种流式模式

### 实时模式 (`/v1/messages`)

```
Kiro Event ──▶ SSE Event (立即发送)
     │
     └── input_tokens: 估算值
```

特点：
- 低延迟，事件实时转发
- `input_tokens` 基于请求内容估算
- 适用于普通客户端

### 缓冲模式 (`/cc/v1/messages`)

```
Kiro Event ──▶ 缓冲 ──▶ 流结束 ──▶ 更正 input_tokens ──▶ 批量发送
     │                                           │
     └── contextUsageEvent ──────────────────────┘
          (包含准确的上下文使用百分比)
```

特点：
- 等待 `contextUsageEvent` 获取准确 `input_tokens`
- 流结束后一次性发送所有事件
- 等待期间每 25 秒发送 `ping` 保活
- 专为 Claude Code 兼容设计

代码位置：
- `BufferedStreamContext`: `stream.rs:1078`
- `finish_and_get_all_events()`: `stream.rs:1128`

## Thinking 块处理

当 `thinking` 模式启用时，需处理 `<thinking>` 标签提取：

```
原始内容: "前置文本<thinking>思考内容</thinking>\n\n后续文本"
              │
              ▼ find_real_thinking_start_tag() (stream.rs:148)
              │
              ▼ find_real_thinking_end_tag() (stream.rs:68)
              │
              ▼
┌─────────────┬─────────────────────┬─────────────┐
│ text_delta  │ thinking_delta      │ text_delta  │
│ (前置文本)   │ (思考内容)           │ (后续文本)   │
└─────────────┴─────────────────────┴─────────────┘
```

关键逻辑：
- 跳过被引用字符包裹的标签（如 \`</thinking>\`）
- 标签识别需要 `\n\n` 后缀确认
- 边界场景：tool_use 开始前、流结束时特殊处理

## 错误恢复

`EventStreamDecoder` 采用四态状态机 (decoder.rs:54)：

```
Ready ──▶ Parsing ──▶ [成功] ──▶ Ready
              │
              └──▶ [失败] ──▶ Recovering ──▶ Ready
                              │
                              └──▶ 连续错误 >= max_errors ──▶ Stopped
```

恢复策略 (decoder.rs:241)：
- Prelude 错误：逐字节跳过，寻找下一帧边界
- Data 错误：跳过整个损坏帧
- 最大连续错误数：5 次

## 相关文档

- `architecture/request-flow.md` - 完整请求处理流水线
- `reference/api-endpoints.md` - API 端点规范
