# Zembra 后端技术选型需求澄清

日期：2026-04-26

## 背景

Zembra 后端使用 Rust 开发，服务于 Zembra 笔记应用。第一阶段主要目标是提供围绕笔记数据的 CRUD 能力，并以共享数据表契约为准实现服务端数据访问逻辑。

共享 schema 由上级项目文档 `GLOBAL.md` 约定，通过 Git submodule 引入：

```text
vendor/zembra-schema
```

该 submodule 固定到共享 schema 版本，数据表说明、SQLite DDL、JSON Schema 和 migration 均以该目录内容为准。本后端仓库不复制维护数据表设计正文。

## 已确认目标

- 使用 Rust 开发 Zembra 笔记应用后端。
- 第一阶段以后端 CRUD 操作为核心。
- 数据库结构与迁移策略遵循 `vendor/zembra-schema`。
- 技术栈优先选择轻量、稳定、适合 CRUD 服务的 Rust 生态组件。
- Rust 项目使用 2024 edition。

## Rust 2024 结论

Rust 2024 edition 不会对当前技术选型带来明显麻烦。Axum、Tokio、SQLx、Serde 等主流库均可在 Rust 2024 edition 项目中正常使用。

实施时的约束：

- `Cargo.toml` 使用 `edition = "2024"`。
- 不因为 2024 edition 引入不必要的新语法或实验性写法。
- 代码风格以稳定、清晰、便于维护为主。

## 技术选型结论

| 层级 | 选型 | 结论 |
|---|---|---|
| Rust edition | Rust 2024 | 项目默认 edition |
| Web 框架 | Axum | HTTP API、路由、中间件、状态注入 |
| 异步运行时 | Tokio | Web 服务运行时 |
| 数据库 | SQLite | 匹配共享 schema 和轻量 CRUD 场景 |
| 数据访问 | SQLx | 显式 SQL、连接池、migration 支持 |
| 序列化 | Serde / Serde JSON | API 请求响应与 JSON 字段处理 |
| 参数校验 | Validator | DTO 层输入校验 |
| 错误处理 | Thiserror + 自定义 API Error | 领域错误和 HTTP 状态码映射 |
| 日志追踪 | Tracing / Tracing Subscriber | 请求日志、结构化日志和错误定位 |
| 配置管理 | Config + TOML + 环境变量覆盖 | 管理端口、数据库路径、日志等级 |
| API 文档 | Utoipa + Utoipa Swagger UI | 生成 OpenAPI 文档，服务客户端联调 |
| 测试 | Tokio Test + SQLx 测试数据库 | Repository、Service、API 集成测试 |
| 构建部署 | Cargo + Docker 多阶段构建 | 本地开发和服务器部署 |

## 推荐架构分层

```text
src/
  main.rs
  app.rs
  config.rs
  error.rs
  routes/
  handlers/
  services/
  repositories/
  models/
  dto/
  migrations/
```

| 模块 | 职责 |
|---|---|
| `routes` | 注册 HTTP 路由 |
| `handlers` | 解析请求、调用 service、返回响应 |
| `services` | 承载业务规则 |
| `repositories` | 使用 SQLx 访问 SQLite |
| `models` | 数据库实体模型 |
| `dto` | API 请求和响应结构 |
| `error` | 统一错误类型和 HTTP 状态码映射 |
| `config` | 服务配置读取 |

## 第一阶段 API 范围

第一阶段采用 REST API，优先覆盖笔记 CRUD：

```text
GET    /health
GET    /api/v1/notes
POST   /api/v1/notes
GET    /api/v1/notes/{id}
PATCH  /api/v1/notes/{id}
DELETE /api/v1/notes/{id}
```

后续可根据共享 schema 扩展：

```text
/api/v1/tags
/api/v1/folders
/api/v1/notes/{id}/blocks
/api/v1/notes/{id}/attachments
```

## 数据库策略

- SQLite 为第一阶段数据库。
- `vendor/zembra-schema` 为数据库契约来源。
- migration 优先读取或同步共享 schema 中的 migration。
- 后端 repository 层使用显式 SQL，避免隐藏 schema 差异。
- schema 升级通过 submodule 指针变更驱动，并同步更新数据访问逻辑。

## 非目标

- 第一阶段不引入 PostgreSQL。
- 第一阶段不引入 GraphQL。
- 第一阶段不引入重型 ORM 抽象。
- 第一阶段不实现复杂多用户认证系统。
- 第一阶段不自行维护一份独立于 `vendor/zembra-schema` 的表结构正文。

## 待后续设计确认

- 共享 schema submodule 初始化与版本固定方式。
- 笔记核心表、标签、目录、附件等资源的实际字段映射。
- 认证策略采用无认证、本地 token 还是 Bearer token。
- 软删除、版本号、同步冲突字段是否已由共享 schema 提供。
- migration 在后端启动时自动执行，还是由部署流程显式执行。
