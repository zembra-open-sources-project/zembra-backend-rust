# r022 仓储结构重构设计文档

日期：2026-05-22

关联需求澄清：`docs/request-clarify/r022-repository-structure-refactor.md`

## 核心功能（WHAT）

继续拆分 notes 和 sync 仓储生产代码，把当前大文件中的核心实体、关联关系、revision、sync outbox、remote apply 和 payload 解析重组为职责清晰的 Rust 目录模块。

### 需求背景（WHY）

r021 已经把路由集成测试迁移到 `tests/`，并对 notes 仓储完成第一层目录化。接下来需要处理真正影响维护性的生产代码边界：notes 仓储仍包含多种领域逻辑，sync 仓储仍把 state、outbox、remote apply 和 payload 字段读取放在同一文件中。若不继续拆分，后续 notes/sync 需求会继续扩大单文件和长函数。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| notes facade 收口 | `src/repositories/notes/mod.rs` 只保留模块声明、re-export、`NotesRepository` facade 和少量协调逻辑 |
| notes 子模块清晰 | core、revisions、tags、links、payloads、validation、types、tests 分别承担单一职责 |
| sync 目录化 | 将 `src/repositories/sync.rs` 拆成 state、outbox、apply、payload、types、tests 等模块 |
| remote apply 可扩展 | 用 enum 和独立函数替代超长多实体 match |
| 类型表达增强 | 在私有边界试点 newtype 和 typed payload，减少裸字符串和散落 JSON 字段读取 |
| 行为保持 | HTTP API、OpenAPI、schema、业务语义完全不变 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| notes core | note CRUD、visible note 查询、recent/random/by-date 查询 |
| notes revisions | revision 写入、revision 列表、current revision 相关 helper |
| notes tags | note_tags 查询、替换、添加、移除和 sync change 记录 |
| notes links | note_links outgoing/backlink 查询、插入、替换和 sync change 记录 |
| notes facade | 保持 `NotesRepository` public API，内部委托到子模块 |
| sync state/outbox | sync state 与待推送 change 查询/标记拆分 |
| sync apply/payload | remote change 应用和 payload 解析拆分 |
| 验证 | 定向仓储测试和全量测试 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| API 行为 | 不改变 routes、handlers、DTO、OpenAPI |
| 数据库 | 不新增 migration，不改 SQL schema |
| 同步协议 | 不改变 sync_changes entity/operation 合同 |
| 依赖 | 不新增大型抽象依赖 |
| 全局 newtype | 不把 DTO/model 全部改为 newtype |

## 实现流程（HOW）

### notes 模块设计

| 模块 | 职责 |
| --- | --- |
| `src/repositories/notes/mod.rs` | `NotesRepository` facade、模块声明、public re-export |
| `src/repositories/notes/types.rs` | `CreateNoteInput`、`CreatedNote`、`UpdateNoteInput`、`NoteLinkInput`、`DailyNoteCountRow` |
| `src/repositories/notes/core.rs` | note row 创建/更新/归档/删除、可见 note 查询、recent/random/date 查询 |
| `src/repositories/notes/revisions.rs` | revision 插入、列表、current revision 更新相关 helper |
| `src/repositories/notes/tags.rs` | note_tags 查询、添加、移除、替换和 sync 记录 |
| `src/repositories/notes/links.rs` | note_links 查询、插入、替换、backlinks/outgoing links |
| `src/repositories/notes/payloads.rs` | note、note_tag、note_link sync payload 组装 |
| `src/repositories/notes/validation.rs` | note ref、full note uuid 校验 |
| `src/repositories/notes/tests.rs` | notes repository 单元测试 |

设计原则：

| 原则 | 说明 |
| --- | --- |
| facade 稳定 | service 层继续通过 `NotesRepository` 调用，不要求上层感知内部拆分 |
| 内部可见性 | 子模块 helper 默认私有，跨子模块需要时使用 `pub(super)` |
| 无行为变更 | 优先做搬迁和 helper 提取，不改变 SQL 条件和 sync payload |
| 小步提交 | 每拆一个边界跑定向测试并提交 |

### sync 模块设计

| 模块 | 职责 |
| --- | --- |
| `src/repositories/sync/mod.rs` | `SyncRepository` facade、模块声明、public re-export |
| `src/repositories/sync/types.rs` | `SyncChangeInput`、`SyncChangeRecord`、`SyncStateRecord` |
| `src/repositories/sync/state.rs` | sync state 查询、创建、成功/错误记录 |
| `src/repositories/sync/outbox.rs` | pending push 查询、push success 标记、sync change 记录 |
| `src/repositories/sync/apply.rs` | remote change 应用分发和实体处理函数 |
| `src/repositories/sync/payload.rs` | typed payload struct、字段解析、payload 错误 |
| `src/repositories/sync/tests.rs` | sync repository 单元测试 |

remote apply 设计：

| 元素 | 设计 |
| --- | --- |
| Entity enum | `RemoteEntityKind` 表示 field、tag、note、note_revision、note_tag、note_link |
| Operation enum | `RemoteOperation` 表示 insert、update、delete、restore、attach、detach |
| 分发 | `TryFrom<&SyncChangeRecord>` 解析 entity/operation，`apply.rs` 中按 enum 组合分派 |
| Payload | 每类实体对应 typed payload，并实现 `TryFrom<&serde_json::Value>` |
| 错误 | payload 缺字段和 SQL 错误继续转为可读字符串，不改变现有 conflict 记录语义 |

### newtype 试点

| 类型 | 使用边界 |
| --- | --- |
| `NoteId` | notes 私有 helper 参数 |
| `NoteRef` | note ref 校验后传递 |
| `RevisionId` | revision 写入和 sync change 组装 |
| `SyncEntityId` | sync apply 和 outbox 私有 helper |

newtype 只在私有 helper 和事务边界试点，保留 DTO/model 的 `String` 字段，避免扩大重构面。

## 测试策略

| 验证 | 预期 |
| --- | --- |
| `cargo test repositories::notes` | notes 仓储行为保持不变 |
| `cargo test repositories::sync` | sync 仓储行为保持不变 |
| `cargo test --test notes_crud_routes --test notes_query_routes --test notes_taxonomy_routes --test notes_links_routes` | notes HTTP 行为保持不变 |
| `cargo test --test sync_config_routes --test sync_routes` | sync HTTP 行为保持不变 |
| `cargo fmt --check` | 格式通过 |
| `cargo test --all-targets` | 全量测试通过 |

## 风险与控制

| 风险 | 控制 |
| --- | --- |
| 模块循环依赖 | 先拆纯类型、payload、validation，再拆 tags/links/revisions，最后收 facade |
| public path 破坏 | 通过 `pub use` 保持 `crate::repositories::notes::*` 和 `crate::repositories::sync::*` 稳定 |
| SQL 行为变化 | 搬迁时不改 SQL 文本和 bind 顺序 |
| sync payload 变化 | typed payload 引入前后保留原字段名和错误语义 |
| 重构过大 | 每个阶段单独测试和提交 |
