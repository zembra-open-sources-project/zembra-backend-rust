# r008 数据库 schema 升级到 v0.3.0

日期：2026-05-04

需求澄清文档：`docs/request-clarify/r008-schema-v030-upgrade.md`

## 设计结论

升级共享 schema submodule 到 `v0.3.0`，并让后端启动迁移显式执行 `003_add_bidirectional_sync.sql`。现有 HTTP API 不新增 workspace 入参，统一落到 schema migration 创建的默认 workspace。

## 关键设计

| 领域 | 设计 |
| --- | --- |
| schema 固定 | 更新 `vendor/zembra-schema` submodule 指针到 `v0.3.0` |
| migration | 依次执行 `001_initial_schema.sql`、`002_add_note_role.sql`、`003_add_bidirectional_sync.sql` |
| 默认 workspace | 使用 `00000000-0000-4000-8000-000000000300` |
| notes CRUD | 所有读写 SQL 加入 `workspace_id` 约束 |
| taxonomy | field/tag 按默认 workspace 查询、创建和去重 |
| 兼容旧库 | 对已有表但缺少 `schema_migrations` 的库补写 `0.1.0`、`0.2.0`、`0.3.0` 记录 |

## 改动范围

| 文件 | 改动 |
| --- | --- |
| `vendor/zembra-schema` | submodule 指针升级到 `v0.3.0` |
| `src/repositories/database.rs` | 接入 `0.3.0` migration 和迁移记录兼容逻辑 |
| `src/repositories/taxonomy.rs` | 增加默认 workspace 常量，更新 field/tag SQL |
| `src/repositories/notes.rs` | 更新 note、revision、note_tag SQL 的 workspace 约束 |
| `docs/exec-plans/active/r008-schema-v030-upgrade.md` | 记录执行计划和进度 |

## 不做范围

- 不新增多 workspace API。
- 不实现 Supabase 双向同步业务逻辑。
- 不复制维护 shared schema 正文。
