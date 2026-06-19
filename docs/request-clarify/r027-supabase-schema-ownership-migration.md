# r027 Supabase schema 适配归属迁移

日期：2026-06-19

## 需求结论

根据 `zembra-schema v0.5.0`，完整接入统一 SQLite/Postgres schema 契约。本仓库需要把本地 SQLite migration 链推进到 `schema_migrations.version = '0.5.0'`，同时将 Supabase/Postgres 远端 schema 的契约归属迁回 `zembra-schema`，删除本仓历史临时 Supabase migration，并收敛 release 产物边界。

## 背景

`r009` 接入 Supabase 后台同步时，本仓库曾新增 `supabase/migrations/001_initial_sync_schema.sql`，用于临时补齐远端 Postgres 表结构。现在 `zembra-schema v0.5.0` 已经提供统一 Postgres 契约、SQLite `0.5.0` 登记迁移和 Supabase 平台边界说明。本仓库继续保留独立 Supabase schema 会造成 source of truth 分裂；本地 SQLite 启动迁移如果仍停在 `0.4.0`，也无法体现已经消费 `v0.5.0` 契约。

## 范围

| 项目 | 结论 |
| --- | --- |
| 共享 schema 来源 | `vendor/zembra-schema` |
| 目标版本 | `v0.5.0` |
| SQLite migration | 接入 `vendor/zembra-schema/migrations/005_register_unified_postgres_contract.sql` |
| SQLite 版本状态 | 新库和既有 v0.4.0 库迁移后都应记录 `schema_migrations.version = '0.5.0'` |
| 远端 Postgres 契约 | 本仓只引用 `vendor/zembra-schema/postgres/`，不维护独立 Postgres DDL 或 migration |
| Supabase 平台边界 | 本仓只引用 `vendor/zembra-schema/supabase/README.md` 中的平台配置边界 |
| 历史 Supabase migration | 直接删除本仓库 `supabase/migrations/001_initial_sync_schema.sql` |
| 历史文档处理 | 只在开头修订引用到 r027，不重写历史正文 |
| Release 产物 | 不包含 schema 文件或本仓 `supabase/migrations/` |
| 后端 sync 业务逻辑 | 保留现有 worker、REST client、配置、HTTP API 和本地 sync repository 行为 |

## 验收标准

- `src/repositories/database.rs` 接入 `005_register_unified_postgres_contract.sql`，启动迁移会补齐 `schema_migrations.version = '0.5.0'`。
- 数据库测试覆盖新库和已有 v0.4.0 元数据缺失/补写场景，断言 `0.5.0` 存在。
- 本仓库不再保留 `supabase/migrations/001_initial_sync_schema.sql`。
- `r009` 相关历史文档开头说明远端 schema 归属已由 `r027` 迁回 `zembra-schema v0.5.0`，并保留历史正文。
- release 相关文档不再把本仓 `supabase/migrations/` 或独立 schema 文件列为发布产物。
- 仓库引用关系能够说明 Supabase/Postgres schema 应使用 `vendor/zembra-schema/postgres/` 和 `vendor/zembra-schema/supabase/README.md`。
- 检查结果确认本仓没有继续把本地 `supabase/migrations/001_initial_sync_schema.sql` 当作可执行 schema 来源。
- Rust 格式化、定向数据库测试和编译检查通过。

## 非范围

- 不修改 `zembra-schema` 的 schema 内容。
- 不在本仓新增 Postgres migration、DDL、表结构、字段定义、索引或约束。
- 不重写 r009 历史需求、设计或计划正文。
- 不修改 sync worker、push/pull、REST client、配置、HTTP API 或业务 repository 行为。
- 不处理 r026 的层级标签乱序补偿。
