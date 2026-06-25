# r035 CRUD API workspace query 参数执行计划

需求澄清文档：`docs/request-clarify/r035-workspace-query-crud-api.md`

设计文档：`docs/design-docs/r035-workspace-query-crud-api.md`

## Stage #1: workspace 请求上下文

### Task #1: 定义 workspace query 与 active workspace 校验

**Status:** Finished

**Files:** Create/Modify `src/dto/workspaces.rs`, `src/repositories/workspaces.rs`, `src/error.rs`, tests

功能 / 实现说明 / 预期验证结果：新增或扩展 `WorkspaceQuery`，包含 required `workspace_id` query 字段，并提供 active workspace 校验入口。校验必须确认 `workspaces.id = ? AND archived_at IS NULL AND deleted_at IS NULL`；缺失、非法 UUID、不存在、归档、删除 workspace 都映射为 `404`。验证覆盖 active workspace 通过、缺失 query 返回 `404`、非法 UUID 返回 `404`、不存在 workspace 返回 `404`、归档和删除 workspace 返回 `404`。

### Task #2: 为 notes handler 引入 workspace 上下文

**Status:** Finished

**Files:** Modify `src/handlers/notes.rs`, `src/dto/notes.rs`, tests

功能 / 实现说明 / 预期验证结果：所有纳入范围的 notes handler 都从 URL query 读取 `workspace_id`，并在调用 service 前完成 active workspace 校验。现有带 `limit`、`date`、`n` 等参数的 query DTO 合并 `workspace_id` 字段，避免同一 handler 多个 `Query` extractor 重复解析 query string。验证覆盖所有纳入接口缺失 `workspace_id` 时返回 `404`。

## Stage #2: service 与 repository workspace 传递

### Task #1: NotesService 全入口显式传递 workspace id

**Status:** Finished

**Files:** Modify `src/services/notes.rs`

功能 / 实现说明 / 预期验证结果：`NotesService` 的 list、create、batch create、recent、daily counts、by-date、random notes/tags/fields、get、update、archive、delete、note tags、note revisions 等入口都接收显式 workspace id，并把它传给 repository。service 不读取 `DEFAULT_WORKSPACE_ID`，normalize create/update 请求只处理业务 payload，不负责 workspace fallback。验证通过编译和 service 调用点全部更新。

### Task #2: NotesRepository CRUD 与查询改为请求 workspace

**Status:** Finished

**Files:** Modify `src/repositories/notes/core.rs`, `src/repositories/notes/revisions.rs`, `src/repositories/notes/tags.rs`, `src/repositories/notes/links.rs`, `src/repositories/notes/payloads.rs`, `src/repositories/notes/types.rs`, `src/repositories/notes/tests.rs`

功能 / 实现说明 / 预期验证结果：所有 notes repository 方法增加 workspace id 参数。已有 `workspace_id = ?` SQL 保留，bind 值从 `DEFAULT_WORKSPACE_ID` 改成请求 workspace id。create/update/archive/delete、note_ref 解析、revision 查询、tag attach/detach/replace、link insert/replace/backlinks/outgoing links 都只作用于指定 workspace。验证覆盖跨 workspace 隔离：列表只返回指定 workspace；详情、更新、删除只在指定 workspace 解析 note_ref；其他 workspace 的同前缀 note 不造成 ambiguous。

### Task #3: taxonomy helper 去除 CRUD 路径默认 workspace 依赖

**Status:** Finished

**Files:** Modify `src/repositories/taxonomy.rs`, `src/repositories/notes/core.rs`, `src/repositories/notes/tags.rs`, tests

功能 / 实现说明 / 预期验证结果：`get_or_create_field_in_transaction`、`get_or_create_tag_in_transaction`、field/tag 查询和层级 tag 创建都接收 workspace id。创建 note、更新 field、添加或替换 tags 时，field/tag 只在请求 workspace 内查找和创建。验证覆盖同名 field/tag 在不同 workspace 下互不复用。

### Task #4: sync_change payload 使用请求 workspace id

**Status:** Finished

**Files:** Modify `src/repositories/notes/payloads.rs`, `src/repositories/notes/core.rs`, `src/repositories/notes/tags.rs`, `src/repositories/notes/links.rs`, tests

功能 / 实现说明 / 预期验证结果：`note_payload`、`note_tag_payload`、`note_link_payload` 和 inline note_revision payload 都使用请求 workspace id，不再从 `DEFAULT_WORKSPACE_ID` 读取 CRUD workspace。验证通过数据库查询 `sync_changes.payload`，确认 create、update、tag attach/detach、link attach/detach 的 payload 中 `workspace_id` 等于 query 参数。

## Stage #3: REST API 合同与文档同步

### Task #1: OpenAPI required workspace_id query 参数

**Status:** Finished

**Files:** Modify `src/handlers/notes.rs`, `src/dto/notes.rs`, `src/api_doc.rs`, `tests/openapi_routes.rs`

功能 / 实现说明 / 预期验证结果：所有纳入范围 handler 的 `#[utoipa::path]` 暴露 required `workspace_id` query 参数。已有 query DTO 需要派生或实现 `IntoParams`，并准确表达 `workspace_id` 为 required。验证 `GET /api-docs/openapi.json` 中所有受影响 path/method 都包含 required `workspace_id` query parameter。

### Task #2: REST API Markdown 文档同步

**Status:** Finished

**Files:** Modify `docs/http-client-server-api.md`

功能 / 实现说明 / 预期验证结果：更新 notes CRUD、recent、daily counts、by-date、random notes/tags/fields、tags、revisions 等接口说明和示例 URL，明确客户端先通过 `GET /workspaces` 获取完整 workspace id，再在受影响接口中使用 `?workspace_id=<uuid>`。验证文档中不再出现无 workspace_id 的受影响 notes URL 示例。

## Stage #4: 自动化测试与回归验证

### Task #1: 路由测试按新合同重写

**Status:** Finished

**Files:** Modify `tests/notes_crud_routes.rs`, `tests/notes_query_routes.rs`, `tests/notes_taxonomy_routes.rs`, `tests/notes_links_routes.rs`, `tests/openapi_routes.rs`, `tests/support/notes.rs`, `tests/support/app.rs`

功能 / 实现说明 / 预期验证结果：所有 notes 路由测试请求都显式传 `workspace_id`。旧的无 `workspace_id` 成功断言必须改为 `404` 或改为带 workspace id 的新成功路径。新增归档 workspace、删除 workspace、跨 workspace note_ref 解析、跨 workspace field/tag 隔离、sync payload workspace id 测试。验证通过 `cargo test --test notes_crud_routes --test notes_query_routes --test notes_taxonomy_routes --test notes_links_routes --test openapi_routes`。

### Task #2: repository 单元测试按新 workspace 合同重写

**Status:** Finished

**Files:** Modify `src/repositories/notes/tests.rs`, `src/repositories/sync/tests.rs` if affected

功能 / 实现说明 / 预期验证结果：repository 测试不再依赖 CRUD 路径隐式 `DEFAULT_WORKSPACE_ID`，改为显式创建 workspace fixture 并传入 workspace id。只保留非 CRUD/sync 内部路径仍需要的 legacy 常量用例。验证通过 `cargo test repositories::notes` 和受影响 sync repository 测试。

### Task #3: 完整验证、计划回写和进度记录

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r035-workspace-query-crud-api.md`, `docs/PROGRESS.md`

功能 / 实现说明 / 预期验证结果：完成实现后运行 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`。所有验证通过后更新本执行计划任务状态和 `docs/PROGRESS.md`，按项目规则提交并尝试推送。预期结果为完整回归通过，工作区只包含本需求相关改动。

验证记录：2026-06-25 已通过 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`。
