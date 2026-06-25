# r029-health-version-tracking

日期：2026-06-25

需求澄清文档：`docs/request-clarify/r029-health-version-tracking.md`

## 核心功能（WHAT）

为 `zembra-backend-rust` 增加运行时版本追踪能力。版本号继续以 Cargo 官方语义版本字段 `Cargo.toml package.version` 为唯一来源，版本追踪策略元数据写入 `Cargo.toml package.metadata.zembra`，运行时统一通过现有 `GET /health` 响应暴露。

### 需求背景（WHY）

仓库已经有 GitHub Release 流水线设计，要求 tag 与 `Cargo.toml package.version` 一致。运行时缺少版本信息会让部署验证只能判断服务是否存活，不能判断服务实例是否为预期版本。本次只补齐服务运行时可观测性，不改变发布流程和数据库契约。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 版本源单一 | `Cargo.toml package.version` 是唯一语义版本号来源 |
| TOML 闭环 | 版本追踪策略元数据存放在本仓库 `Cargo.toml` 中 |
| 健康检查可追踪 | `/health` 直接返回版本信息 |
| 合同同步 | OpenAPI schema 与响应字段保持一致 |
| 可回归 | 自动化测试覆盖 health 响应新增字段 |

### 范围边界

| 类型 | 内容 |
| --- | --- |
| In Scope | `Cargo.toml package.metadata.zembra`、版本信息读取模块、`HealthResponse` 字段、health route 测试、OpenAPI schema 回归 |
| Out of Scope | 新 endpoint、外部环境变量控制、自动 bump、数据库 schema、Release workflow 改造 |

## 实现流程（HOW）

### Cargo.toml 元数据

| 字段 | 归属 | 说明 |
| --- | --- | --- |
| `package.version` | Cargo 标准字段 | 语义版本号唯一来源 |
| `package.metadata.zembra.version_policy` | 仓库元数据 | 记录版本策略，默认 `semver` |
| `package.metadata.zembra.release_channel` | 仓库元数据 | 记录当前发布通道，默认 `dev` |

### 运行时版本模块

新增 `src/version.rs`，提供一个轻量 `VersionInfo` 结构和 `version_info()` 函数。`version` 使用 Cargo 编译期注入的 `CARGO_PKG_VERSION` 常量读取，`version_policy` 和 `release_channel` 使用编译期 `include_str!("../Cargo.toml")` 读取仓库 TOML 并通过已有 `toml` 依赖解析。这里不读取 `.zembra.env`，也不读取运行时外部环境变量。

| 函数 | 功能 |
| --- | --- |
| `version_info()` | 返回当前服务版本追踪信息 |
| `read_metadata_value()` | 从仓库 TOML 的 `package.metadata.zembra` 中读取指定字符串字段 |

### Health 响应

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `status` | `&'static str` | 现有服务状态 |
| `service` | `&'static str` | 现有服务名 |
| `database_initialized` | `bool` | 现有数据库初始化状态 |
| `version` | `&'static str` | Cargo package version |
| `version_policy` | `String` | 仓库 TOML 中定义的版本策略 |
| `release_channel` | `String` | 仓库 TOML 中定义的发布通道 |

## 测试用例

### 编译检查

| 用例 | 预期结果 |
| --- | --- |
| `cargo fmt --check` | 格式检查通过 |
| `cargo check` | 编译检查通过 |
| `cargo clippy -- -D warnings` | 无 warning |

### 手工检查

| 用例 | 预期结果 |
| --- | --- |
| 请求 `GET /health` | 返回 `version`、`version_policy`、`release_channel` |
| 请求 `/api-docs/openapi.json` | `HealthResponse` schema 包含版本字段 |

### 回归检查

| 用例 | 预期结果 |
| --- | --- |
| health route 集成测试 | 响应状态为 200，且新增字段与 Cargo/TOML 配置一致 |
| OpenAPI route 测试 | 现有 OpenAPI JSON 测试继续通过 |
