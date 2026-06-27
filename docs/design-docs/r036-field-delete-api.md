# r036 field 删除接口设计文档

日期：2026-06-27

需求澄清文档：`docs/request-clarify/r036-field-delete-api.md`

## 核心功能（WHAT）

新增 `POST /fields/delete` 接口，使用 JSON body 接收 `workspace_id` 和 `field_id`。后端在指定 active workspace 内查找 field，并统计同一 workspace 下该 field 关联的可见 note 数量。可见 note 数量为 0 时删除 field；可见 note 数量大于 0 时保留 field 并返回 `409 Conflict`。

### 需求背景（WHY）

当前 field 会在 note 创建或更新时被自动创建，并通过 `GET /fields` 查询，但没有删除入口。长期使用后，客户端需要清理已经不被可见 note 使用的 field，避免 field 列表持续积累无效项。field 删除必须保护仍在使用的 field，避免客户端误删后造成现有可见 note 的分类信息不可解释。

### 需求目标（GOAL）

客户端可以通过 `POST /fields/delete` 删除空 field。接口只删除 body 指定 workspace 下的目标 field，删除前以可见 note 数量作为保护条件。成功路径删除 field；冲突路径返回 `409` 且不改数据库；不存在或无效 workspace 路径返回 `404`。

### 范围边界

纳入范围：新增 taxonomy request/response DTO、新增 taxonomy handler、新增 taxonomy route、新增 repository 删除方法、OpenAPI 注册、路由测试、OpenAPI 测试。接口路径为 `POST /fields/delete`，请求 body 为 `{ "workspace_id": "...", "field_id": "..." }`，有关联可见 note 时返回 `409 Conflict`，field 不存在时返回 `404`。

不纳入范围：不支持 `DELETE /fields`，不支持 path 或 query 参数删除 field，不支持 field name 删除，不支持批量删除，不删除 tag，不新增 schema 或 migration，不移动可见 note 到其他 field，不实现后台空 field 清理任务。

## 实现流程（HOW）

DTO 层在 `src/dto/taxonomy.rs` 新增 `DeleteFieldRequest` 和删除成功响应类型。`DeleteFieldRequest` 包含 `workspace_id: String` 和 `field_id: String`，需要派生 `Deserialize`、`Validate` 和 `ToSchema`；响应类型可以包含 `deleted: bool` 与 `field_id: String`，用于让客户端明确本次删除结果。

handler 层在 `src/handlers/taxonomy.rs` 新增 `delete_field`。handler 使用 JSON body 解析请求，解析失败返回既有 invalid JSON 错误，参数校验失败返回既有 validation 错误。handler 先通过 `WorkspacesRepository::ensure_active` 校验 body 中的 `workspace_id`，校验失败沿用 workspace 不存在的 `404` 口径；然后调用 taxonomy repository 删除方法。repository 返回 field 不存在时映射为 `404`，返回有关联可见 note 时映射为 `409 Conflict`。

repository 层在 `src/repositories/taxonomy.rs` 新增面向 field 删除的结果类型和方法，推荐方法签名为 `delete_unused_field(&self, workspace_id: &str, field_id: &str) -> Result<DeleteFieldOutcome, TaxonomyDeleteError>`。方法在事务内完成 field 存在性检查、可见 note 数量统计和 field 删除，避免检查后到删除前出现不一致窗口。field 存在性检查必须限定 `workspace_id = ? AND id = ?`；note 数量统计必须限定 `workspace_id = ? AND field_id = ? AND deleted_at IS NULL AND archived_at IS NULL`。

删除操作以 `fields` 表中的目标记录为主。由于当前 SQLite 复合外键在删除 field 时会触发 `ON DELETE SET NULL`，并可能尝试置空 `notes.workspace_id`，实现需要在删除 field 前手动将同 workspace 下已删除或已归档 notes 的 `field_id` 置空，再删除 field。可见 notes 仍然通过 `409 Conflict` 阻止删除，不允许被自动改写。删除成功时需要写入 `entity_type = "field"`、`operation = "delete"` 的同步变更，因为现有 field 创建会记录 sync change，删除也应进入同步 outbox 以保持远端一致。

路由层在 `src/routes/taxonomy.rs` 为 `/fields/delete` 注册 POST handler。OpenAPI 需要在 `src/api_doc.rs` 注册新增 handler 和 DTO schema，并在 handler 的 `#[utoipa::path]` 中声明 `post`、`path = "/fields/delete"`、request body、`200`、`400`、`404`、`409`、`422` 和 `500` 响应。

错误映射保持直接。无效 JSON 返回 `400`；字段缺失或空字符串返回 `422`；workspace 缺失、非法、不存在、归档或删除返回 `404`；field 在指定 workspace 下不存在返回 `404`；field 下存在至少 1 条可见 note 返回 `409`；数据库错误返回 `500`。

## 测试用例

| 场景 | 预期 |
| --- | --- |
| `POST /fields/delete` body 指向 active workspace 下无可见 note 的 field | 返回 `200`，响应标记删除成功，数据库中目标 field 不存在 |
| 目标 field 下存在未删除、未归档 note | 返回 `409 Conflict`，数据库中目标 field 仍存在 |
| 目标 field 下只有已归档 note | 返回 `200`，目标 field 被删除 |
| 目标 field 下只有已删除 note | 返回 `200`，目标 field 被删除 |
| body 中 workspace 不存在、已归档或已删除 | 返回 `404` |
| body 中 field 不存在 | 返回 `404` |
| body 缺少 `workspace_id` 或 `field_id` | 返回 `422` |
| OpenAPI JSON | 包含 `/fields/delete` 的 `post` path、request body schema 和 `409` 响应 |
| 完整回归 | `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` 通过 |
