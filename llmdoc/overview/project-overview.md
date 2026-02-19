# kiro-rs 项目概览

## Identity

**What**: Anthropic Claude API 兼容代理服务，将 Anthropic API 请求转换为 Kiro API 请求。
**Purpose**: 让使用 Anthropic API 的客户端无缝接入 Kiro 服务，提供完整的 Claude 模型体验。

## High-Level Description

kiro-rs 是一个高性能的 API 代理网关，核心职责是协议转换。它接收标准 Anthropic Claude API 请求，将其转换为 Kiro 专有协议格式，并处理 OAuth 认证、Token 自动刷新、流式响应转换等复杂逻辑。

项目采用分层架构设计：`anthropic` 模块处理入站请求解析和认证，`kiro` 模块负责上游 API 调用和协议转换，`admin` 模块提供可选的管理界面。核心转换逻辑位于 `src/anthropic/converter.rs`。

支持两种认证方式：Social OAuth 和 AWS IdC（Builder-ID/IAM）。多凭据配置支持按优先级自动故障转移和负载均衡，单凭据最多重试 3 次，单请求最多重试 9 次。

流式响应采用 SSE (Server-Sent Events) 格式，通过 AWS Event Stream 解析器（`src/kiro/parser/`）解码上游二进制流，转换为 Anthropic 兼容的 JSON 事件流。

## Key Features

- **Anthropic API 兼容** - 完整支持 `/v1/messages`、`/v1/models` 等端点
- **流式响应** - SSE 实时流式输出，支持 Thinking 模式和工具调用
- **OAuth Token 自动刷新** - 自动管理 AccessToken 生命周期
- **多凭据支持** - 按优先级故障转移，支持 `priority` 和 `balanced` 负载均衡模式
- **凭据级代理** - 每个凭据可独立配置 HTTP/SOCKS5 代理
- **Admin Web UI** - 可选的管理界面，支持凭据管理、余额查询

## Tech Stack

| 类别 | 技术 |
|------|------|
| Web 框架 | Axum 0.8 |
| 异步运行时 | Tokio |
| HTTP 客户端 | Reqwest (rustls) |
| 序列化 | Serde, serde_json |
| 日志 | tracing, tracing-subscriber |
| 命令行 | Clap |

## Target Users

- 使用 Anthropic SDK 的开发者，希望接入 Kiro 服务
- 需要多账号管理、负载均衡的团队
- 需要自建 Claude API 代理的用户

## Core Code References

| 功能 | 位置 |
|------|------|
| 程序入口 | `src/main.rs:1` |
| API 路由配置 | `src/anthropic/router.rs` |
| 请求处理器 | `src/anthropic/handlers.rs` |
| 协议转换器 | `src/anthropic/converter.rs` |
| 流式响应处理 | `src/anthropic/stream.rs` |
| Kiro API 提供者 | `src/kiro/provider.rs` |
| Token 管理 | `src/kiro/token_manager.rs` |
| OAuth 凭证模型 | `src/kiro/model/credentials.rs` |
| Admin 服务 | `src/admin/service.rs` |

## Related

- `architecture/request-flow.md` - 请求处理流程
- `architecture/credential-management.md` - 凭据管理机制
- `guides/quick-start.md` - 快速开始指南
- `reference/api-endpoints.md` - API 端点参考
