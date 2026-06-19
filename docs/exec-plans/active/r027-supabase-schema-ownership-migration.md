# r027 Supabase schema 适配归属迁移

日期：2026-06-19

需求澄清文档：`docs/request-clarify/r027-supabase-schema-ownership-migration.md`
设计文档：`docs/design-docs/r027-supabase-schema-ownership-migration.md`

## 关联设计文档

`docs/design-docs/r027-supabase-schema-ownership-migration.md`

## Stage #1: 接入 shared schema v0.5.0

### 任务 #1: 接入 SQLite v0.5.0 登记迁移

**Status:** Designed

**Files:**
- Modify: `src/repositories/database.rs`

- 功能：让后端启动迁移完整消费 `zembra-schema v0.5.0`。
- 实现说明：新增 `include_str!("../../vendor/zembra-schema/migrations/005_register_unified_postgres_contract.sql")` 常量；在 `migrate()` 中检查 `schema_migrations` 是否已有 `0.5.0`，缺失时执行该登记迁移；保持 `0.1.0` 到 `0.4.0` 的既有顺序不变。
- 预期验证结果：新库启动后 `schema_migrations.version` 包含 `0.5.0`。

### 任务 #2: 补齐旧库迁移元数据兼容

**Status:** Designed

**Files:**
- Modify: `src/repositories/database.rs`

- 功能：让已有 v0.4.0 结构但缺少迁移元数据的本地库补写到 `0.5.0`。
- 实现说明：`bootstrap_schema_migrations` 在确认 v0.4.0 结构存在后，继续补写 `0.5.0`；更新函数文档中仍写 v0.4.0 的描述。
- 预期验证结果：删除 `schema_migrations` 后重新运行 `migrate()`，`0.1.0`、`0.2.0`、`0.3.0`、`0.4.0`、`0.5.0` 都存在。

### 任务 #3: 更新数据库测试

**Status:** Designed

**Files:**
- Modify: `src/repositories/database.rs`

- 功能：用测试锁定 v0.5.0 接入结果。
- 实现说明：更新现有 migration metadata 测试断言到 `0.5.0`；必要时新增新库 migrate 后包含 `0.5.0` 的测试。
- 预期验证结果：`cargo test database` 或等价定向测试通过，并能覆盖 `0.5.0` schema version。

## Stage #2: 移除本仓 Supabase schema 归属

### 任务 #4: 删除历史 Supabase migration

**Status:** Designed

**Files:**
- Delete: `supabase/migrations/001_initial_sync_schema.sql`

- 功能：移除本仓历史临时维护的 Supabase/Postgres schema 文件。
- 实现说明：只删除该 migration 文件；不新增替代 schema 文件，不修改 `vendor/zembra-schema`。
- 预期验证结果：`test ! -e supabase/migrations/001_initial_sync_schema.sql` 通过。

### 任务 #5: 修订 r009 历史文档开头

**Status:** Designed

**Files:**
- Modify: `docs/request-clarify/r009-supabase-sync.md`
- Modify: `docs/design-docs/r009-supabase-sync.md`
- Modify: `docs/exec-plans/active/r009-supabase-sync.md`

- 功能：让读者在进入 r009 历史正文前看到 r027 的新 schema ownership 和 v0.5.0 接入口径。
- 实现说明：仅在文档开头增加简短说明和 r027 文档引用；不重写 r009 原有正文、表格和历史记录。
- 预期验证结果：三个 r009 文档开头都能看到远端 schema 已迁回 `zembra-schema v0.5.0`，且本仓运行时接入 SQLite `005` 登记迁移。

## Stage #3: 收敛 release 产物边界

### 任务 #6: 更新 release 相关文档

**Status:** Designed

**Files:**
- Modify: `docs/release.md`
- Modify: `docs/request-clarify/r012-github-release-pipeline.md`
- Modify: `docs/design-docs/r012-github-release-pipeline.md`
- Modify: `docs/exec-plans/active/r012-github-release-pipeline.md`

- 功能：移除 release 产物包含本仓 `supabase/migrations/` 或独立 schema 文件的表述。
- 实现说明：将 schema 来源说明收敛为消费固定版本 `vendor/zembra-schema`；release 包本身不复制 schema 文件。
- 预期验证结果：`rg -n "supabase/migrations" docs/release.md docs/request-clarify/r012-github-release-pipeline.md docs/design-docs/r012-github-release-pipeline.md docs/exec-plans/active/r012-github-release-pipeline.md` 不再返回发布产物相关命中。

## Stage #4: 验证和记录

### 任务 #7: 运行验证

**Status:** Designed

**Files:**
- Verify: `src/repositories/database.rs`
- Verify: `docs/`
- Verify: `supabase/`
- Verify: `src/`
- Verify: `tests/`
- Verify: `config/`

- 功能：确认本地 SQLite 已接入 `0.5.0`，且本仓不再把历史 Supabase migration 当作 schema 来源。
- 实现说明：运行格式、定向数据库测试、编译检查和引用检查；确认 diff 不包含 sync 业务逻辑或 `vendor/zembra-schema` 内容修改。
- 预期验证结果：`cargo fmt --check`、`cargo test database`、`cargo check` 通过；`rg -n "001_initial_sync_schema|supabase/migrations" docs src tests config supabase` 的命中符合 r027 预期；`rg -n "vendor/zembra-schema/postgres|vendor/zembra-schema/supabase" docs` 能找到新消费路径；`git diff --stat` 只包含 r027 范围内文件。

### 任务 #8: 更新执行计划状态并提交

**Status:** Designed

**Files:**
- Modify: `docs/exec-plans/active/r027-supabase-schema-ownership-migration.md`
- Verify: repository

- 功能：按实际执行结果更新任务状态，并创建原子提交。
- 实现说明：每个 Stage 完成后更新任务状态；提交前检查 `git status --porcelain`、`git diff --stat` 和必要 diff。commit message 使用 Conventional Commits。
- 预期验证结果：工作区只包含 r027 相关改动；提交信息满足 `fix: ...`、`chore: ...` 或 `docs: ...` 格式要求。
