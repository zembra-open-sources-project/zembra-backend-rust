# r004 Server CRUD API 开发计划

日期：2026-05-02

需求来源：`docs/http-client-server-api.md`
设计文档：`docs/design-docs/r004-server-crud-api.md`
关联需求澄清：`docs/request-clarify/r001-backend-tech-stack.md`、`docs/request-clarify/r003-shared-schema-submodule.md`

## Related Design Doc

`docs/design-docs/r004-server-crud-api.md`

## Stage #1: 数据库运行时接入

### Task #1: 创建数据库连接池和 AppState

**Status:** Finished

**Files:** Modify `src/main.rs`, `src/app.rs`, `src/error.rs`; Create `src/repositories/database.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `Database::connect`、`AppState`、`build_router(state)` |
| Implementation Notes | 根据 `Settings.database.sqlite_url()` 创建 `SqlitePool`，将 pool 注入 Axum state，扩展启动错误类型 |
| Expected Verification Result | server 启动时能建立 SQLite 连接；路由测试可用测试 state 构建 router |

### Task #2: 接入 shared schema migration

**Status:** Finished

**Files:** Modify `src/repositories/database.rs`; Verify `vendor/zembra-schema/migrations/`, `src/migrations/`

| 项目 | 内容 |
| --- | --- |
| Function | `Database::migrate` |
| Implementation Notes | 优先使用 `vendor/zembra-schema/migrations/` 作为 migration 来源；如 SQLx 编译期路径受限，建立明确的 `src/migrations` 桥接方案并记录原因 |
| Expected Verification Result | 临时 SQLite 数据库执行 migration 后包含 CRUD 所需表 |

### Task #3: 扩展 health 数据库状态

**Status:** Finished

**Files:** Modify `src/handlers/health.rs`, `src/routes/health.rs`, `src/dto/mod.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `health` |
| Implementation Notes | `/health` 返回 `status`、`service`、`database_initialized`，通过 state 判断数据库可用状态 |
| Expected Verification Result | `GET /health` 返回 `200 OK` 和完整 JSON 结构 |

## Stage #2: 数据模型、DTO 和错误合同

### Task #1: 定义数据库模型

**Status:** Designed

**Files:** Create/Modify `src/models/mod.rs`, `src/models/note.rs`, `src/models/field.rs`, `src/models/tag.rs`, `src/models/revision.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `NoteRecord`、`FieldRecord`、`TagRecord`、`NoteRevisionRecord` |
| Implementation Notes | 字段严格对齐 shared schema；时间、ID、nullable 字段按 SQLite 实际类型映射 |
| Expected Verification Result | SQLx query mapping 可编译，模型可序列化为 API 响应 |

### Task #2: 定义 API DTO

**Status:** Designed

**Files:** Create/Modify `src/dto/mod.rs`, `src/dto/notes.rs`, `src/dto/taxonomy.rs`, `src/dto/error.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `CreateNoteRequest`、`BatchCreateNotesRequest`、`NoteResponse`、`ListFieldsResponse`、`ListTagsResponse`、`ErrorResponse` |
| Implementation Notes | 使用 Serde 和 Validator 表达 JSON 合同；role 默认值为 `Human` |
| Expected Verification Result | DTO 序列化字段名与 `docs/http-client-server-api.md` 一致 |

### Task #3: 统一 API 错误映射

**Status:** Designed

**Files:** Modify `src/error.rs`; Create `src/error/api.rs` if needed

| 项目 | 内容 |
| --- | --- |
| Function | `ApiError`、`IntoResponse` |
| Implementation Notes | 映射 validation、not found、ambiguous ref、database error 和 database not initialized |
| Expected Verification Result | handler 测试中错误状态码和 JSON code 与设计文档一致 |

## Stage #3: Repository CRUD

### Task #1: 实现 field/tag 查询或创建

**Status:** Designed

**Files:** Create/Modify `src/repositories/mod.rs`, `src/repositories/taxonomy.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `get_or_create_field`、`get_or_create_tag`、`list_fields`、`list_tags` |
| Implementation Notes | 使用显式 SQL；field/tag 按 name 唯一语义查询；列表按 name 升序 |
| Expected Verification Result | repository 测试覆盖查询、创建、重复创建和排序 |

### Task #2: 实现 note 创建和批量创建事务

**Status:** Designed

**Files:** Create/Modify `src/repositories/notes.rs`, `src/repositories/revisions.rs`, `src/repositories/taxonomy.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `create_note`、`create_notes_batch` |
| Implementation Notes | note、初始 revision、current_revision_id 和 note_tags 在事务内完成；batch 任一失败整体回滚 |
| Expected Verification Result | 测试验证完整写入链路、重复 tag 去重、失败回滚 |

### Task #3: 实现 note 查询、更新、归档和软删除

**Status:** Designed

**Files:** Modify `src/repositories/notes.rs`, `src/repositories/revisions.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `list_notes`、`get_note_by_ref`、`update_note_content`、`archive_note`、`delete_note`、`list_note_revisions` |
| Implementation Notes | note ref 支持完整 ID 或唯一前缀；更新写入 revision；删除使用 `deleted_at` |
| Expected Verification Result | 测试覆盖 ref 解析、冲突、更新 revision、软删除过滤和归档 |

### Task #4: 实现 note tag 关联维护

**Status:** Designed

**Files:** Modify `src/repositories/taxonomy.rs`, `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `list_note_tags`、`add_tag_to_note`、`remove_tag_from_note` |
| Implementation Notes | 添加关联保持幂等；移除只删除关联，不删除 tag 实体 |
| Expected Verification Result | 测试验证重复添加不产生重复关联，删除后 note tags 正确 |

## Stage #4: Service 和 HTTP API

### Task #1: 实现 NotesService

**Status:** Designed

**Files:** Create/Modify `src/services/mod.rs`, `src/services/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `NotesService::create_note`、`create_notes_batch`、`list_notes`、`update_note`、`delete_note` |
| Implementation Notes | 承载 role 校验、content 校验、tag 清洗和 repository 错误转换 |
| Expected Verification Result | service 测试覆盖业务校验、默认 role 和 tag 去重 |

### Task #2: 实现 Notes/Fields/Tags handlers

**Status:** Designed

**Files:** Create/Modify `src/handlers/notes.rs`, `src/handlers/taxonomy.rs`, `src/routes/notes.rs`, `src/routes/taxonomy.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `create_note`、`create_notes_batch`、`list_fields`、`list_tags`、note CRUD handlers |
| Implementation Notes | handler 只处理 Axum extractor、状态码和 DTO，不直接写 SQL |
| Expected Verification Result | API 测试覆盖成功响应、错误响应和状态码 |

### Task #3: 注册路由和 OpenAPI 注解

**Status:** Designed

**Files:** Modify `src/app.rs`, `src/routes/mod.rs`, `src/handlers/mod.rs`, `src/dto/*`

| 项目 | 内容 |
| --- | --- |
| Function | `build_router`、route modules、utoipa schema derive |
| Implementation Notes | 注册 CRUD 路由；保持 `/health` 兼容；补齐 OpenAPI schema 以便后续联调 |
| Expected Verification Result | 所有目标 path 可路由，未知路径仍返回 Axum 默认 404 |

## Stage #5: 集成验证和计划回写

### Task #1: 增加端到端 API 集成测试

**Status:** Designed

**Files:** Create/Modify `tests/` or inline module tests

| 项目 | 内容 |
| --- | --- |
| Function | API integration tests |
| Implementation Notes | 使用临时 SQLite 数据库启动 router，覆盖 create/list/update/delete 和错误响应 |
| Expected Verification Result | `cargo test` 能在干净环境中稳定通过 |

### Task #2: 运行完整验证

**Status:** Designed

**Files:** Verify repository |

| 项目 | 内容 |
| --- | --- |
| Function | build/regression verification |
| Implementation Notes | 运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy` |
| Expected Verification Result | 四项验证全部通过，验证结果记录到本计划执行记录 |

### Task #3: 更新执行记录并按 Stage 提交

**Status:** Designed

**Files:** Modify `docs/exec-plans/active/r004-server-crud-api.md`

| 项目 | 内容 |
| --- | --- |
| Function | progress record |
| Implementation Notes | 每个 Stage 完成后更新任务状态和执行记录；如修改代码，按仓库规则进行一次 conventional commit |
| Expected Verification Result | 计划状态与实际代码一致，提交信息满足仓库 Git 防火墙规则 |

## 执行记录

- 2026-05-02：完成 CRUD 现状判断，确认当前仅有数据库配置和 SQLx 依赖，尚未实现连接池、migration、repository、service 和业务 API。
- 2026-05-02：落地 Server CRUD API 设计文档和开发计划，等待进入编码阶段。
- 2026-05-02：完成 Stage #1，新增 `Database` 连接池封装，启动时执行 v0.2.0 shared schema migration，`AppState` 注入 Axum router，并扩展 `/health` 返回 `service` 和 `database_initialized`。
- 2026-05-02：Stage #1 已通过 `cargo check` 和 `cargo test` 验证。

## 约束

- 不复制维护 shared schema 正文。
- 不实现认证授权。
- 不修改 CLI HTTP client。
- 每个 Stage 完成后，如果修改了代码，需要执行一次原子提交。
- 提交信息必须满足 `^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert): .{10,}$`。
