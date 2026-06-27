# r036 field 删除接口执行计划

需求澄清文档：`docs/request-clarify/r036-field-delete-api.md`

设计文档：`docs/design-docs/r036-field-delete-api.md`

## Stage #1: API 合同与仓储能力

### Task #1: 新增删除请求和错误响应合同

**Status:** Finished

**Files:** Modify `src/dto/taxonomy.rs`, `src/error.rs`, `src/api_doc.rs`

功能 / 实现说明 / 预期验证结果：新增 `DeleteFieldRequest` 和删除成功响应 DTO，字段为 body 中的 `workspace_id` 与 `field_id`，派生 `Deserialize`、`Validate` 和 `ToSchema`。确认现有错误类型是否已经能表达 `409 Conflict`；如果没有，新增明确的 conflict 错误变体并保持 JSON 错误响应结构一致。预期验证结果是 DTO 能被 OpenAPI 注册，`cargo check` 能通过新增类型引用。

### Task #2: 新增 taxonomy repository 删除方法

**Status:** Finished

**Files:** Modify `src/repositories/taxonomy.rs`, `src/repositories/notes/tests.rs` or dedicated repository tests

功能 / 实现说明 / 预期验证结果：新增 `delete_unused_field` 方法，在事务内按 `workspace_id` 和 `field_id` 查找 field，统计同 workspace 下 `deleted_at IS NULL AND archived_at IS NULL` 的关联 notes，数量为 0 时删除 field，数量大于 0 时返回 conflict 结果。删除成功时记录 field delete sync change；为避开 SQLite 复合外键删除 field 时置空 `notes.workspace_id` 的约束问题，删除前只清理同 workspace 下已删除或已归档 notes 的 `field_id`。预期验证结果是 repository 测试覆盖成功删除、可见 note 阻止删除、归档 note 不阻止删除、删除 note 不阻止删除、field 不存在返回 not found。

## Stage #2: HTTP 路由、handler 和 OpenAPI

### Task #1: 实现 `POST /fields/delete` handler 和路由

**Status:** Finished

**Files:** Modify `src/handlers/taxonomy.rs`, `src/routes/taxonomy.rs`

功能 / 实现说明 / 预期验证结果：新增 `delete_field` handler，使用 JSON body 解析 `DeleteFieldRequest`，先校验 body 参数，再通过 `WorkspacesRepository::ensure_active` 校验 active workspace，最后调用 `TaxonomyRepository::delete_unused_field`。路由注册为 `/fields/delete` 的 POST。预期验证结果是无效 JSON 返回 `400`，缺字段或空字段返回 `422`，无效 workspace 和不存在 field 返回 `404`，有关联可见 note 返回 `409`，成功删除返回 `200`。

### Task #2: 维护 OpenAPI 合同

**Status:** Finished

**Files:** Modify `src/handlers/taxonomy.rs`, `src/api_doc.rs`, `tests/openapi_routes.rs`

功能 / 实现说明 / 预期验证结果：为 handler 添加 `#[utoipa::path]`，声明 `post`、`path = "/fields/delete"`、request body、taxonomy tag、`200`、`400`、`404`、`409`、`422` 和 `500` 响应，并在 `ApiDoc` 注册新增 path 和 schema。预期验证结果是 `GET /api-docs/openapi.json` 包含 `/fields/delete` 的 `post` 方法、请求体 schema 和 `409` 响应。

## Stage #3: 路由测试和回归验证

### Task #1: 新增 field 删除路由测试

**Status:** Finished

**Files:** Modify `tests/notes_taxonomy_routes.rs`, `tests/support/notes.rs`

功能 / 实现说明 / 预期验证结果：新增 HTTP 路由测试覆盖删除成功、可见 note 阻止删除、归档 note 不阻止删除、删除 note 不阻止删除、field 不存在返回 `404`、workspace 不存在或非 active 返回 `404`、body 缺字段返回 `422`。测试 fixture 使用 active workspace 和真实 field id，不通过 field name 删除。预期验证结果是 `cargo test --test notes_taxonomy_routes` 通过。

### Task #2: 完整验证、计划回写和提交

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r036-field-delete-api.md`, `docs/PROGRESS.md`

功能 / 实现说明 / 预期验证结果：实现完成后运行 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`，并验证 OpenAPI JSON 可返回 `200 OK` 且包含 `/fields/delete`。验证通过后更新执行计划任务状态和 `docs/PROGRESS.md`，按项目规则 stage、commit 并尝试推送。预期结果是完整回归通过，工作区只包含本需求相关改动。

验证记录：2026-06-27 已通过 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test --test notes_taxonomy_routes delete_field -- --nocapture`、`cargo test --test openapi_routes openapi_json_lists_runtime_api_paths` 和 `cargo test`。
