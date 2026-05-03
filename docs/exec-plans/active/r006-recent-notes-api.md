# r006 Recent Notes API 开发计划

日期：2026-05-03

关联需求澄清：`docs/request-clarify/r006-recent-notes-api.md`
设计文档：`docs/design-docs/r006-recent-notes-api.md`

## Related Design Doc

`docs/design-docs/r006-recent-notes-api.md`

## Stage #1: Recent Notes API 落地

### Task #1: 定义 recent notes DTO 和查询链路

**Status:** Finished

**Files:** Modify `src/dto/notes.rs`, `src/repositories/notes.rs`, `src/services/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `RecentNotesRequest`、`NotesRepository::list_recent_notes`、`NotesService::recent_notes` |
| Implementation Notes | `RecentNotesRequest.limit` 可选，校验范围 1 到 100；repository 查询使用 `deleted_at IS NULL` 和 `archived_at IS NULL`，按 `updated_at DESC` 限制返回数量 |
| Expected Verification Result | service/repository 能返回未删除、未归档的最近 notes，默认数量为 50 |

### Task #2: 新增 handler、route 和 OpenAPI 注册

**Status:** Finished

**Files:** Modify `src/handlers/notes.rs`, `src/routes/notes.rs`, `src/api_doc.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `recent_notes` handler、`POST /notes/recent` route、`ApiDoc` path/schema 注册 |
| Implementation Notes | handler 解析 JSON body，映射 invalid JSON 和 validation error；响应复用 `ListNotesResponse`；OpenAPI 标注声明 request body、response 和错误响应 |
| Expected Verification Result | `POST /notes/recent` 可路由，OpenAPI JSON 包含 `/notes/recent` |

## Stage #2: 验证和计划回写

### Task #1: 增加 recent notes 自动化测试

**Status:** Finished

**Files:** Modify `tests/` or existing inline tests

| 项目 | 内容 |
| --- | --- |
| Function | API integration tests |
| Implementation Notes | 覆盖默认 limit、自定义 limit、更新时间倒序、归档过滤、软删除过滤、limit validation 和 OpenAPI path 暴露 |
| Expected Verification Result | `cargo test` 能稳定验证 recent notes API 行为 |

### Task #2: 运行完整验证并回写执行记录

**Status:** Finished

**Files:** Verify repository; Modify `docs/exec-plans/active/r006-recent-notes-api.md`

| 项目 | 内容 |
| --- | --- |
| Function | build/regression verification、progress record |
| Implementation Notes | 运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy`；记录验证结果和任务完成状态 |
| Expected Verification Result | 四项验证通过，计划状态与实际代码一致 |

## 执行记录

- 2026-05-03：完成需求澄清，确认 `POST /notes/recent` 使用 body limit，默认 50，范围 1 到 100，按 `updated_at DESC` 返回未删除、未归档笔记。
- 2026-05-03：完成设计文档和开发计划，等待进入编码阶段。
- 2026-05-03：完成 Stage #1，新增 `RecentNotesRequest`、`NotesRepository::list_recent_notes`、`NotesService::recent_notes`、`recent_notes` handler、`POST /notes/recent` 路由和 OpenAPI 注册。
- 2026-05-03：完成 Stage #2，新增 recent notes repository/API 测试，覆盖排序、limit、软删除过滤、归档过滤、validation error 和 OpenAPI path 暴露。
- 2026-05-03：最终验证通过：`cargo fmt --check`、`cargo check`、`cargo test`（20 passed）、`cargo clippy`。

## 约束

- 不修改现有 `GET /notes` 行为。
- 不新增分页、鉴权、前端页面或数据库 schema。
- 新增或修改 HTTP handler 时必须同步维护 OpenAPI 标注和 `ApiDoc` 注册。
- 每个 Stage 完成后，如果修改了代码，需要执行一次原子提交。
- 提交信息必须满足 `^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert): .{10,}$`。
