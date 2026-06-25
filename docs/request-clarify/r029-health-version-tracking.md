# r029-health-version-tracking

日期：2026-06-25

## 需求背景

当前仓库已有 tag 驱动 GitHub Release 设计，版本来源明确为 `Cargo.toml package.version`，Release tag 需要与 Cargo 版本一致。现有运行时 `/health` 只返回服务状态、服务名和数据库初始化状态，不能直接判断当前进程运行的是哪个后端版本。

## 需求目标

本次需求为后端增加版本号追踪机制，并把运行时版本信息收敛到现有 `GET /health` 响应中。版本号和版本追踪配置必须闭环在本仓库 TOML 中，不使用 `.zembra.env`、CI 环境变量或运行时外部环境变量作为版本控制入口。

## 范围边界

### In Scope

| 项目 | 说明 |
| --- | --- |
| 版本唯一来源 | 使用 `Cargo.toml` 中的 `[package].version` 作为语义版本号来源 |
| 版本策略元数据 | 在 `Cargo.toml` 中记录仓库内闭环的版本追踪策略元数据 |
| `/health` 响应 | 在现有 `GET /health` 返回体中增加版本追踪字段 |
| OpenAPI | 同步维护 `HealthResponse` schema |
| 测试 | 增加 health route 响应字段测试 |

### Out of Scope

| 项目 | 原因 |
| --- | --- |
| 新增 `/version` endpoint | 用户已确认版本信息收敛到 `/health` |
| 外部环境变量控制版本 | 用户明确要求在本仓库 TOML 中闭环 |
| 自动版本号递增工具 | 本次只建立版本追踪机制，不改发版流程 |
| 数据库 schema 变更 | 版本追踪只属于服务运行时信息，不涉及数据库契约 |

## 验收标准

| 编号 | 标准 |
| --- | --- |
| A1 | `GET /health` 返回体包含 `version` 字段，值与 `Cargo.toml package.version` 一致 |
| A2 | `GET /health` 返回体包含仓库 TOML 中定义的版本追踪策略字段 |
| A3 | OpenAPI JSON 中 `HealthResponse` schema 包含新增字段 |
| A4 | 不新增 `.zembra.env` 配置项，不依赖外部环境变量控制版本 |
| A5 | `cargo fmt --check`、`cargo check`、`cargo test` 和 `cargo clippy -- -D warnings` 通过 |

## 已确认决策

| 决策 | 结论 |
| --- | --- |
| 版本号来源 | `Cargo.toml package.version` |
| 配置闭环 | 版本控制和追踪策略只放在仓库 TOML 中 |
| 运行时入口 | 版本信息收敛到 `GET /health` |
| 外部环境变量 | 不作为版本控制入口 |
