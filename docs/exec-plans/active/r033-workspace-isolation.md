# r033 workspace 隔离机制执行计划

需求澄清文档：`docs/request-clarify/r033-workspace-isolation.md`

设计文档：`docs/design-docs/r033-workspace-isolation.md`

## Stage #1: workspace 摘要 API

### 任务 #1: 新增 workspace 摘要查询

**Status:** Finished

**Files:** Create/Modify `src/repositories/workspaces.rs`, `src/repositories/mod.rs`, tests

功能 / 实现说明 / 预期验证结果：新增 workspace repository，按可见 notes 聚合 `visible_note_count` 和 `latest_note_created_at`，派生 UUID 去连字符前 8 位 `short_hash`；`tests/workspaces_routes.rs` 覆盖排序、归档删除过滤和空 workspace。

### 任务 #2: 暴露 GET /workspaces

**Status:** Finished

**Files:** Create/Modify `src/dto/workspaces.rs`, `src/handlers/workspaces.rs`, `src/routes/workspaces.rs`, `src/app.rs`, `src/api_doc.rs`, `tests/openapi_routes.rs`

功能 / 实现说明 / 预期验证结果：新增 API handler、route 和 OpenAPI schema，返回完整 workspace id、短 hash、可见笔记数量和最近可见笔记创建时间；`cargo test --test workspaces_routes` 和 `cargo test openapi_json_lists_runtime_api_paths` 通过。

## Stage #2: 初始化与迁移

### 任务 #1: 新初始化生成随机 workspace

**Status:** Finished

**Files:** Modify `src/repositories/database.rs`, `src/init.rs`, `tests/init_tests.rs`

功能 / 实现说明 / 预期验证结果：文件数据库连接后会把 shared schema 创建的 legacy fixed workspace 迁移成本地随机 UUID；内存测试库保留 legacy fixture 兼容。`tests/init_tests.rs` 断言 `zembra-backend init` 创建的新库 workspace id 不是 legacy id。

### 任务 #2: 固定 legacy workspace 本地迁移

**Status:** Finished

**Files:** Modify `src/repositories/database.rs`, repository tests

功能 / 实现说明 / 预期验证结果：检测本地固定 legacy workspace 后，更新 `workspaces.id`，依赖 shared schema 的外键级联把本地业务表、同步表和状态表引用迁移到同一个新 UUID；不执行任何远端写入。

## Stage #3: 多 workspace 同步

### 任务 #1: 去除远端业务表固定 workspace 过滤

**Status:** Finished

**Files:** Modify `src/sync/supabase.rs`, sync tests

功能 / 实现说明 / 预期验证结果：Supabase 快照读取不再只按固定默认 workspace 过滤业务表和 `sync_changes`；`sync::supabase` 请求构造测试确认 URL 不含固定 `workspace_id` filter。

### 任务 #2: 防止本地随机空 workspace 污染远端

**Status:** Finished

**Files:** Modify `src/sync/diff.rs`, tests

功能 / 实现说明 / 预期验证结果：表级同步当前不走旧增量 outbox/state 路径；真正需要拦截的是本地随机空 workspace 作为 local-only row 被推到远端。`diff_snapshots` 跳过没有任何依赖同步行的本地独有空 workspace，`sync::diff` 测试覆盖该行为。

## Stage #4: 验证与收尾

### 任务 #1: 完整验证与真实启动验收准备

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r033-workspace-isolation.md`, `docs/PROGRESS.md`

功能 / 实现说明 / 预期验证结果：已通过 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`。已在 sandbox 外执行 `cargo run` 启动真实 backend，后台同步日志显示 `background synchronization cycle finished pushed=0 pulled=116`，未再出现由 workspace id 引起的同步错误。
