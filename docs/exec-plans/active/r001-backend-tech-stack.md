# Zembra 后端工程初始化执行计划

日期：2026-04-26

需求澄清文档：`docs/request-clarify/r001-backend-tech-stack.md`
设计文档：`docs/design-docs/r001-backend-tech-stack.md`

## Stage 1：工程骨架初始化

| Task | 状态 | 功能 | 实现要点 | 预期测试结果 |
|---|---|---|---|---|
| T1 | Finished | 初始化 Cargo 工程 | 使用 Rust 2024 edition，加入已确认基础依赖 | `cargo check` 通过 |
| T2 | Finished | 建立基础模块 | 创建 app、config、error、routes、handlers 等模块入口 | 编译通过，模块无业务流程 |
| T3 | Finished | 增加基础配置 | 提供 TOML 配置样例和环境变量样例 | 服务可读取默认配置 |
| T4 | Finished | 增加健康检查 | 提供 `/health` 接口用于启动验证 | 测试通过 |

## 执行记录

- 初始化 Cargo 二进制工程，使用 Rust 2024 edition。
- 加入 Axum、Tokio、SQLx、Serde、Validator、Thiserror、Tracing、Config、Utoipa 基础依赖。
- 创建 `src/` 基础模块结构，只保留业务层入口，不实现 notes CRUD。
- 添加 `config/default.toml`、`.env.example` 和 `.gitignore` 基础配置。
- 添加 `/health` 健康检查接口和路由测试。

## 约束

- 不实现 notes CRUD。
- 不实现认证和同步流程。
- 不维护独立 schema 正文。
- 只提交必要工程初始化代码。
