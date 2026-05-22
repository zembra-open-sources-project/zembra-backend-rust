# r021 代码结构重构需求澄清

日期：2026-05-22

## 背景

本轮代码评审发现，当前项目已经出现大文件、长函数和职责边界变宽的问题。问题主要集中在 `notes` 仓储、`sync` 仓储和 `app` 路由集成测试。虽然当前功能仍可运行，但继续在原结构上叠加 notes、sync、taxonomy 能力，会增加维护成本和回归风险。

## 仓库现状

| 项目 | 当前结论 |
| --- | --- |
| `src/repositories/notes.rs` | 文件约 2297 行，生产代码约 1433 行，测试约 864 行，是当前最明显的超大仓储文件 |
| `src/app.rs` | 文件约 1798 行，其中测试约 1771 行，生产代码很少，实际承担了大量 HTTP 集成测试职责 |
| `src/repositories/sync.rs` | 文件约 1036 行，包含 sync state、outbox、remote apply、payload 解析和测试 |
| `src/services/notes.rs` | 文件约 643 行，仍可接受，但已受 notes 仓储职责扩张影响 |
| `src/handlers/notes.rs` | 文件约 597 行，主要由 OpenAPI 标注和 handler 数量造成，暂不作为第一优先级 |
| 测试目录 | 当前没有独立 `tests/` 目录，路由集成测试主要内嵌在 `src/app.rs` |
| 架构文档 | `ARCHITECTURE.md` 目前内容较少，无法提供足够模块边界约束 |

## 已发现问题

| 编号 | 问题 | 影响 |
| --- | --- | --- |
| P1 | `src/repositories/notes.rs` 承担 note CRUD、revision、field/tag、note links、sync change、payload 和校验等多类职责 | 仓储层边界不清晰，新增 notes 能力容易继续放大单文件 |
| P2 | `src/app.rs` 把路由构建和大量 HTTP 集成测试放在同一文件 | 应用入口可读性下降，测试主题难以按业务定位 |
| P3 | `apply_remote_change_in_transaction` 使用超长 `match` 处理多种实体和操作 | sync 新增实体时容易继续扩展单个分发函数 |
| P4 | `create_note_in_transaction` 和 `update_note` 串联过多事务步骤 | 创建和修改流程难以单独验证，也难以复用部分事务逻辑 |
| P5 | notes 和 sync 仓储缺少明确的 public facade 与私有实现边界 | 内部 helper、输入类型和持久化细节容易互相泄漏 |
| P6 | 裸 `String` / `&str` 在 note id、note ref、revision id、sync entity id 等领域标识之间频繁传递 | 类型层面无法阻止误传，事务 helper 签名表达力不足 |
| P7 | remote sync payload 直接通过 `serde_json::Value` 读取字段 | 字段名和校验逻辑散落在 apply 流程中，错误边界不够清晰 |

## 需求理解

本轮需求不是新增业务功能，而是对现有代码结构做行为保持型重构。目标是把测试、notes 仓储、sync 仓储和跨模块职责边界拆清楚，并用 Rust 的模块系统、trait、newtype、enum、`TryFrom`、私有模块和 re-export 等语言特性，让代码更容易扩展和验证。

## 建议纳入范围

| 项目 | 结论 |
| --- | --- |
| 测试拆分 | 将 `src/app.rs` 内嵌 HTTP 集成测试拆到 `tests/`，按 health、CORS、OpenAPI、notes CRUD、notes query、notes taxonomy、notes links、sync config、sync 操作分文件 |
| 测试支架 | 新增 `tests/support`，集中管理 in-memory database、router dispatch、response JSON、notes test builder 等辅助能力 |
| notes 文件拆分 | 将 `src/repositories/notes.rs` 拆为目录模块，至少包含 `mod.rs`、`types.rs`、`core.rs`、`revisions.rs`、`tags.rs`、`links.rs`、`payloads.rs` |
| sync 文件拆分 | 将 `src/repositories/sync.rs` 拆为目录模块，至少包含 `mod.rs`、`types.rs`、`state.rs`、`outbox.rs`、`apply.rs`、`payload.rs` |
| 长函数拆分 | 拆分 `create_note_in_transaction`、`update_note`、`apply_remote_change_in_transaction` 等长函数 |
| trait 边界 | 为 notes 仓储按服务层真实依赖拆分小粒度能力 trait，例如 reader、writer、taxonomy、link 能力 |
| newtype | 在私有 helper 和事务边界逐步引入 `NoteId`、`NoteRef`、`RevisionId`、`TagId`、`FieldId`、`SyncEntityId` 等领域标识 |
| typed payload | 为 sync remote apply 引入 typed payload struct 和 `TryFrom<&serde_json::Value>`，减少散落 JSON 字段读取 |
| 可见性控制 | 使用私有模块、`pub(super)` 和 `pub use` 控制内部实现暴露范围 |

## 建议不纳入范围

- 不修改 HTTP API 路径、method、状态码和响应结构。
- 不修改 OpenAPI 对外合同，除非拆分后路径注册方式需要同步搬迁。
- 不新增数据库迁移。
- 不调整 notes、sync、taxonomy 的业务语义。
- 不引入大型依赖或替换 SQLx。
- 不为了抽象而引入 `async_trait`，除非后续明确需要 trait object 或 mock 边界。
- 不一次性把 DTO、数据库 model 和所有外部接口改成 newtype。
- 不归档当前执行计划，归档必须等待用户验收。

## 验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `src/app.rs` 不再承载大规模 HTTP 集成测试 |
| A2 | 路由集成测试按主题拆分到 `tests/`，且可独立运行 |
| A3 | `src/repositories/notes.rs` 被拆分为职责清晰的目录模块，外部 import 路径保持稳定 |
| A4 | `src/repositories/sync.rs` 被拆分为职责清晰的目录模块，`SyncRepository` public facade 保持稳定 |
| A5 | notes 创建、修改、tag、link、revision、sync change 的原有测试全部通过 |
| A6 | sync remote apply 的幂等、winner revision、note link attach/detach 测试全部通过 |
| A7 | `cargo fmt --check` 通过 |
| A8 | `cargo clippy --all-targets -- -D warnings` 通过，或清楚记录非本次引入的历史阻塞 |
| A9 | `cargo test --all-targets` 通过 |
| A10 | 重构过程中未改变 HTTP API、OpenAPI path、数据库 schema 和业务行为 |

## 需要决策的问题

当前不需要额外决策。范围已经聚焦为行为保持型结构重构，优先级为先拆测试，再拆大文件，最后引入 trait、newtype 和 typed payload 收紧边界。
