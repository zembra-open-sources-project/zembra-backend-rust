# r015 Random Notes API 开发计划

日期：2026-05-16

关联需求澄清：`docs/request-clarify/r015-random-notes-api.md`
设计文档：`docs/design-docs/r015-random-notes-api.md`

## Related Design Doc

`docs/design-docs/r015-random-notes-api.md`

## Stage #1: Random Notes 查询链路落地

### Task #1: 定义 DTO、Service 和 Repository 查询

**Status:** Finished

**Files:** Modify `src/dto/notes.rs`, `src/services/notes.rs`, `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `RandomNotesQuery`、`NotesService::random_notes`、`NotesRepository::list_random_notes` |
| Implementation Notes | `RandomNotesQuery.n` 必填，范围 1 到 50；repository 使用 `ORDER BY RANDOM() LIMIT ?` 查询未删除、未归档 notes；无笔记时返回空 Vec |
| Expected Verification Result | Service 能返回最多 `n` 条随机可见 notes |

### Task #2: 新增 handler、route 和 OpenAPI 注册

**Status:** Finished

**Files:** Modify `src/handlers/notes.rs`, `src/routes/notes.rs`, `src/api_doc.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `random_notes` handler、`GET /random/notes` route、`ApiDoc` path 注册 |
| Implementation Notes | handler 使用 `Query<RandomNotesQuery>` 解析 query 参数并执行 `Validate`；响应复用 `ListNotesResponse`；OpenAPI 标注声明 query 参数、成功响应、validation error 和 database error |
| Expected Verification Result | `GET /random/notes?n=5` 可路由，OpenAPI JSON 包含 `/random/notes` |

## Stage #2: 自动化测试和完整验证

### Task #1: 增加 Random Notes 行为测试

**Status:** Finished

**Files:** Modify existing inline tests

| 项目 | 内容 |
| --- | --- |
| Function | repository/API tests |
| Implementation Notes | 覆盖合法 `n`、`n = 0`、`n = 51`、notes 不足、无笔记、软删除过滤、归档过滤和 OpenAPI path 暴露 |
| Expected Verification Result | `cargo test` 能稳定验证 random notes API 行为 |

### Task #2: 运行完整验证并回写执行记录

**Status:** Finished

**Files:** Verify repository; Modify `docs/exec-plans/active/r015-random-notes-api.md`

| 项目 | 内容 |
| --- | --- |
| Function | build/regression verification、progress record |
| Implementation Notes | 运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy`；记录验证结果和任务完成状态 |
| Expected Verification Result | 四项验证通过，计划状态与实际代码一致 |

## 执行记录

- 2026-05-16：完成需求澄清，确认 `GET /random/notes?n=N` 使用必填 query 参数 `n`，范围 1 到 50，随机返回未删除、未归档 notes，无可用 notes 时返回空数组。
- 2026-05-16：完成设计文档和开发计划，进入编码阶段。
- 2026-05-16：完成 Stage #1，新增 `RandomNotesQuery`、`NotesRepository::list_random_notes`、`NotesService::random_notes`、`GET /random/notes` handler、route 和 OpenAPI 注册。
- 2026-05-16：完成 Stage #2，新增 random notes repository/API 测试，覆盖合法 `n`、`n = 0`、`n = 51`、notes 不足、无笔记、软删除过滤、归档过滤和 OpenAPI path 暴露。
- 2026-05-16：最终验证通过：`cargo fmt --check`、`cargo check`、`cargo test`（82 passed）、`cargo clippy`。

## 约束

- 不修改现有 `/notes`、`/notes/recent`、`/random/tags`、`/random/fields` 行为。
- 不新增分页、seed、鉴权、前端页面或数据库 schema。
- 新增 HTTP handler 时必须同步维护 OpenAPI 标注和 `ApiDoc` 注册。
- 每个 Stage 完成后，如果修改了代码，需要执行一次原子提交。
- 提交信息必须满足 `^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert): .{10,}$`。
