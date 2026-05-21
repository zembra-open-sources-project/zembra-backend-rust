# r018 Daily Note Counts API 开发计划

日期：2026-05-21

## 关联需求澄清

`docs/request-clarify/r018-daily-note-counts-api.md`

## 关联设计文档

`docs/design-docs/r018-daily-note-counts-api.md`

## 阶段 #1: 文档与项目纪律

### 任务 #1: 固化可见笔记默认口径

**Status:** Finished

**Files:** Modify `AGENTS.md`

- 功能：将 notes 相关接口默认只处理未删除、未归档笔记写入项目纪律。
- 实现说明：在后端额外约束中增加 `deleted_at IS NULL` 与 `archived_at IS NULL` 默认过滤要求，禁止后续作为澄清点重复询问。
- 预期验证结果：`AGENTS.md` 包含该纪律，且后续实现按该口径执行。

### 任务 #2: 建立 r018 需求、设计与计划文档

**Status:** Finished

**Files:** Create `docs/request-clarify/r018-daily-note-counts-api.md`, `docs/design-docs/r018-daily-note-counts-api.md`, `docs/exec-plans/active/r018-daily-note-counts-api.md`

- 功能：记录本轮已确认范围、接口合同、实现落点和验证标准。
- 实现说明：沿用项目 rNNN 文档体系，保持同 basename。
- 预期验证结果：三个文档落点正确，且设计引用需求澄清、计划引用二者。

## 阶段 #2: API 实现

### 任务 #1: 增加 DTO、service 与 repository 统计能力

**Status:** Finished

**Files:** Modify `src/dto/notes.rs`, `src/services/notes.rs`, `src/repositories/notes.rs`

- 功能：提供 30 天每日笔记数统计的数据结构和业务方法。
- 实现说明：repository 查询 `DEFAULT_WORKSPACE_ID` 下 `created_at >= start_timestamp` 且未删除未归档笔记，按 `date(created_at, 'unixepoch', 'localtime')` 聚合；service 生成本地日期 30 天序列并补 0。
- 预期验证结果：service 返回固定 30 条，日期升序，无数据日期为 0。

### 任务 #2: 注册 HTTP route 与 OpenAPI

**Status:** Finished

**Files:** Modify `src/handlers/notes.rs`, `src/routes/notes.rs`, `src/api_doc.rs`

- 功能：暴露 `GET /notes/stats/daily-counts`。
- 实现说明：handler 调用 `NotesService::daily_note_counts()`，OpenAPI 标注 200 和 500 响应，并在 `ApiDoc` 注册 path 和 schema。
- 预期验证结果：HTTP 调用返回 `DailyNoteCountsResponse`，OpenAPI JSON 包含该 path。

### 任务 #3: 补充自动化测试

**Status:** Finished

**Files:** Modify `src/app.rs`

- 功能：覆盖统计窗口、可见性过滤、空日期补 0、OpenAPI 注册。
- 实现说明：复用现有 test helper 创建笔记，必要时直接设置 `created_at` 以构造确定日期。
- 预期验证结果：新增测试通过，现有 notes 路由测试不回归。

## 阶段 #3: 验证与提交

### 任务 #1: 格式、构建与回归验证

**Status:** Finished

**Files:** Verify full workspace

- 功能：确认新增接口可编译、测试通过、OpenAPI 合同可用。
- 实现说明：运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy`；如服务可启动，再验证 `/api-docs/openapi.json` 包含 `/notes/stats/daily-counts`。
- 预期验证结果：所有命令通过；计划状态回写为 Finished；完成阶段后提交一次 Conventional Commit。
- 完成记录：已通过 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`。
