# r017 PATCH Note Taxonomy API 开发计划

日期：2026-05-21

关联需求澄清：`docs/request-clarify/r017-patch-note-taxonomy-api.md`
设计文档：`docs/design-docs/r017-patch-note-taxonomy-api.md`

## Related Design Doc

`docs/design-docs/r017-patch-note-taxonomy-api.md`

## Stage #1: PATCH 请求模型和服务归一化

### Task #1: 扩展 UpdateNoteRequest 表达 field/tags

**Status:** Finished

**Files:** Modify `src/dto/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `UpdateNoteRequest` |
| Implementation Notes | 保留 `content` 必填和 `device_id` 可选；新增 `field: Option<Option<String>>` 用于区分 absent/null/string；新增 `tags: Option<Vec<String>>` 表达 tags absent 或整体替换；保持 `ToSchema` 派生，必要时用 schema 注释说明 null 语义 |
| Expected Verification Result | DTO 可反序列化三态 field 和可选 tags；OpenAPI schema 能暴露新增字段 |

### Task #2: 定义服务层归一化输入

**Status:** Finished

**Files:** Modify `src/services/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `NotesService::update_note`、新增内部 update input 结构或 helper |
| Implementation Notes | `content` 继续使用 `normalize_required_text`；field absent 映射为不修改，null 映射为默认 `inbox`，字符串使用 `normalize_required_text`；tags 使用现有 `normalize_tags`，只有请求传入 tags 时才替换 |
| Expected Verification Result | service 能把请求转换为 repository 可执行输入，空白 field 返回 validation error，content 必填行为不变 |

## Stage #2: Repository 更新事务扩展

### Task #1: 扩展 note 更新事务支持 field

**Status:** Finished

**Files:** Modify `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `NotesRepository::update_note` |
| Implementation Notes | 以统一更新方法承载 content、field、tags；field absent 保留旧 `field_id`，field string/null 通过 `get_or_create_field_in_transaction` 得到目标 field 并更新 `notes.field_id`；field insert sync change 复用现有 helper；不保留旧 `update_note_content` 兼容封装 |
| Expected Verification Result | content 更新仍写 revision；field 不传时不变；field 字符串或 null 时 note.field_id 正确更新 |

### Task #2: 实现 tags 整体替换

**Status:** Finished

**Files:** Modify `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | note tags replacement helper、`NotesRepository::update_note` |
| Implementation Notes | 当 tags 为 `None` 时不修改关联；为 `Some` 时查询或创建目标 tags，读取当前 note_tags，计算需要 attach 和 detach 的 tag_id；新增关联使用 `INSERT OR IGNORE` 并记录 `note_tag attach`，删除关联记录 `note_tag detach`；不删除 tag 实体 |
| Expected Verification Result | tags 数组会整体替换；空数组清空关联；未传 tags 保持原关联；sync changes 覆盖 attach/detach |

## Stage #3: Handler、OpenAPI 和测试覆盖

### Task #1: 更新 API 行为测试

**Status:** Finished

**Files:** Modify `src/app.rs`, `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | repository/API tests for PATCH note taxonomy |
| Implementation Notes | 覆盖只传 content、field 字符串、field null 默认 inbox、field 未传保持不变、tags 替换、tags 清空、tags 未传保持不变、空白 field validation、OpenAPI schema 暴露 field/tags |
| Expected Verification Result | `cargo test` 能稳定验证 PATCH content/field/tags 组合行为 |

### Task #2: 核对 OpenAPI 标注和 ApiDoc 注册

**Status:** Finished

**Files:** Verify `src/handlers/notes.rs`, `src/api_doc.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `update_note` OpenAPI path、`ApiDoc` schemas |
| Implementation Notes | `PATCH /notes/{note_ref}` 已注册 handler 和 `UpdateNoteRequest` schema；实现后需确认 schema 包含 `field` 和 `tags`，错误响应仍覆盖 invalid JSON、validation、not found、conflict 和 database error |
| Expected Verification Result | `/api-docs/openapi.json` 包含 `UpdateNoteRequest.field` 和 `UpdateNoteRequest.tags` |

## Stage #4: 完整验证和执行记录

### Task #1: 运行完整验证

**Status:** Finished

**Files:** Verify repository

| 项目 | 内容 |
| --- | --- |
| Function | build/regression verification |
| Implementation Notes | 运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy`；若 API 合同改动完成，启动服务并验证 `/api-docs/openapi.json` 返回 200 且包含新增 schema 字段 |
| Expected Verification Result | 格式、类型、测试、clippy 和 OpenAPI 合同验证通过 |

### Task #2: 回写计划执行记录

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r017-patch-note-taxonomy-api.md`

| 项目 | 内容 |
| --- | --- |
| Function | progress record |
| Implementation Notes | 开发过程中按任务更新状态；完成每个 Stage 后记录关键实现和验证结果；未经用户验收不移动到 completed |
| Expected Verification Result | 执行计划状态与代码、测试结果一致 |

## 执行记录

- 2026-05-21：完成需求澄清，确认 `PATCH /notes/{note_ref}` 扩展支持 tags 整体替换和 field 可选更新；content 严格必选，field 未传不修改，field 为 null 时设置默认 `inbox`，返回体保持 `NoteRecord`。
- 2026-05-21：完成设计文档和开发计划，等待进入编码阶段。
- 2026-05-21：完成 Stage #1，扩展 `UpdateNoteRequest` 支持 `field` 三态和可选 `tags`，服务层将 `field: null` 归一化为默认 `inbox`，继续保持 content 严格必选。
- 2026-05-21：完成 Stage #2，新增统一 `NotesRepository::update_note` 更新事务，正向承载 content、field 和 tags 整体替换，不保留旧 `update_note_content` 兼容封装。
- 2026-05-21：完成 Stage #3，补充 repository/API 测试，覆盖 field 未传、field null、field 字符串、tags 替换、tags 清空、空白 field validation 和 OpenAPI schema 暴露。
- 2026-05-21：完成 Stage #4，验证通过：`cargo fmt --check`、`cargo check`、`cargo test`（89 passed）、`cargo clippy`；运行服务后确认 `/api-docs/openapi.json` 返回 200 且 `UpdateNoteRequest` 包含 `field` 和 `tags`。

## 约束

- 不新增新的编辑 path，不改 `PATCH /notes/{note_ref}` 返回 body。
- `content` 必须保持严格必选，继续写入 revision。
- `field: null` 必须设置为默认 `inbox`，禁止清空 field。
- `tags` 必须使用整体替换语义。
- 不删除不再关联的 tag/field 实体。
- 新增或修改 HTTP handler/DTO 时必须同步维护 OpenAPI schema 和相关测试。
- 每个 Stage 完成后，如果修改了代码，需要执行一次原子提交。
- 提交信息必须满足 `^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert): .{10,}$`。
