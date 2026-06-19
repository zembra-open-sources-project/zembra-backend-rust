# r027 Supabase schema 适配归属迁移

日期：2026-06-19

需求澄清文档：`docs/request-clarify/r027-supabase-schema-ownership-migration.md`
设计文档：`docs/design-docs/r027-supabase-schema-ownership-migration.md`

## 关联设计文档

`docs/design-docs/r027-supabase-schema-ownership-migration.md`

## Stage #1: 移除本仓 Supabase schema 归属

### 任务 #1: 删除历史 Supabase migration

**Status:** Designed

**Files:**
- Delete: `supabase/migrations/001_initial_sync_schema.sql`

- 功能：移除本仓历史临时维护的 Supabase/Postgres schema 文件。
- 实现说明：只删除该 migration 文件；不新增替代 schema 文件，不修改 `vendor/zembra-schema`。
- 预期验证结果：`test ! -e supabase/migrations/001_initial_sync_schema.sql` 通过。

### 任务 #2: 修订 r009 历史文档开头

**Status:** Designed

**Files:**
- Modify: `docs/request-clarify/r009-supabase-sync.md`
- Modify: `docs/design-docs/r009-supabase-sync.md`
- Modify: `docs/exec-plans/active/r009-supabase-sync.md`

- 功能：让读者在进入 r009 历史正文前看到 r027 的新 schema ownership 口径。
- 实现说明：仅在文档开头增加简短说明和 r027 文档引用；不重写 r009 原有正文、表格和历史记录。
- 预期验证结果：三个 r009 文档开头都能看到远端 schema 已迁回 `zembra-schema v0.5.0` 的说明。

## Stage #2: 收敛 release 产物边界

### 任务 #3: 更新 release 相关文档

**Status:** Designed

**Files:**
- Modify: `docs/release.md`
- Modify: `docs/request-clarify/r012-github-release-pipeline.md`
- Modify: `docs/design-docs/r012-github-release-pipeline.md`
- Modify: `docs/exec-plans/active/r012-github-release-pipeline.md`

- 功能：移除 release 产物包含本仓 `supabase/migrations/` 或独立 schema 文件的表述。
- 实现说明：将 schema 来源说明收敛为消费固定版本 `vendor/zembra-schema`；release 包本身不复制 schema 文件。
- 预期验证结果：`rg -n "supabase/migrations" docs/release.md docs/request-clarify/r012-github-release-pipeline.md docs/design-docs/r012-github-release-pipeline.md docs/exec-plans/active/r012-github-release-pipeline.md` 不再返回发布产物相关命中。

## Stage #3: 验证和记录

### 任务 #4: 验证引用和范围

**Status:** Designed

**Files:**
- Verify: `docs/`
- Verify: `supabase/`
- Verify: `src/`
- Verify: `tests/`
- Verify: `config/`

- 功能：确认本仓不再把历史 Supabase migration 当作 schema 来源，同时保留新的消费路径说明。
- 实现说明：运行引用检查，确认剩余命中只允许出现在 r027 需求、设计、计划或技术债历史记录中；检查 diff 不包含 Rust 业务代码和 `vendor/zembra-schema` 内容修改。
- 预期验证结果：`rg -n "001_initial_sync_schema|supabase/migrations" docs src tests config supabase` 的命中符合 r027 预期；`rg -n "vendor/zembra-schema/postgres|vendor/zembra-schema/supabase" docs` 能找到新消费路径；`git diff --stat` 只包含 r027 范围内文件。

### 任务 #5: 更新执行计划状态并提交

**Status:** Designed

**Files:**
- Modify: `docs/exec-plans/active/r027-supabase-schema-ownership-migration.md`
- Verify: repository

- 功能：按实际执行结果更新任务状态，并创建原子提交。
- 实现说明：每个 Stage 完成后更新任务状态；提交前检查 `git status --porcelain`、`git diff --stat` 和必要 diff。commit message 使用 Conventional Commits。
- 预期验证结果：工作区只包含 r027 相关改动；提交信息满足 `docs: ...` 或 `chore: ...` 格式要求。
