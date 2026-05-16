# r014 Random Fields API 开发计划

日期：2026-05-16

关联需求澄清：`docs/request-clarify/r014-random-fields-api.md`
设计文档：`docs/design-docs/r014-random-fields-api.md`

## Related Design Doc

`docs/design-docs/r014-random-fields-api.md`

## Stage #1: Random Fields 查询链路落地

### Task #1: 定义 DTO 和 Service 返回结构

**Status:** Finished

**Files:** Modify `src/dto/notes.rs`, `src/services/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `RandomFieldsQuery`、`FieldNotesResponse`、`FieldNotesGroup`、`NotesService::random_field_notes` |
| Implementation Notes | `RandomFieldsQuery.n` 可选，范围 1 到 20，默认 3；`count` 可选，范围 1 到 100，默认 20；响应顶级字段固定为 `field_notes` |
| Expected Verification Result | Service 能根据 query 返回按 field 分组且累计 notes 不超过 `count` 的响应结构 |

### Task #2: 实现 Repository 随机 field 和 notes 查询

**Status:** Finished

**Files:** Modify `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `NotesRepository::random_fields`、`NotesRepository::list_visible_notes_by_field` |
| Implementation Notes | `random_fields` 使用 `ORDER BY RANDOM() LIMIT ?` 从默认 workspace fields 中抽取；`list_visible_notes_by_field` 通过 `notes.field_id` 查询并过滤 `deleted_at IS NULL` 和 `archived_at IS NULL`；notes 使用 `ORDER BY RANDOM() LIMIT ?` |
| Expected Verification Result | Repository 能在 field 数不足时返回现有数量，并只返回可展示 notes |

### Task #3: 新增 handler、route 和 OpenAPI 注册

**Status:** Finished

**Files:** Modify `src/handlers/notes.rs`, `src/routes/notes.rs`, `src/api_doc.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `random_field_notes` handler、`GET /random/fields` route、`ApiDoc` path/schema 注册 |
| Implementation Notes | handler 使用 `Query<RandomFieldsQuery>` 解析 query 参数并执行 `Validate`；OpenAPI 标注声明 query 参数、成功响应、validation error 和 database error |
| Expected Verification Result | `GET /random/fields?n=3&count=20` 可路由，OpenAPI JSON 包含 `/random/fields` |

## Stage #2: 自动化测试和完整验证

### Task #1: 增加 Random Fields 行为测试

**Status:** Finished

**Files:** Modify existing inline tests or `tests/`

| 项目 | 内容 |
| --- | --- |
| Function | repository/API integration tests |
| Implementation Notes | 覆盖默认参数、合法参数、`n = 0`、`n = 21`、`count = 0`、`count = 101`、field 不足、累计 count、软删除过滤、归档过滤、空 field 和 OpenAPI path 暴露 |
| Expected Verification Result | `cargo test` 能稳定验证 random fields API 行为 |

### Task #2: 运行完整验证并回写执行记录

**Status:** Finished

**Files:** Verify repository; Modify `docs/exec-plans/active/r014-random-fields-api.md`

| 项目 | 内容 |
| --- | --- |
| Function | build/regression verification、progress record |
| Implementation Notes | 运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy`；记录验证结果和任务完成状态 |
| Expected Verification Result | 四项验证通过，计划状态与实际代码一致 |

## 执行记录

- 2026-05-16：完成需求澄清，确认 `GET /random/fields?n=N&count=CNT` 使用 query 参数 `n` 和 `count`，按随机 field 分组返回 `field_notes`，notes 累计不超过 `count` 且只包含未删除、未归档记录。
- 2026-05-16：完成设计文档和开发计划，进入编码阶段。
- 2026-05-16：完成 Stage #1，新增 `RandomFieldsQuery`、`FieldNotesResponse`、`FieldNotesGroup`、repository 随机 field/可见 notes 查询、service 累计 count 组装、`GET /random/fields` handler、route 和 OpenAPI 注册。
- 2026-05-16：完成 Stage #2，新增 random fields repository/API 测试，覆盖默认参数、合法参数、`n = 0`、`n = 21`、`count = 0`、`count = 101`、field 不足、累计 count、软删除过滤、归档过滤、空 field 和 OpenAPI path 暴露。
- 2026-05-16：最终验证通过：`cargo fmt --check`、`cargo check`、`cargo test`（74 passed）、`cargo clippy`。

## 约束

- 不修改现有 `/fields`、`/tags`、`/notes`、`/notes/recent`、`/random/tags` 行为。
- 不新增分页、seed、鉴权、前端页面或数据库 schema。
- 新增 HTTP handler 时必须同步维护 OpenAPI 标注和 `ApiDoc` 注册。
- 每个 Stage 完成后，如果修改了代码，需要执行一次原子提交。
- 提交信息必须满足 `^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert): .{10,}$`。
