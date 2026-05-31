# r026 v0.4.0 代码适配范围判断

## 关联设计文档

设计文档：`docs/design-docs/r026-schema-v040-adaptation-scope.md`
需求澄清文档：`docs/request-clarify/r026-schema-v040-adaptation-scope.md`

## Stage #1: 结构化 Tag 模型和查询

### 任务 #1: 扩展 TagRecord 为 v0.4.0 结构

**Status:** Finished

**Files:**
- Modify: `src/models/tag.rs`
- Verify: `tests/openapi_routes.rs`

功能：让 `TagRecord` 表达 `id/name/parent_tag_id/path/depth/created_at`。

实现说明：保留 `Serialize`、`FromRow`、`ToSchema`；为新增成员变量补充文档注释。`name` 表示当前节点名，不再表示完整路径；`path` 表示完整路径。

预期验证结果：项目编译通过，OpenAPI components 中 `TagRecord` 暴露 `parent_tag_id`、`path`、`depth`。

### 任务 #2: 更新 taxonomy repository 查询和创建返回

**Status:** Finished

**Files:**
- Modify: `src/repositories/taxonomy.rs`
- Verify: `src/repositories/notes/tests.rs`

功能：`get_or_create_tag_in_transaction` 和 `list_tags` 返回结构化 tag 字段。

实现说明：所有 tag SELECT 改为 `id, name, parent_tag_id, path, depth, created_at`；逐级创建逻辑保留，创建后返回当前节点 `name` 和完整 `path`。`list_tags` 继续按 `path ASC` 排序。

预期验证结果：创建 `books/python` 后，根节点 `name=books, path=books, depth=0`，叶子节点 `name=python, path=books/python, depth=1`。

## Stage #2: Notes API 和随机标签语义适配

### 任务 #3: 更新 note tags 查询和 metadata path 生成

**Status:** Finished

**Files:**
- Modify: `src/repositories/notes/tags.rs`
- Modify: `src/repositories/notes/core.rs`
- Verify: `src/repositories/notes/tests.rs`
- Verify: `tests/notes_taxonomy_routes.rs`
- Verify: `tests/notes_crud_routes.rs`

功能：note tags API 返回结构化 `TagRecord`，note metadata 继续返回完整 path 字符串。

实现说明：`list_note_tags_by_id` 查询结构化字段并按 `tags.path ASC` 排序；创建和更新 note 时，`resolved_tags` 使用 `tag.path` 组装 metadata；删除/替换仍按完整 path 查找目标 tag。

预期验证结果：`GET /notes/{note_ref}/tags` 返回 `name=python`、`path=books/python`；note create/update response 的 `metadata.tags` 返回 `books/python`。

### 任务 #4: 更新 random tags 查询候选和响应

**Status:** Finished

**Files:**
- Modify: `src/repositories/notes/core.rs`
- Verify: `src/repositories/notes/tests.rs`
- Verify: `tests/notes_query_routes.rs`

功能：`/random/tags` 从所有标签节点抽样，并返回结构化 tag 对象。

实现说明：`random_tags` 查询所有 tags 节点，不 join `note_tags`，不限制叶子节点。`list_visible_notes_by_tag` 仍只返回与该 tag 节点直接关联的可见 notes，父节点无直接关联时可返回空 notes。

预期验证结果：创建 `books/python` 后，随机候选能包含 `books` 和 `python` 两个节点；响应中的 tag 对象包含层级字段。

执行记录：已完成 Stage #1 和 Stage #2。验证通过：`cargo fmt --check`、`cargo check`、`cargo test taxonomy_creates_hierarchical_tag_nodes`、`cargo test repositories::notes`。

## Stage #3: OpenAPI 和 sync 基础校验

### 任务 #5: 更新 DTO 注释和 OpenAPI 回归

**Status:** Designed

**Files:**
- Modify: `src/dto/notes.rs`
- Modify: `src/dto/taxonomy.rs`
- Modify: `tests/openapi_routes.rs`

功能：让请求字段说明和 OpenAPI schema 对齐 v0.4.0 标签语义。

实现说明：更新 create/update note 请求中 `tags` 字段注释，明确数组元素是完整 tag path；OpenAPI 测试断言 `TagRecord` schema 包含 `parent_tag_id`、`path`、`depth`。

预期验证结果：`cargo test openapi_json_lists_runtime_api_paths` 通过，并确认 schema 暴露结构化 tag 字段。

### 任务 #6: 补充 sync tag payload 基础校验测试

**Status:** Designed

**Files:**
- Modify: `src/repositories/sync/payload.rs`
- Modify: `src/repositories/sync/tests.rs`

功能：验证远端 tag insert payload 的 v0.4.0 字段解析和写入。

实现说明：保留 `TagPayload` 对 `name/parent_tag_id/path/depth/created_at` 的解析；补充完整 payload apply 成功测试。缺少 `path` 或 `depth` 的旧 payload 是否继续兼容，应按设计文档选择：r026 只保留基础字段校验，不做乱序补偿。

预期验证结果：完整 tag payload 能应用为结构化 tag；不可应用 payload 进入现有 conflict 路径。

## Stage #4: 整体验证和计划回写

### 任务 #7: 运行全量验证并更新执行记录

**Status:** Designed

**Files:**
- Modify: `docs/exec-plans/active/r026-schema-v040-adaptation-scope.md`
- Modify: `docs/PROGRESS.md`
- Verify: `cargo fmt --check`
- Verify: `cargo check`
- Verify: `cargo test`
- Verify: `cargo clippy -- -D warnings`

功能：确认 r026 全部适配点通过自动化验证，并记录执行进度。

实现说明：完成每个 Stage 后按项目规则进行原子提交；最终更新执行计划状态和 `docs/PROGRESS.md`，不归档计划，等待用户验收。

预期验证结果：格式、编译、全量测试和 clippy 全部通过；工作区只包含 r026 相关变更。
