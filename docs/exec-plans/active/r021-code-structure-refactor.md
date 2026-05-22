# 代码结构重构执行计划

> 日期：2026-05-22

**目标：** 解决当前项目中的超大测试文件、超大仓储文件、长函数和职责边界不清晰问题，并建立更符合 Rust 模块系统与 trait 抽象的代码结构。

**相关依据：**
- `AGENTS.md`
- `ARCHITECTURE.md`
- `docs/request-clarify/r021-code-structure-refactor.md`
- 代码评审结论：`src/app.rs` 测试过大，`src/repositories/notes.rs`、`src/repositories/sync.rs` 存在文件和函数膨胀。

**架构：** 先做无行为变化的测试拆分和模块搬迁，再按领域边界拆分 notes/sync 仓储实现，最后引入小粒度 trait、newtype 和私有模块，稳定服务层依赖与事务协作边界。

**技术栈：** Rust 2024、Axum、SQLx、Tokio、Utoipa、Cargo integration tests。

**范围：**
- 拆分 `src/app.rs` 中的路由集成测试。
- 拆分 `src/repositories/notes.rs` 和 `src/repositories/sync.rs` 的超大实现。
- 重组 notes、sync 相关仓储职责，充分使用 Rust module、trait、newtype、sealed/private module、extension trait 等语言特性。
- 保持外部 HTTP API、OpenAPI path、DTO 和数据库 schema 行为不变。

**非范围：**
- 不修改业务功能语义。
- 不新增数据库迁移。
- 不调整 HTTP path、状态码、响应结构。
- 不引入大型依赖或 ORM 替代 SQLx。

---

## Phase #1: 建立可拆分测试支架

### Task #1: 创建 HTTP 集成测试公共工具模块

**状态：** Finished

**文件：**
- 创建：`tests/support/mod.rs`
- 创建：`tests/support/app.rs`
- 修改：`Cargo.toml`（仅在测试目标确实需要显式声明时修改）
- 验证：`cargo test health_route_returns_ok`

- 功能：把 `src/app.rs` 中的 `test_state`、`send_with_state`、`send_with_cors`、`response_json`、测试建库逻辑迁移到 integration test support 模块。
- 实现说明：使用 `pub(crate)` 暴露测试辅助函数；保留 `AppState` 构造方式；用 `AtomicUsize` 或临时路径 helper 保持 sync config 测试文件隔离；不把 support 模块暴露给生产代码。
- 预期验证结果：迁移后的第一个健康检查测试通过，生产代码编译不依赖 `tests/support`。
- 完成记录：已新增 `src/lib.rs` 供 integration tests 引用，创建 `tests/support/app.rs`，并通过 `cargo test --test health_routes --test cors_routes --test openapi_routes` 验证。

### Task #2: 建立 notes 测试数据构造器

**状态：** Finished

**文件：**
- 创建：`tests/support/notes.rs`
- 修改：`tests/support/mod.rs`
- 验证：`cargo test create_note_route_returns_created_note`

- 功能：沉淀 `create_note`、`create_tagged_note`、`create_field_note`、`create_note_with_metadata`、`set_updated_at`、`set_created_at` 等测试 helper。
- 实现说明：使用小型 builder struct，例如 `TestNoteBuilder`，通过方法链表达 `content`、`field`、`tags`、`links`；避免在测试中重复构造 `CreateNoteRequest`。
- 预期验证结果：notes 路由测试能复用 builder，单个测试文件不再重复底层 SQL 或服务层创建逻辑。
- 完成记录：已新增 `tests/support/notes.rs`，提供 `TestNoteBuilder` 和 notes 测试数据 helper。

---

## Phase #2: 拆分独立测试

### Task #1: 拆分 CORS、health、OpenAPI 测试

**状态：** Finished

**文件：**
- 创建：`tests/health_routes.rs`
- 创建：`tests/cors_routes.rs`
- 创建：`tests/openapi_routes.rs`
- 修改：`src/app.rs`
- 验证：`cargo test health_routes cors_routes openapi_routes`

- 功能：把 `src/app.rs` 中非 notes 业务测试迁移到独立 integration test 文件。
- 实现说明：`src/app.rs` 保留 `AppState`、`build_router`、`build_router_with_cors`、CORS 私有函数和必要单元测试；HTTP 行为测试全部通过 public router 进行验证。
- 预期验证结果：`src/app.rs` 测试代码明显减少，CORS 和 OpenAPI 行为保持不变。
- 完成记录：已新增 `tests/health_routes.rs`、`tests/cors_routes.rs`、`tests/openapi_routes.rs`，并从 `src/app.rs` 移除对应旧测试。

### Task #2: 按业务主题拆分 notes 路由测试

**状态：** Finished

**文件：**
- 创建：`tests/notes_crud_routes.rs`
- 创建：`tests/notes_query_routes.rs`
- 创建：`tests/notes_taxonomy_routes.rs`
- 创建：`tests/notes_links_routes.rs`
- 修改：`src/app.rs`
- 验证：`cargo test notes_crud_routes notes_query_routes notes_taxonomy_routes notes_links_routes`

- 功能：把 create/update/archive/delete、recent/random/date 查询、field/tag、note links 相关测试从 `src/app.rs` 拆出。
- 实现说明：每个测试文件只验证一个业务主题；共享构造逻辑统一走 `tests/support`；测试命名保留原语义，方便 `cargo test <old_name>` 定位。
- 预期验证结果：`src/app.rs` 不再承担路由集成测试集合职责，HTTP 行为测试仍覆盖原有关键路径。
- 完成记录：已创建 `tests/notes_crud_routes.rs`、`tests/notes_links_routes.rs`、`tests/notes_taxonomy_routes.rs`、`tests/notes_query_routes.rs`，迁移 create、link metadata、patch taxonomy、recent notes、daily counts、notes by date、random notes、random tags、random fields 测试，并通过定向测试与 `cargo test --all-targets`。

### Task #3: 拆分 sync 配置与 sync 操作测试

**状态：** Coding

**文件：**
- 创建：`tests/sync_config_routes.rs`
- 创建：`tests/sync_routes.rs`
- 修改：`src/app.rs`
- 验证：`cargo test sync_config_routes sync_routes`

- 功能：把 sync config、sync status、manual sync 相关测试从 `src/app.rs` 拆出。
- 实现说明：sync config 测试继续使用独立临时 toml 路径；禁止共享全局文件路径；对 disabled sync 的错误响应保留断言。
- 预期验证结果：sync 相关路由测试独立运行稳定，不依赖 notes 路由测试顺序。

---

## Phase #3: 拆分超大文件和长函数

### Task #1: 将 notes 仓储拆成目录模块

**状态：** Designed

**文件：**
- 创建：`src/repositories/notes/mod.rs`
- 创建：`src/repositories/notes/types.rs`
- 创建：`src/repositories/notes/core.rs`
- 创建：`src/repositories/notes/revisions.rs`
- 创建：`src/repositories/notes/tags.rs`
- 创建：`src/repositories/notes/links.rs`
- 创建：`src/repositories/notes/payloads.rs`
- 删除或缩减：`src/repositories/notes.rs`
- 修改：`src/repositories/mod.rs`
- 验证：`cargo test repositories::notes`

- 功能：把 `src/repositories/notes.rs` 从单文件拆为目录模块，保持 `crate::repositories::notes::NotesRepository` 和输入类型路径稳定。
- 实现说明：`mod.rs` 只保留 public type re-export 和 `NotesRepository` facade；`types.rs` 放 `CreateNoteInput`、`UpdateNoteInput`、`NoteLinkInput`；`core.rs` 放 note CRUD；`revisions.rs` 放 revision 写入和 winner 查询；`tags.rs` 放 note_tags 关联；`links.rs` 放 note_links；`payloads.rs` 放 sync payload 组装。
- 预期验证结果：外部调用方无需改 import；每个 notes 子模块职责单一；原仓储测试全部通过。

### Task #2: 拆分 `create_note_in_transaction` 与 `update_note`

**状态：** Designed

**文件：**
- 修改：`src/repositories/notes/core.rs`
- 修改：`src/repositories/notes/revisions.rs`
- 修改：`src/repositories/notes/tags.rs`
- 修改：`src/repositories/notes/links.rs`
- 修改：`src/repositories/notes/payloads.rs`
- 验证：`cargo test create_note_writes_revision_field_and_tags update_note_writes_new_revision update_note_replaces_and_clears_outgoing_links`

- 功能：把创建和更新流程拆成可命名的事务步骤，降低单函数长度。
- 实现说明：提取 `insert_note_row`、`insert_revision_row`、`record_note_insert_change`、`record_revision_insert_change`、`replace_note_taxonomy`、`replace_note_links` 等私有函数；函数参数使用具体 newtype 或轻量上下文 struct，避免传递过多裸字符串。
- 预期验证结果：`create_note_in_transaction` 和 `update_note` 只保留流程编排；行为和 sync change 记录保持一致。

### Task #3: 将 sync 仓储拆成变更应用模块

**状态：** Designed

**文件：**
- 创建：`src/repositories/sync/mod.rs`
- 创建：`src/repositories/sync/types.rs`
- 创建：`src/repositories/sync/state.rs`
- 创建：`src/repositories/sync/outbox.rs`
- 创建：`src/repositories/sync/apply.rs`
- 创建：`src/repositories/sync/payload.rs`
- 删除或缩减：`src/repositories/sync.rs`
- 修改：`src/repositories/mod.rs`
- 验证：`cargo test repositories::sync`

- 功能：把 sync 仓储拆成 state 管理、outbox 查询、remote apply、payload 解析四个边界。
- 实现说明：`mod.rs` 保持 `SyncRepository` public facade；`apply.rs` 处理远端 change 应用；`payload.rs` 提供 typed payload parser；`state.rs` 管理 sync state；`outbox.rs` 管理待推送 change。
- 预期验证结果：`SyncRepository` public API 不变，`apply_remote_change_in_transaction` 不再是单个超长 match。

### Task #4: 拆分 remote change 应用分支

**状态：** Designed

**文件：**
- 修改：`src/repositories/sync/apply.rs`
- 修改：`src/repositories/sync/payload.rs`
- 验证：`cargo test apply_remote_changes_is_idempotent apply_remote_revision_selects_deterministic_winner apply_remote_note_link_attach_and_detach`

- 功能：将 field、tag、note、note_revision、note_tag、note_link 的应用逻辑拆成独立函数。
- 实现说明：定义 `RemoteEntityKind` 和 `RemoteOperation` enum，并实现 `TryFrom<&SyncChangeRecord>`；match 只负责分发到 `apply_field_insert`、`apply_note_upsert`、`apply_revision_insert`、`apply_note_tag_attach`、`apply_note_link_detach` 等函数。
- 预期验证结果：新增实体时只需新增 enum 分支和对应函数，不需要扩展一个超长函数。

---

## Phase #4: 使用 Rust 语言特性重组职责边界

### Task #1: 引入仓储能力 trait，稳定服务层依赖

**状态：** Designed

**文件：**
- 创建：`src/repositories/notes/ports.rs`
- 修改：`src/services/notes.rs`
- 修改：`src/repositories/notes/mod.rs`
- 验证：`cargo test services::notes`

- 功能：把 `NotesService` 依赖的仓储能力拆成小 trait，例如 `NoteReader`、`NoteWriter`、`NoteTaxonomyWriter`、`NoteLinkReader`。
- 实现说明：trait 使用 async method 时优先采用返回 `impl Future` 的稳定写法或保留 concrete repository 注入，避免为抽象而引入 `async_trait` 依赖；trait 只覆盖服务层真实需要的能力。
- 预期验证结果：服务层业务编排与仓储实现细节解耦，测试可逐步引入轻量 fake。

### Task #2: 用 newtype 表达领域标识

**状态：** Designed

**文件：**
- 创建：`src/repositories/notes/ids.rs`
- 创建：`src/repositories/sync/ids.rs`
- 修改：`src/repositories/notes/*.rs`
- 修改：`src/repositories/sync/*.rs`
- 验证：`cargo test validate_note_ref validate_full_note_uuid list_recent_notes_uses_full_note_uuid_cursor`

- 功能：减少裸 `String` / `&str` 在 note id、note ref、revision id、tag id、sync entity id 之间误传的风险。
- 实现说明：定义 `NoteId`、`NoteRef`、`RevisionId`、`TagId`、`FieldId`、`SyncEntityId` 等 newtype；先在私有 helper 和事务边界使用，不一次性改 DTO 和数据库模型。
- 预期验证结果：核心事务 helper 的函数签名更清晰，外部 API 不受影响。

### Task #3: 用 typed payload 代替散落的 serde_json 字段读取

**状态：** Designed

**文件：**
- 创建：`src/repositories/sync/payload_types.rs`
- 修改：`src/repositories/sync/apply.rs`
- 修改：`src/repositories/sync/payload.rs`
- 验证：`cargo test apply_remote_changes_is_idempotent apply_remote_note_link_attach_and_detach`

- 功能：把 `required_text`、`optional_text`、`required_i64` 等字段读取收敛到 typed payload。
- 实现说明：为 `FieldPayload`、`TagPayload`、`NotePayload`、`NoteRevisionPayload`、`NoteTagPayload`、`NoteLinkPayload` 实现 `TryFrom<&serde_json::Value>`；错误类型使用小型 enum 并实现 `Display`。
- 预期验证结果：remote apply 函数不再直接操作 JSON 字段名，缺字段错误仍可读。

### Task #4: 用私有模块和 re-export 控制可见性

**状态：** Designed

**文件：**
- 修改：`src/repositories/notes/mod.rs`
- 修改：`src/repositories/sync/mod.rs`
- 修改：`src/repositories/mod.rs`
- 验证：`cargo test --all-targets`

- 功能：通过 Rust module privacy 限制内部 helper 泄漏，保持 public API 小而稳定。
- 实现说明：只 `pub use` 需要被 service、handler、测试直接使用的类型；内部事务 helper 使用 `pub(super)` 或私有函数；避免 `pub mod` 暴露实现细节。
- 预期验证结果：外部模块只能看到 facade 和输入输出类型，内部拆分不会扩大公共 API。

---

## Phase #5: 收口验证和文档更新

### Task #1: 全量验证

**状态：** Designed

**文件：**
- 验证：`cargo fmt --check`
- 验证：`cargo clippy --all-targets -- -D warnings`
- 验证：`cargo test --all-targets`

- 功能：确认重构没有引入格式、lint 和行为回归。
- 实现说明：先跑定向测试，再跑全量；如果 clippy 暴露已有历史问题，只修复本次重构直接引入的问题，其他问题记录到技术债。
- 预期验证结果：格式检查、clippy、全量测试通过，或清楚记录非本次引入的阻塞项。

### Task #2: 更新结构纪律和技术债记录

**状态：** Designed

**文件：**
- 修改：`docs/PROGRESS.md`
- 修改：`docs/exec-plans/tech-debt-tracker.md`（如存在剩余问题）
- 修改：`docs/exec-plans/active/r021-code-structure-refactor.md`

- 功能：记录重构完成情况、验证结果和剩余结构债。
- 实现说明：每完成一个 Phase 更新本计划状态；只有用户验收后才能移动到 `docs/exec-plans/completed/`。
- 预期验证结果：文档能准确反映已完成拆分和仍需后续处理的边界。
