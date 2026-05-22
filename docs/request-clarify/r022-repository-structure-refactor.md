# r022 仓储结构重构需求澄清

日期：2026-05-22

## 背景

r021 已经完成路由集成测试拆分，并把 `src/app.rs` 从测试聚合文件恢复为路由构建文件；同时 notes 仓储已从单文件移动为目录模块，并拆出 `tests.rs`、`types.rs`、`payloads.rs`、`validation.rs`。剩余问题集中在 notes 和 sync 仓储生产代码：`src/repositories/notes/mod.rs` 仍超过千行，`src/repositories/sync.rs` 仍包含 state、outbox、remote apply、payload 解析等多类职责。

## 仓库现状

| 项目 | 当前结论 |
| --- | --- |
| `src/app.rs` | 已完成测试拆分，文件约 217 行，不再作为本轮重点 |
| `src/repositories/notes/mod.rs` | 仍约 1254 行，包含 CRUD、revision、tag、link、查询和事务 helper |
| `src/repositories/notes/tests.rs` | 已独立承载 notes 仓储单元测试 |
| `src/repositories/notes/types.rs` | 已承载 notes 仓储输入输出结构 |
| `src/repositories/notes/payloads.rs` | 已承载 sync payload 组装函数 |
| `src/repositories/notes/validation.rs` | 已承载 note ref 校验函数 |
| `src/repositories/sync.rs` | 仍约 1036 行，包含 sync public facade、state、outbox、remote apply、payload 字段读取和测试 |

## 已发现问题

| 编号 | 问题 | 影响 |
| --- | --- | --- |
| P1 | notes 仓储 public facade 和具体领域实现仍在 `mod.rs` 中混杂 | 后续新增 notes 能力仍容易回到大文件 |
| P2 | notes 创建和更新事务流程仍串联 note row、revision、tag、link、sync change | 单函数理解成本高，局部修改回归风险大 |
| P3 | note tags、note links、revision 查询与 core note CRUD 边界仍不清晰 | 关联关系逻辑难以独立测试和复用 |
| P4 | sync 仓储仍使用单文件承载多类同步职责 | remote apply 新增实体时容易继续扩大单个文件和 match |
| P5 | sync remote payload 仍通过 `serde_json::Value` 散落读取字段 | 缺字段、类型错误和实体 payload 合同不集中 |
| P6 | note id、revision id、tag id、sync entity id 等仍大量使用裸字符串 | 函数签名表达力不足，类型层面无法降低误传风险 |

## 需求理解

本轮 r022 是 r021 的后续生产代码重构，不新增业务能力。目标是在 r021 已建立的测试保护下，继续拆分 notes 和 sync 仓储生产代码，明确 public facade、领域子模块、事务 helper 和 sync apply 边界，并逐步引入 Rust module privacy、re-export、enum、`TryFrom` 和小粒度 newtype 来收紧代码结构。

## 建议纳入范围

| 项目 | 结论 |
| --- | --- |
| notes core 拆分 | 将 note CRUD、可见性查询、按日期/随机查询等核心 note 查询写入能力拆到 `src/repositories/notes/core.rs` |
| notes revision 拆分 | 将 revision 写入、revision 列表、winner/current revision 相关 helper 拆到 `src/repositories/notes/revisions.rs` |
| notes tags 拆分 | 将 note_tags 查询、添加、移除、替换和 sync change 记录拆到 `src/repositories/notes/tags.rs` |
| notes links 拆分 | 将 note_links 查询、插入、替换、backlinks/outgoing links 拆到 `src/repositories/notes/links.rs` |
| notes facade 收口 | 让 `src/repositories/notes/mod.rs` 只保留 `NotesRepository` facade、模块声明和必要 re-export |
| sync 目录模块 | 将 `src/repositories/sync.rs` 拆为 `sync/mod.rs`、`types.rs`、`state.rs`、`outbox.rs`、`apply.rs`、`payload.rs`、`tests.rs` |
| sync apply 拆分 | 将 remote change 按 entity/operation 拆成独立函数，并以 enum 进行分发 |
| typed payload | 为 sync remote payload 引入 typed struct 和 `TryFrom<&serde_json::Value>` |
| newtype 试点 | 在私有 helper 和事务边界优先引入 note/sync 领域 ID newtype，不扩大到 DTO 和 model |

## 建议不纳入范围

- 不修改 HTTP API、OpenAPI path、DTO 响应结构和数据库 schema。
- 不改变 notes、taxonomy、sync 的业务语义。
- 不引入 ORM 或替换 SQLx。
- 不为抽象引入大型依赖。
- 不一次性将所有 model/DTO 字段改为 newtype。
- 不归档 r021 或 r022 执行计划，归档必须等待用户验收。

## 验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `src/repositories/notes/mod.rs` 成为小型 facade，不再承载大段事务实现 |
| A2 | notes core、revision、tag、link、payload、validation、types、tests 模块职责清晰 |
| A3 | `crate::repositories::notes::*` 对外常用类型和 `NotesRepository` 路径保持稳定 |
| A4 | `src/repositories/sync.rs` 拆成目录模块，`SyncRepository` public facade 路径保持稳定 |
| A5 | `apply_remote_change_in_transaction` 不再是单个超长多实体 match |
| A6 | sync remote payload 字段读取集中到 typed payload 层 |
| A7 | `cargo fmt --check` 通过 |
| A8 | `cargo test repositories::notes repositories::sync` 通过 |
| A9 | `cargo test --all-targets` 通过 |
| A10 | 重构过程中没有改变 HTTP API、OpenAPI、数据库 schema 和业务行为 |

## 需要决策的问题

当前不需要额外决策。r022 的范围已经限定为行为保持型仓储结构重构，优先顺序为 notes 生产代码拆分、sync 生产代码拆分、typed payload/newtype 收口。
