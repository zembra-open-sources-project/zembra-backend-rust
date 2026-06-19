# r027 Supabase schema 适配归属迁移

日期：2026-06-19

## 需求结论

根据 `zembra-schema v0.5.0`，将 Supabase/Postgres 远端 schema 的契约归属从本仓库迁回 `zembra-schema`。本仓库只消费 `vendor/zembra-schema` 中固定版本的数据契约，不再维护、发布或修补独立的 Supabase/Postgres schema migration。

## 背景

`r009` 接入 Supabase 后台同步时，本仓库曾新增 `supabase/migrations/001_initial_sync_schema.sql`，用于临时补齐远端 Postgres 表结构。现在 `zembra-schema v0.5.0` 已经提供统一 Postgres 契约和 Supabase 边界说明，项目也已明确 `zembra-schema` 是 SQLite、Supabase/Postgres 以及后续数据库形态的唯一数据契约来源。

## 范围

| 项目 | 结论 |
| --- | --- |
| 远端 schema 来源 | `vendor/zembra-schema` |
| 目标版本 | `v0.5.0` |
| 历史 Supabase migration | 直接删除本仓库 `supabase/migrations/001_initial_sync_schema.sql` |
| 历史文档处理 | 只在开头修订引用到 r027，不重写历史正文 |
| Release 产物 | 不包含 schema 文件或本仓 `supabase/migrations/` |
| 后端 sync 逻辑 | 保留现有 worker、REST client、配置、HTTP API 和本地 sync repository 行为 |

## 验收标准

- 本仓库不再保留 `supabase/migrations/001_initial_sync_schema.sql`。
- `r009` 相关历史文档开头说明远端 schema 归属已由 `r027` 迁回 `zembra-schema`。
- release 相关文档不再把本仓 `supabase/migrations/` 或独立 schema 文件列为发布产物。
- 仓库引用关系能够说明 Supabase/Postgres schema 应使用 `vendor/zembra-schema/postgres/` 和 `vendor/zembra-schema/supabase/README.md`。
- 检查结果确认本仓没有继续把本地 `supabase/migrations/001_initial_sync_schema.sql` 当作可执行 schema 来源。

## 非范围

- 不修改 `zembra-schema` 的 schema 内容。
- 不新增 Postgres migration。
- 不重写 r009 历史需求、设计或计划正文。
- 不修改 sync worker、push/pull、REST client 的业务逻辑。
- 不处理 r026 的层级标签乱序补偿。
