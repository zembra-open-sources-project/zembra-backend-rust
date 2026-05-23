# r022 仓储结构重构执行计划

> 日期：2026-05-22

**目标：** 承接 r021 已完成的测试拆分，继续拆分 notes 和 sync 仓储生产代码，降低大文件、长函数和职责混杂风险。

**相关需求澄清：** `docs/request-clarify/r022-repository-structure-refactor.md`

**相关设计文档：** `docs/design-docs/r022-repository-structure-refactor.md`

**架构：** 通过 Rust 目录模块、`pub use` facade、`pub(super)` helper、typed payload、enum 分发和小范围 newtype，把 notes/sync 仓储拆成 core、revisions、tags、links、state、outbox、apply、payload 等边界。

**技术栈：** Rust 2024、SQLx、Tokio、serde_json、Cargo tests。

**范围 / 非范围：**
- 范围：notes 仓储生产代码拆分、sync 仓储生产代码拆分、remote apply 分支拆分、typed payload/newtype 试点、计划和进度记录。
- 非范围：HTTP API、OpenAPI、DTO、数据库 schema、业务语义变更。

---

## Phase #1: notes 仓储生产代码拆分

### Task #1: 完成 notes core 模块拆分

**状态：** Finished

**文件：**
- 创建：`src/repositories/notes/core.rs`
- 修改：`src/repositories/notes/mod.rs`
- 验证：`cargo test repositories::notes`

- 功能：迁移 note row 创建、读取、更新、归档、删除、recent/random/date 查询和可见性查询。
- 实现说明：保持 `NotesRepository` public method 名称不变；`mod.rs` 只做 facade 委托；SQL 文本、bind 顺序、可见性过滤条件不变。
- 预期结果：`src/repositories/notes/mod.rs` 明显变薄，notes 查询和 CRUD 行为保持不变。

- 完成记录：已将原 `src/repositories/notes/mod.rs` 机械迁移为 `src/repositories/notes/core.rs`，新的 `mod.rs` 只保留模块声明、`NotesRepository` re-export、类型 re-export 和测试模块声明。
- 验证结果：`cargo test repositories::notes` 通过。

### Task #2: 拆分 notes revisions 模块

**状态：** Finished

**文件：**
- 创建：`src/repositories/notes/revisions.rs`
- 修改：`src/repositories/notes/mod.rs`
- 修改：`src/repositories/notes/core.rs`
- 验证：`cargo test create_note_writes_revision_field_and_tags update_note_writes_new_revision`

- 功能：迁移 revision 插入、revision 列表和更新 current revision 相关 helper。
- 实现说明：revision helper 使用 `pub(super)` 暴露给 core；保持 `note_revisions` 写入字段和 sync payload 语义不变。
- 预期结果：创建和更新 note 的 revision 行为保持一致。

- 完成记录：已创建 `src/repositories/notes/revisions.rs`，迁移 `list_note_revisions`，并将 revision 插入逻辑收敛为 `insert_note_revision_in_transaction`。
- 验证结果：`cargo test repositories::notes` 通过。

### Task #3: 拆分 notes tags 模块

**状态：** Finished

**文件：**
- 创建：`src/repositories/notes/tags.rs`
- 修改：`src/repositories/notes/mod.rs`
- 修改：`src/repositories/notes/core.rs`
- 验证：`cargo test tag_association_is_idempotent_and_removable update_note_sets_field_and_replaces_tags update_note_can_clear_all_tag_associations`

- 功能：迁移 `list_note_tags`、`list_note_tags_by_id`、`add_tag_to_note`、`remove_tag_from_note`、`replace_note_tags_in_transaction`。
- 实现说明：tag sync change 继续复用 `payloads.rs`；事务 helper 保持 `pub(super)`。
- 预期结果：tag 关联、替换、移除和 sync outbox 行为保持一致。

- 完成记录：已创建 `src/repositories/notes/tags.rs`，迁移 tag 查询、添加、移除和替换逻辑，保留 note_tag sync payload 和 attach/detach 语义。
- 验证结果：`cargo test repositories::notes` 通过。

### Task #4: 拆分 notes links 模块

**状态：** Finished

**文件：**
- 创建：`src/repositories/notes/links.rs`
- 修改：`src/repositories/notes/mod.rs`
- 修改：`src/repositories/notes/core.rs`
- 验证：`cargo test create_note_writes_outgoing_links update_note_replaces_and_clears_outgoing_links note_links_reject_hidden_and_self_targets`

- 功能：迁移 outgoing links、backlinks、link insert/replace/select helper。
- 实现说明：link 校验继续只允许可见目标 note，保持自引用拒绝和 sync change attach/detach 语义。
- 预期结果：note links API 和仓储测试保持通过。

- 完成记录：已创建 `src/repositories/notes/links.rs`，迁移 outgoing links、backlinks、link insert/replace/select 相关逻辑。
- 验证结果：`cargo test repositories::notes` 通过。

### Task #5: 拆分 `create_note_in_transaction` 与 `update_note`

**状态：** Finished

**文件：**
- 修改：`src/repositories/notes/core.rs`
- 修改：`src/repositories/notes/revisions.rs`
- 修改：`src/repositories/notes/tags.rs`
- 修改：`src/repositories/notes/links.rs`
- 验证：`cargo test repositories::notes`

- 功能：把创建和更新流程拆成命名清晰的事务步骤。
- 实现说明：提取 `insert_note_row`、`update_note_row`、`record_note_insert_change`、`record_note_update_change`、`record_revision_insert_change` 等 helper。
- 预期结果：长函数缩短，事务流程仍由 core 编排。

- 完成记录：已提取 `insert_note_row_in_transaction`、`update_note_row_in_transaction`，并让 revision、tag、link 事务步骤委托给对应子模块 helper。
- 验证结果：`cargo test repositories::notes`、`cargo fmt --check` 和 `cargo test --all-targets` 通过。

---

## Phase #2: sync 仓储目录化

### Task #1: 将 sync 仓储迁移为目录模块

**状态：** Designed

**文件：**
- 创建：`src/repositories/sync/mod.rs`
- 创建：`src/repositories/sync/types.rs`
- 创建：`src/repositories/sync/tests.rs`
- 删除或缩减：`src/repositories/sync.rs`
- 修改：`src/repositories/mod.rs`
- 验证：`cargo test repositories::sync`

- 功能：保持 `crate::repositories::sync::SyncRepository`、`SyncChangeInput`、`SyncChangeRecord`、`SyncStateRecord` public 路径稳定。
- 实现说明：先做机械搬迁和 re-export，不改 SQL 和行为。
- 预期结果：sync 目录结构建立，原 sync 测试通过。

### Task #2: 拆分 sync state 和 outbox

**状态：** Designed

**文件：**
- 创建：`src/repositories/sync/state.rs`
- 创建：`src/repositories/sync/outbox.rs`
- 修改：`src/repositories/sync/mod.rs`
- 验证：`cargo test repositories::sync`

- 功能：将 state 查询/记录和 sync_changes outbox 读写拆开。
- 实现说明：`state.rs` 负责 `get_or_create_state`、`record_success`、`record_error`、`list_states`；`outbox.rs` 负责 pending push、mark success、record change。
- 预期结果：SyncRepository facade 委托清晰，外部调用不变。

### Task #3: 拆分 sync remote apply

**状态：** Designed

**文件：**
- 创建：`src/repositories/sync/apply.rs`
- 修改：`src/repositories/sync/mod.rs`
- 验证：`cargo test apply_remote_changes_is_idempotent apply_remote_revision_selects_deterministic_winner apply_remote_note_link_attach_and_detach`

- 功能：将 remote apply 从单个超长 match 拆成实体级函数。
- 实现说明：先保持 string match 语义，再引入 enum 分发；每个实体函数只处理一种实体/操作组合。
- 预期结果：remote apply 行为保持不变，新增实体不再扩展一个超长函数。

---

## Phase #3: typed payload 与 newtype 收口

### Task #1: 引入 sync typed payload

**状态：** Designed

**文件：**
- 创建：`src/repositories/sync/payload.rs`
- 修改：`src/repositories/sync/apply.rs`
- 验证：`cargo test repositories::sync`

- 功能：把 `required_text`、`optional_text`、`required_i64`、`optional_i64` 收敛到 typed payload struct。
- 实现说明：为 field、tag、note、note_revision、note_tag、note_link 定义 payload struct，并实现 `TryFrom<&serde_json::Value>`。
- 预期结果：apply 逻辑不再散落 JSON 字段读取。

### Task #2: 引入 remote entity/operation enum

**状态：** Designed

**文件：**
- 修改：`src/repositories/sync/apply.rs`
- 修改：`src/repositories/sync/types.rs`
- 验证：`cargo test repositories::sync`

- 功能：用 `RemoteEntityKind` 和 `RemoteOperation` 表达 remote change 分发。
- 实现说明：实现 `TryFrom<&SyncChangeRecord>`，未知组合继续返回现有 unsupported 错误语义。
- 预期结果：remote apply 分发清晰，错误语义不变。

### Task #3: notes/sync 私有 newtype 试点

**状态：** Designed

**文件：**
- 创建：`src/repositories/notes/ids.rs`
- 创建：`src/repositories/sync/ids.rs`
- 修改：`src/repositories/notes/*.rs`
- 修改：`src/repositories/sync/*.rs`
- 验证：`cargo test --all-targets`

- 功能：在私有 helper 和事务边界使用 `NoteId`、`NoteRef`、`RevisionId`、`SyncEntityId` 等 newtype。
- 实现说明：不修改 DTO/model public 字段；通过 `AsRef<str>` 或显式 accessor 降低调用噪音。
- 预期结果：关键 helper 签名表达力增强，外部 API 不变。

---

## Phase #4: 验证与记录

### Task #1: 全量验证

**状态：** Designed

**文件：**
- 验证：`cargo fmt --check`
- 验证：`cargo test repositories::notes repositories::sync`
- 验证：`cargo test --all-targets`

- 功能：确认仓储拆分没有行为回归。
- 实现说明：先跑定向仓储测试，再跑全量测试；如 clippy 暴露历史问题，只记录非本次引入项。
- 预期结果：格式检查和测试通过。

### Task #2: 更新进度记录

**状态：** Designed

**文件：**
- 修改：`docs/PROGRESS.md`
- 修改：`docs/exec-plans/active/r022-repository-structure-refactor.md`

- 功能：记录 r022 执行进度和剩余风险。
- 实现说明：完成阶段后更新任务状态；未经用户验收不移动到 completed。
- 预期结果：进度文档能反映 r022 当前状态。
