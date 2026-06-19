# r027 Supabase schema 适配归属迁移

日期：2026-06-19

需求澄清文档：`docs/request-clarify/r027-supabase-schema-ownership-migration.md`

## 核心功能（WHAT）

将 Supabase/Postgres 远端 schema 的维护归属从本仓库迁回 `zembra-schema`。本仓库删除历史临时 migration，只保留 sync 业务实现，并通过文档说明远端 schema 应消费 `vendor/zembra-schema` 固定版本产物。

### 需求背景（WHY）

`r009` 为了快速接入 Supabase 后台同步，在本仓库新增了 `supabase/migrations/001_initial_sync_schema.sql`。`zembra-schema v0.5.0` 已经发布统一 Postgres 契约、SQLite 登记迁移和 Supabase 边界说明，继续在本仓维护独立 Supabase schema 会造成 schema source of truth 分裂。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 归属收敛 | Supabase/Postgres schema 只由 `zembra-schema` 维护 |
| 本仓消费 | 本仓只引用 `vendor/zembra-schema/postgres/` 和 `vendor/zembra-schema/supabase/README.md` |
| 历史口径修订 | r009 历史文档只在开头增加 r027 归属说明，不重写原始正文 |
| 发布边界收敛 | release 产物不包含本仓 schema 文件或 `supabase/migrations/` |
| 业务行为稳定 | 不修改 sync worker、REST client、配置、HTTP API 和 repository 业务逻辑 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| 删除历史 migration | 删除 `supabase/migrations/001_initial_sync_schema.sql` |
| r009 文档头部修订 | 在需求、设计、执行计划开头说明 schema 归属已由 r027 迁回 `zembra-schema` |
| release 文档修订 | 移除本仓 `supabase/migrations/` 作为发布产物的表述 |
| 消费路径说明 | 明确远端 schema 来源为 `vendor/zembra-schema/postgres/` 和 `vendor/zembra-schema/supabase/README.md` |
| 引用验证 | 检查仓库不再把本地 `supabase/migrations/001_initial_sync_schema.sql` 当作可执行 schema 来源 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| `zembra-schema` 内容 | 不修改 schema、migration、DDL、JSON Schema 或 Supabase 说明 |
| 新增 Postgres migration | 本仓不新增任何数据库 schema 产物 |
| r009 正文重写 | 保留历史文档正文，只在开头追加新归属说明 |
| sync 业务逻辑 | 不修改 worker、push/pull、REST client、配置、HTTP API 或 repository 行为 |
| r026 技术债 | 不处理层级标签乱序补偿 |

## 实现流程（HOW）

### 设计结论

本次改动是文档和仓库边界修正，不是 schema 迁移实现。删除本仓历史 Supabase migration 后，部署或初始化 Supabase/Postgres 远端 schema 的入口统一转向 `vendor/zembra-schema/postgres/`；Supabase 平台专属配置边界转向 `vendor/zembra-schema/supabase/README.md`。

### 文件影响

| 文件 | 变更 |
| --- | --- |
| `supabase/migrations/001_initial_sync_schema.sql` | 删除历史临时 migration |
| `docs/request-clarify/r009-supabase-sync.md` | 开头增加 r027 归属迁移说明 |
| `docs/design-docs/r009-supabase-sync.md` | 开头增加 r027 归属迁移说明 |
| `docs/exec-plans/active/r009-supabase-sync.md` | 开头增加 r027 归属迁移说明 |
| `docs/release.md` | 移除 `supabase/migrations/` 发布产物表述 |
| `docs/request-clarify/r012-github-release-pipeline.md` | 修订 release 产物范围，不包含 schema |
| `docs/design-docs/r012-github-release-pipeline.md` | 修订 release 产物范围，不包含 schema |
| `docs/exec-plans/active/r012-github-release-pipeline.md` | 修订 release 产物范围，不包含 schema |

### 历史文档修订规则

r009 文档只允许在开头加入简短说明，说明远端 schema 归属已经由 r027 调整到 `zembra-schema v0.5.0`。原 r009 正文保留为历史记录，不做段落重写，避免把历史执行上下文改成当前事实。

### Release 产物规则

release 文档不再把本仓 `supabase/migrations/` 或任何独立 schema 文件列为产物。若需要说明 schema 来源，只能写为“使用仓库固定的 `vendor/zembra-schema` 契约”，不能把 schema 文件复制为本仓发布物。

## 测试用例

### 编译检查

| 用例 | 预期 |
| --- | --- |
| `cargo check` | 不要求作为本轮必跑项，因为本轮不修改 Rust 代码 |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| `test ! -e supabase/migrations/001_initial_sync_schema.sql` | 历史 migration 已删除 |
| `rg -n "supabase/migrations|001_initial_sync_schema" docs src tests config supabase` | 不再存在把本仓 migration 作为可执行 schema 来源的说明 |
| `rg -n "vendor/zembra-schema/postgres|vendor/zembra-schema/supabase" docs` | 文档能找到新的远端 schema 消费路径 |

### 回归检查

| 用例 | 预期 |
| --- | --- |
| `git status --porcelain` | 只包含 r027 范围内的文档和历史 migration 删除 |
| `git diff --stat` | 不包含 `vendor/zembra-schema` 内容修改，不包含 Rust 业务代码修改 |
