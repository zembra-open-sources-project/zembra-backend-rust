# r027 Supabase schema 适配归属迁移

日期：2026-06-19

需求澄清文档：`docs/request-clarify/r027-supabase-schema-ownership-migration.md`

## 核心功能（WHAT）

完整接入 `zembra-schema v0.5.0` 统一契约：后端 SQLite migration 链推进到 `0.5.0`，远端 Supabase/Postgres schema 归属迁回 `zembra-schema`，本仓删除历史临时 Supabase migration，并通过文档和 release 边界说明本仓只消费固定版本 schema 产物。

### 需求背景（WHY）

`zembra-schema v0.5.0` 引入了三类和本仓相关的变化：

| 变化 | 对本仓影响 |
| --- | --- |
| SQLite 登记迁移 `005_register_unified_postgres_contract.sql` | 本地启动迁移必须记录 `schema_migrations.version = '0.5.0'` |
| Postgres DDL 和 migration | 远端业务 schema 来源改为 `vendor/zembra-schema/postgres/` |
| Supabase 平台边界说明 | 本仓不能继续把 Supabase 平台配置和业务 schema 混在 `supabase/migrations/` 中维护 |

当前代码的 SQLite migration 链只执行到 `0.4.0`，且本仓仍保留 `supabase/migrations/001_initial_sync_schema.sql`。如果只删除历史 migration，不接入 `005`，本仓仍没有完整消费 `v0.5.0`。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| SQLite 版本推进 | 新库和既有库迁移后都记录 `schema_migrations.version = '0.5.0'` |
| 归属收敛 | Supabase/Postgres schema 只由 `zembra-schema` 维护 |
| 本仓消费 | 本仓引用 `vendor/zembra-schema/migrations/`、`vendor/zembra-schema/postgres/` 和 `vendor/zembra-schema/supabase/README.md` |
| 历史口径修订 | r009 历史文档只在开头增加 r027 归属说明，不重写原始正文 |
| 发布边界收敛 | release 产物不包含本仓 schema 文件或 `supabase/migrations/` |
| 业务行为稳定 | 不修改 sync worker、REST client、配置、HTTP API 和业务 repository 行为 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| SQLite v0.5.0 接入 | 在 `src/repositories/database.rs` include 并执行 `005_register_unified_postgres_contract.sql` |
| 迁移兼容 | 对已有 v0.5.0 结构或已记录版本保持幂等；对已有 v0.4.0 库补写 `0.5.0` |
| 数据库测试 | 增加 `0.5.0` migration 记录断言 |
| 删除历史 migration | 删除 `supabase/migrations/001_initial_sync_schema.sql` |
| r009 文档头部修订 | 在需求、设计、执行计划开头说明 schema 归属已由 r027 迁回 `zembra-schema` |
| release 文档修订 | 移除本仓 `supabase/migrations/` 作为发布产物的表述 |
| 消费路径说明 | 明确远端 schema 来源为 `vendor/zembra-schema/postgres/` 和 `vendor/zembra-schema/supabase/README.md` |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| `zembra-schema` 内容 | 不修改 schema、migration、DDL、JSON Schema 或 Supabase 说明 |
| 本仓 Postgres schema | 不新增 Postgres migration、DDL、表结构、字段定义、索引或约束 |
| r009 正文重写 | 保留历史文档正文，只在开头追加新归属说明 |
| sync 业务逻辑 | 不修改 worker、push/pull、REST client、配置、HTTP API 或业务 repository 行为 |
| r026 技术债 | 不处理层级标签乱序补偿 |

## 实现流程（HOW）

### 设计结论

本次改动分为运行时契约接入和仓库归属收敛两部分。运行时接入只触碰 SQLite migration 编排，让本地数据库记录 `0.5.0`；远端 Postgres/Supabase schema 不在本仓实现，只通过文档和 release 边界指向 `vendor/zembra-schema`。

### SQLite migration 设计

| 关注点 | 方案 |
| --- | --- |
| migration 来源 | `vendor/zembra-schema/migrations/005_register_unified_postgres_contract.sql` |
| 代码入口 | `src/repositories/database.rs` |
| 新库路径 | `001_initial_schema.sql` 创建基础表后，依次执行 `002`、`003`、`004`、`005` 检查逻辑 |
| 既有库路径 | 已有 `schema_migrations` 且缺少 `0.5.0` 时执行 `005` |
| 缺元数据旧库 | `bootstrap_schema_migrations` 在识别到 v0.4.0 结构后补写 `0.5.0` |
| 幂等性 | 先查 `schema_migrations`，已存在 `0.5.0` 时不执行登记迁移 |

`005_register_unified_postgres_contract.sql` 只写入 schema version，不新增 SQLite 表或字段。本仓不得把 Postgres DDL 翻译成本地 migration。

### Supabase/Postgres 归属设计

| 文件 | 角色 |
| --- | --- |
| `vendor/zembra-schema/postgres/001_initial_schema.sql` | Postgres 当前完整 DDL |
| `vendor/zembra-schema/postgres/migrations/005_add_unified_schema_contract.sql` | Postgres v0.5.0 bootstrap migration |
| `vendor/zembra-schema/supabase/README.md` | Supabase 平台配置边界说明 |
| `supabase/migrations/001_initial_sync_schema.sql` | 本仓历史临时产物，本次删除 |

### 文件影响

| 文件 | 变更 |
| --- | --- |
| `src/repositories/database.rs` | 接入 SQLite `005` 登记迁移并更新测试 |
| `supabase/migrations/001_initial_sync_schema.sql` | 删除历史临时 migration |
| `docs/request-clarify/r009-supabase-sync.md` | 开头增加 r027 归属迁移说明 |
| `docs/design-docs/r009-supabase-sync.md` | 开头增加 r027 归属迁移说明 |
| `docs/exec-plans/active/r009-supabase-sync.md` | 开头增加 r027 归属迁移说明并保留历史正文 |
| `docs/release.md` | 移除 `supabase/migrations/` 发布产物表述 |
| `docs/request-clarify/r012-github-release-pipeline.md` | 修订 release 产物范围，不包含 schema |
| `docs/design-docs/r012-github-release-pipeline.md` | 修订 release 产物范围，不包含 schema |
| `docs/exec-plans/active/r012-github-release-pipeline.md` | 修订 release 产物范围，不包含 schema |
| `docs/exec-plans/active/r027-supabase-schema-ownership-migration.md` | 执行时更新任务状态和验证记录 |

### 历史文档修订规则

r009 文档只允许在开头加入简短说明，说明远端 schema 归属已经由 r027 调整到 `zembra-schema v0.5.0`，并说明本仓运行时会消费 SQLite `005` 登记迁移。原 r009 正文保留为历史记录，不做段落重写。

### Release 产物规则

release 文档不再把本仓 `supabase/migrations/` 或任何独立 schema 文件列为产物。若需要说明 schema 来源，只能写为“使用仓库固定的 `vendor/zembra-schema` 契约”，不能把 schema 文件复制为本仓发布物。

## 测试用例

### 编译检查

| 用例 | 预期 |
| --- | --- |
| `cargo fmt --check` | 格式检查通过 |
| `cargo check` | 编译通过 |

### 自动化测试

| 用例 | 预期 |
| --- | --- |
| 新库 migration | `Database::connect("sqlite://:memory:")` 后 `schema_migrations` 包含 `0.5.0` |
| 缺元数据旧库补写 | 删除 `schema_migrations` 后重新 migrate，`0.1.0` 到 `0.5.0` 都存在 |
| 定向数据库测试 | `cargo test database` 或等价定向测试通过 |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| `test ! -e supabase/migrations/001_initial_sync_schema.sql` | 历史 migration 已删除 |
| `rg -n "supabase/migrations|001_initial_sync_schema" docs src tests config supabase` | 不再存在把本仓 migration 作为可执行 schema 来源的说明 |
| `rg -n "vendor/zembra-schema/postgres|vendor/zembra-schema/supabase" docs` | 文档能找到新的远端 schema 消费路径 |

### 回归检查

| 用例 | 预期 |
| --- | --- |
| `git status --porcelain` | 只包含 r027 范围内的代码、文档和历史 migration 删除 |
| `git diff --stat` | 不包含 `vendor/zembra-schema` 内容修改，不包含 sync 业务逻辑改动 |
