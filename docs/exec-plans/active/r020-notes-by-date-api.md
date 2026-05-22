# r020 Notes By Date API 开发计划

日期：2026-05-22

## 关联文档

- 需求澄清：`docs/request-clarify/r020-notes-by-date-api.md`
- 设计文档：`docs/design-docs/r020-notes-by-date-api.md`

## 阶段 #1: API 合同与查询链路

### 任务 #1: 新增 DTO、Service 和 Repository 查询

**Status:** Finished

**Files:** Modify `src/dto/notes.rs`, `src/services/notes.rs`, `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `NotesByDateQuery`、`NotesByDateResponse`、`NotesService::notes_by_date`、`NotesRepository::list_visible_notes_created_between` |
| Implementation Notes | `date` query 必填；service 使用 `NaiveDate::parse_from_str` 校验 `YYYY-MM-DD`，计算本地日开始和下一日开始 timestamp；repository 查询默认 workspace 下未删除、未归档 notes，并按 `created_at DESC, id DESC` 排序 |
| Expected Verification Result | service 能返回指定本地日期内创建的可见 notes，非法日期返回 validation error |

### 任务 #2: 新增 Handler、Route 和 OpenAPI 注册

**Status:** Finished

**Files:** Modify `src/handlers/notes.rs`, `src/routes/notes.rs`, `src/api_doc.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `notes_by_date` handler、`GET /notes/by-date` route、`ApiDoc` path/schema 注册 |
| Implementation Notes | handler 使用 `Query<NotesByDateQuery>` 接收参数并调用 `Validate`；route 必须注册在 `/notes/{note_ref}` 前；OpenAPI 标注声明 params、200、422、500 |
| Expected Verification Result | `GET /notes/by-date?date=YYYY-MM-DD` 可路由，OpenAPI JSON 包含 `/notes/by-date` |

## 阶段 #2: 测试与验证

### 任务 #3: 补齐 API 行为测试

**Status:** Finished

**Files:** Modify `src/app.rs`

| 项目 | 内容 |
| --- | --- |
| Function | 新增 notes-by-date API route 测试 |
| Implementation Notes | 复用测试 helper 创建 notes 并直接写 `created_at`；覆盖目标日期返回、空结果、删除归档过滤、非法日期、缺少日期和 OpenAPI path |
| Expected Verification Result | `cargo test notes_by_date` 和完整 `cargo test` 均通过 |

### 任务 #4: 整体验证与计划回写

**Status:** Finished

**Files:** Verify repository; Modify `docs/exec-plans/active/r020-notes-by-date-api.md`

| 项目 | 内容 |
| --- | --- |
| Function | 格式、编译、测试、lint 验证和计划状态记录 |
| Implementation Notes | 运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`；通过后把本计划任务状态更新为 `Finished` 并记录验证结果 |
| Expected Verification Result | 所有验证命令通过，计划文档记录最终结果 |

## 进度记录

- 2026-05-22：完成需求澄清，确认新增 `GET /notes/by-date?date=YYYY-MM-DD`，按服务器本地日期返回当天创建的未删除、未归档笔记，排序为 `created_at DESC, id DESC`。
- 2026-05-22：完成设计文档和开发计划，等待进入编码实现。
- 2026-05-22：完成 Stage #1，新增 notes-by-date DTO、repository 查询、service 日期解析、handler、route 和 OpenAPI 注册。
- 2026-05-22：完成 Stage #2，新增 API 测试覆盖目标日期查询、空结果、删除归档过滤、非法日期、缺少日期和 OpenAPI path；已通过 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings` 验证。

## 风险与约束

- 日期口径必须和 `GET /notes/stats/daily-counts` 保持一致，避免前端统计日期和明细日期不匹配。
- notes 相关查询必须固定过滤 `deleted_at IS NULL` 和 `archived_at IS NULL`。
- 新路由必须放在动态 `/{note_ref}` 路由之前，避免路径匹配歧义。
