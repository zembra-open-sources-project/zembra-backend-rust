# r008 数据库 schema 升级到 v0.3.0

日期：2026-05-04

需求澄清文档：`docs/request-clarify/r008-schema-v030-upgrade.md`
设计文档：`docs/design-docs/r008-schema-v030-upgrade.md`

## Stage 1: Schema 指针与 migration

| Task | 状态 | 功能 | 实现要点 | 预期测试结果 |
| --- | --- | --- | --- | --- |
| T1 | Finished | 升级 shared schema submodule | 将 `vendor/zembra-schema` 固定到 `v0.3.0` | `git -C vendor/zembra-schema describe --tags --exact-match HEAD` 输出 `v0.3.0` |
| T2 | Finished | 接入 v0.3.0 migration | include `003_add_bidirectional_sync.sql`，启动时记录 `0.3.0` | 新库和旧库迁移后存在 workspace 与 sync 表 |

## Stage 2: Repository 兼容

| Task | 状态 | 功能 | 实现要点 | 预期测试结果 |
| --- | --- | --- | --- | --- |
| T3 | Finished | notes SQL 兼容 workspace schema | notes、note_revisions、note_tags 读写统一使用默认 workspace | note CRUD 测试通过 |
| T4 | Finished | taxonomy SQL 兼容 workspace schema | fields、tags 查询创建按默认 workspace 约束 | taxonomy 测试通过 |

## Stage 3: 验证

| Task | 状态 | 功能 | 实现要点 | 预期测试结果 |
| --- | --- | --- | --- | --- |
| T5 | Finished | 工程验证 | 执行 fmt、check、test、clippy | 全部通过 |
| T6 | Finished | 启动验收服务 | 使用 `.zembra.env` 启动后端服务 | 服务可访问，等待用户验收 |

## 进度记录

- 2026-05-04：确认 `vendor/zembra-schema` 远端存在 `v0.3.0`，该版本新增 workspace 维度与同步表。
- 2026-05-04：完成 submodule 指针升级、migration 接入，以及 notes/taxonomy SQL 默认 workspace 兼容。
- 2026-05-04：已通过 `cargo fmt --check`、`cargo check`、`cargo test` 和 `cargo clippy -- -D warnings` 验证。
- 2026-05-04：已启动服务并验证 `/health` 返回 `200 OK`，`/api-docs/openapi.json` 返回 `200 OK`；当前数据库 `/Users/yat/.zembra/zembra.sqlite3` 已记录 `0.1.0`、`0.2.0`、`0.3.0`，并包含 `workspaces`、`sync_changes`、`sync_state`、`sync_conflicts`。
