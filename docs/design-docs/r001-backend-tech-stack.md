# Zembra 后端工程初始化设计

日期：2026-04-26

需求澄清文档：`docs/request-clarify/r001-backend-tech-stack.md`

## 设计目标

基于已确认技术选型初始化 Rust 后端工程基础设施，为后续 CRUD 开发提供稳定起点。本阶段只建立工程骨架、配置、依赖和可验证的基础服务入口，不实现笔记、标签、目录等业务流程。

## 技术栈

| 类别 | 选择 |
|---|---|
| Rust edition | 2024 |
| Web 框架 | Axum |
| 异步运行时 | Tokio |
| 数据库访问 | SQLx + SQLite |
| 序列化 | Serde / Serde JSON |
| 校验 | Validator |
| 错误处理 | Thiserror |
| 日志 | Tracing / Tracing Subscriber |
| 配置 | Config + TOML |
| OpenAPI | Utoipa / Utoipa Swagger UI |

## 初始化范围

- 创建 `Cargo.toml` 和 `Cargo.lock`。
- 创建 `src/` 基础模块结构。
- 创建 `config/default.toml` 配置样例。
- 创建 `.env.example`。
- 创建 `.gitignore`。
- 提供 `/health` 健康检查接口，用于验证 Axum 服务可启动。
- 保留 `repositories`、`services`、`models`、`dto` 等模块目录入口，不写业务 CRUD 逻辑。

## 非目标

- 不实现 notes CRUD。
- 不实现认证流程。
- 不创建独立业务表结构。
- 不复制 `vendor/zembra-schema` 的 schema 正文。
- 不引入前端或客户端逻辑。
