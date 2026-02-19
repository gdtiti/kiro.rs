# kiro-rs 文档导航

## 项目概述

kiro-rs 是一个用 Rust 编写的 Anthropic Claude API 兼容代理服务，将 Anthropic API 请求转换为 Kiro API 请求。

## 文档目录

### Overview（项目概览）

| 文档 | 描述 |
|------|------|
| `overview/project-overview.md` | 项目定位和核心功能 |

### Architecture（架构设计）

| 文档 | 描述 |
|------|------|
| `architecture/credential-management.md` | 多凭据管理：优先级排序、故障转移、代理解析 |
| `architecture/request-flow.md` | 请求处理流水线（LLM 检索地图） |
| `architecture/streaming-response.md` | 流式响应架构（二进制→SSE 转换、两种模式） |
| `architecture/token-refresh.md` | Token 自动刷新机制 |
| `architecture/admin-api.md` | Admin 管理系统架构（LLM 检索地图） |
| `architecture/protocol-conversion.md` | Anthropic → Kiro 协议转换（LLM 检索地图） |

### Guides（使用指南）

| 文档 | 描述 |
|------|------|
| `guides/quick-start.md` | 快速启动指南 |
| `guides/multi-credential.md` | 多凭据配置指南 |

### Reference（参考规范）

| 文档 | 描述 |
|------|------|
| `reference/coding-conventions.md` | 编码规范和最佳实践 |
| `reference/api-endpoints.md` | API 端点参考 |

## 快速检索

### 按问题查找

| 我想了解... | 查看文档 |
|-------------|----------|
| 项目是什么 | `overview/project-overview.md` |
| 怎么工作 | `architecture/` 目录 |
| API 怎么转换 | `architecture/protocol-conversion.md` |
| 怎么做某事 | `guides/` 目录 |
| 具体规范 | `reference/` 目录 |
| 如何贡献代码 | `reference/coding-conventions.md` |

## 技术栈

- **Web 框架**: Axum 0.8
- **异步运行时**: Tokio
- **HTTP 客户端**: Reqwest
- **序列化**: Serde
- **日志**: tracing
