# r009 Supabase 后台同步接入

日期：2026-05-04

> r027 更新：Supabase/Postgres 远端 schema 归属已迁回 `zembra-schema v0.5.0`。本仓运行时只接入 `vendor/zembra-schema/migrations/005_register_unified_postgres_contract.sql` 的 SQLite 版本登记迁移，不再维护本仓 `supabase/migrations/001_initial_sync_schema.sql`；以下正文保留 r009 当时的历史执行上下文。

需求澄清文档：`docs/request-clarify/r009-supabase-sync.md`

## Related Design Doc

`docs/design-docs/r009-supabase-sync.md`

## Stage #1: 配置与 Supabase Migration

### Task #1: 扩展 sync 配置

**Status:** Finished

**Files:** Modify `src/config.rs`, `.env.example`

**Function:** 让后端能从 `.zembra.env` 读取后台同步配置。

**Implementation Notes:** 新增 `SyncSettings`，包含 `enabled`、`interval_seconds`、`supabase_url`、`service_role_key`。默认 `enabled=false`，默认间隔 60 秒。`enabled=true` 时校验 URL 和 key 非空，并限制最小间隔。

**Expected Verification Result:** 配置单元测试覆盖默认值、正常配置、缺少密钥和过小间隔。

### Task #2: 新增 Supabase Postgres migration

**Status:** Finished

**Files:** Create `supabase/migrations/001_initial_sync_schema.sql`

**Function:** 在本仓库维护第一版 Supabase 远端同步表结构。

**Implementation Notes:** 创建 `workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`sync_changes`。类型按 Postgres 设计：workspace 用 `uuid`，payload 用 `jsonb`，时间戳用 `bigint`。

**Expected Verification Result:** migration 文件可读，包含默认 workspace 插入和关键索引；后续可在 Supabase SQL editor 或 CLI 执行。

## Stage #2: 本地 Sync Change 生成

### Task #3: 新增 sync repository 基础能力

**Status:** Finished

**Files:** Create `src/repositories/sync.rs`, Modify `src/repositories/mod.rs`

**Function:** 封装 `sync_changes`、`sync_state`、`sync_conflicts` 的本地读写。

**Implementation Notes:** 提供写入 change、读取 push/pull 游标、更新游标、幂等检查、记录冲突等方法。所有函数保留文档字符串。

**Expected Verification Result:** repository 单元测试覆盖 change 写入、游标读写和重复 change 幂等判断。

### Task #4: notes 写入生成 change

**Status:** Finished

**Files:** Modify `src/repositories/notes.rs`

**Function:** 本地 note 创建、更新、归档、删除、tag attach/detach 生成同步变更。

**Implementation Notes:** 在现有 SQLite transaction 内同步写入业务表和 `sync_changes`。覆盖 `note`、`note_revision`、`note_tag`。创建 note 时生成 note insert 与 revision insert；更新 note 时生成 revision insert 与 note update。

**Expected Verification Result:** notes repository 测试断言业务写入后存在对应 `sync_changes`。

### Task #5: taxonomy 写入生成 change

**Status:** Finished

**Files:** Modify `src/repositories/taxonomy.rs`

**Function:** field/tag 创建时生成同步变更。

**Implementation Notes:** 仅在实际插入新 field/tag 时生成 change；已存在对象复用时不重复生成 insert change。

**Expected Verification Result:** taxonomy 测试覆盖新建 field/tag 生成 change，复用已有 field/tag 不重复生成。

## Stage #3: Supabase Client 与同步服务

### Task #6: 实现 Supabase REST client

**Status:** Finished

**Files:** Create `src/sync/supabase.rs`, Create `src/sync/mod.rs`, Modify `Cargo.toml`

**Function:** 封装对 Supabase REST/PostgREST 的同步表访问。

**Implementation Notes:** 使用 `reqwest` 发送 HTTPS 请求，设置 `apikey` 和 `Authorization` header。提供 upsert changes、fetch remote changes 方法。错误类型不能包含 service role key。

**Expected Verification Result:** 使用 mock HTTP 测试 header、URL、序列化和错误脱敏。

### Task #7: 实现 sync service

**Status:** Finished

**Files:** Create `src/services/sync.rs`, Modify `src/services/mod.rs`

**Function:** 编排 push、pull、run_once 和 status。

**Implementation Notes:** push 按本地游标读取 change 并 upsert 到 Supabase；pull 按远端游标拉取 change，排除本设备 change，应用到本地。每次成功推进 `sync_state`，失败记录错误。

**Expected Verification Result:** 单元测试覆盖 push 成功推进游标、pull 幂等应用、失败记录错误。

### Task #8: 实现远端 change 应用和冲突处理

**Status:** Finished

**Files:** Modify `src/repositories/sync.rs`, Modify `src/services/sync.rs`

**Function:** 将远端 change 应用到本地业务表，并处理第一版冲突。

**Implementation Notes:** 根据 `entity_type` 和 `operation` 分派到应用逻辑。note revision 冲突保留所有 revision，按最大 `(created_at, device_id, revision_id)` 设置 `current_revision_id`。无法理解的 payload 记录 `schema_incompatible`。

**Expected Verification Result:** 测试覆盖重复 change、并发 revision winner、delete vs update 冲突记录。

## Stage #4: 后台 Worker 与 HTTP API

### Task #9: 接入后台常驻 worker

**Status:** Finished

**Files:** Create `src/sync/worker.rs`, Modify `src/main.rs`, Modify `src/app.rs`

**Function:** 服务启动后按配置频率运行后台同步。

**Implementation Notes:** `sync.enabled=false` 时不启动 worker。`enabled=true` 时创建 sync service 并 `tokio::spawn` 循环执行 `run_once`。每轮失败只记录日志，不终止 HTTP 服务。

**Expected Verification Result:** 测试或本地启动验证 `enabled=false` 服务正常启动；`enabled=true` 缺配置会失败；配置完整时 worker 创建成功。

### Task #10: 新增 sync HTTP API

**Status:** Finished

**Files:** Create `src/handlers/sync.rs`, Create `src/routes/sync.rs`, Create `src/dto/sync.rs`, Modify `src/handlers/mod.rs`, Modify `src/routes/mod.rs`, Modify `src/dto/mod.rs`, Modify `src/app.rs`, Modify `src/api_doc.rs`

**Function:** 暴露同步状态和手动触发 API。

**Implementation Notes:** 新增 `GET /sync/status`、`POST /sync/run`、`POST /sync/push`、`POST /sync/pull`。响应只返回配置摘要、游标和最近错误，不返回 Supabase key。所有 handler 维护 `#[utoipa::path]`。

**Expected Verification Result:** route 测试覆盖 200 响应；OpenAPI JSON 包含四个 sync path。

## Stage #5: 整体验证与记录

### Task #11: 回归验证

**Status:** Finished

**Files:** Verify repository

**Function:** 确认 Supabase 同步接入不破坏既有后端行为。

**Implementation Notes:** 执行格式化、编译、测试、clippy、本地启动、`/health` 和 `/api-docs/openapi.json` 检查。

**Expected Verification Result:** `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings` 全部通过；OpenAPI JSON 包含 sync path。

### Task #12: 更新执行记录

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r009-supabase-sync.md`, Modify `docs/PROGRESS.md`

**Function:** 记录实现过程、验证结果和等待验收状态。

**Implementation Notes:** 每个 Stage 完成后更新任务状态和进度记录。未经用户验收不移动到 `docs/exec-plans/completed/`。

**Expected Verification Result:** 文档记录和实际实现状态一致。

## 进度记录

- 2026-05-04：完成需求澄清，确认使用 Supabase、后台常驻同步、默认 workspace、本仓库维护 Supabase migration、新增 sync API。
- 2026-05-04：完成设计文档和开发计划，等待用户审核。
- 2026-05-04：Stage #1 完成 sync 配置扩展与 Supabase Postgres migration 初稿，等待验证与提交。
- 2026-05-04：Stage #2 完成本地 `sync_changes` 生成，覆盖 note、note_revision、field、tag 和 note_tag 写入路径；已通过 `cargo fmt --check` 和 `cargo test notes`。
- 2026-05-04：Stage #3 完成 Supabase REST client、sync service、push/pull 游标编排和远端 change 幂等应用；已通过 `cargo fmt --check` 和 `cargo test sync`。
- 2026-05-04：Stage #4 完成后台同步 worker 与 `/sync/status`、`/sync/run`、`/sync/push`、`/sync/pull` API 接入，OpenAPI 已注册 sync path；已通过 `cargo fmt --check`、`cargo test sync` 和 `cargo check`。
- 2026-05-04：Stage #5 完成整体验证，已通过 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`；本地启动服务后 `/health` 返回 `200`，`/api-docs/openapi.json` 包含 sync API path。
