# r013 Random Tags API 开发计划

日期：2026-05-16

关联需求澄清：`docs/request-clarify/r013-random-tags-api.md`
设计文档：`docs/design-docs/r013-random-tags-api.md`

## Related Design Doc

`docs/design-docs/r013-random-tags-api.md`

## Stage #1: Random Tags 查询链路落地

### Task #1: 定义 DTO 和 Service 返回结构

**Status:** Finished

**Files:** Modify `src/dto/notes.rs`, `src/services/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `RandomTagsQuery`、`TaggedNotesResponse`、`TaggedNotesGroup`、`NotesService::random_tagged_notes` |
| Implementation Notes | `RandomTagsQuery.n` 可选，校验范围 1 到 20，默认值在 service 层使用 3；响应顶级字段固定为 `tagged_notes`，每组包含 `TagRecord` 和 `Vec<NoteRecord>` |
| Expected Verification Result | Service 能根据 query 返回按 tag 分组的 tagged notes 响应结构 |

### Task #2: 实现 Repository 随机 tag 和 notes 查询

**Status:** Finished

**Files:** Modify `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `NotesRepository::random_tags`、`NotesRepository::list_visible_notes_by_tag` |
| Implementation Notes | `random_tags` 使用 `ORDER BY RANDOM() LIMIT ?` 从默认 workspace tags 中抽取；`list_visible_notes_by_tag` 通过 `note_tags` 关联 notes，并过滤 `deleted_at IS NULL` 和 `archived_at IS NULL`；组内按 `updated_at DESC, id DESC` 排序 |
| Expected Verification Result | Repository 能在 tag 数不足时返回现有数量，并只返回可展示 notes |

### Task #3: 新增 handler、route 和 OpenAPI 注册

**Status:** Finished

**Files:** Modify `src/handlers/notes.rs`, `src/routes/notes.rs`, `src/api_doc.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `random_tagged_notes` handler、`GET /random/tags` route、`ApiDoc` path/schema 注册 |
| Implementation Notes | handler 使用 `Query<RandomTagsQuery>` 解析 query 参数并执行 `Validate`；OpenAPI 标注声明 query 参数、成功响应、validation error 和 database error |
| Expected Verification Result | `GET /random/tags?n=3` 可路由，OpenAPI JSON 包含 `/random/tags` |

## Stage #2: 自动化测试和完整验证

### Task #1: 增加 Random Tags 行为测试

**Status:** Finished

**Files:** Modify existing inline tests or `tests/`

| 项目 | 内容 |
| --- | --- |
| Function | repository/API integration tests |
| Implementation Notes | 覆盖默认 `n`、合法 `n`、`n = 0`、`n = 21`、tag 不足、软删除过滤、归档过滤、重复 note 分组和 OpenAPI path 暴露 |
| Expected Verification Result | `cargo test` 能稳定验证 random tags API 行为 |

### Task #2: 运行完整验证并回写执行记录

**Status:** Finished

**Files:** Verify repository; Modify `docs/exec-plans/active/r013-random-tags-api.md`

| 项目 | 内容 |
| --- | --- |
| Function | build/regression verification、progress record |
| Implementation Notes | 运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy`；记录验证结果和任务完成状态 |
| Expected Verification Result | 四项验证通过，计划状态与实际代码一致 |

## Stage #3: Random Tags count 参数扩展

### Task #1: 扩展 DTO、Repository 和 Service 累计 count 支持

**Status:** Finished

**Files:** Modify `src/dto/notes.rs`, `src/repositories/notes.rs`, `src/services/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `RandomTagsQuery.count`、`NotesRepository::list_visible_notes_by_tag`、`NotesService::random_tagged_notes` |
| Implementation Notes | `count` 可选，范围 1 到 100，默认 20；repository 按 tag ID 随机查询可见 notes 并支持 limit；service 按随机 tag 顺序逐组分配剩余额度，所有 notes 累计不超过 `count` |
| Expected Verification Result | `GET /random/tags?n=N&count=CNT` 返回的所有 notes 总数不超过 `CNT` |

### Task #2: 补齐 count 参数测试和验证记录

**Status:** Finished

**Files:** Modify `src/app.rs`, `src/repositories/notes.rs`, `docs/exec-plans/active/r013-random-tags-api.md`

| 项目 | 内容 |
| --- | --- |
| Function | random tags count tests、progress record |
| Implementation Notes | 覆盖默认 `count`、合法 `count`、`count = 0`、`count = 101`、累计 count、软删除/归档过滤和 OpenAPI query 暴露；运行完整验证并回写执行记录 |
| Expected Verification Result | `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy` 全部通过 |

## 执行记录

- 2026-05-16：完成需求澄清，确认 `GET /random/tags?n=N` 使用 query 参数 `n`，范围 1 到 20，按随机 tag 分组返回 `tagged_notes`，notes 只包含未删除、未归档记录。
- 2026-05-16：完成设计文档和开发计划，等待进入编码阶段。
- 2026-05-16：完成 Stage #1，新增 `RandomTagsQuery`、`TaggedNotesResponse`、`TaggedNotesGroup`、repository 随机 tag/可见 notes 查询、service 组装、`GET /random/tags` handler、route 和 OpenAPI 注册。
- 2026-05-16：完成 Stage #2，新增 random tags repository/API 测试，覆盖默认 `n`、合法 `n`、`n = 0`、`n = 21`、tag 不足、软删除过滤、归档过滤、重复 note 分组和 OpenAPI path 暴露。
- 2026-05-16：最终验证通过：`cargo fmt --check`、`cargo check`、`cargo test`（63 passed）、`cargo clippy`。
- 2026-05-16：确认 random tags API 增量扩展，新增 `count` query 参数，默认 20，范围 1 到 100，用于限制所有 tag 分组内 notes 的累计数量。
- 2026-05-16：完成 Stage #3，扩展 `RandomTagsQuery.count`、`NotesRepository::list_visible_notes_by_tag` limit 查询和 `NotesService::random_tagged_notes` 累计 count 组装，补齐 API/repository 测试。
- 2026-05-16：Stage #3 验证通过：`cargo fmt --check`、`cargo check`、`cargo test`（76 passed）、`cargo clippy`。

## 约束

- 不修改现有 `/tags`、`/notes`、`/notes/recent` 行为。
- 不新增分页、seed、鉴权、前端页面或数据库 schema。
- 新增 HTTP handler 时必须同步维护 OpenAPI 标注和 `ApiDoc` 注册。
- 每个 Stage 完成后，如果修改了代码，需要执行一次原子提交。
- 提交信息必须满足 `^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert): .{10,}$`。
