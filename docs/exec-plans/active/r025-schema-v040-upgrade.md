# r025 数据库 schema 升级到 v0.4.0

日期：2026-05-31

需求澄清文档：`docs/request-clarify/r025-schema-v040-upgrade.md`
设计文档：`docs/design-docs/r025-schema-v040-upgrade.md`

## Stage #1: 接入 shared schema v0.4.0

### Task #1: 升级 schema submodule

**状态：** Finished

| 项目 | 内容 |
| --- | --- |
| 文件 | `vendor/zembra-schema` |
| 功能 | 将 submodule 固定到 `v0.4.0` |
| 实现说明 | fetch tag 后 checkout `v0.4.0`，保持 detached tag 指针 |
| 预期验证结果 | `git -C vendor/zembra-schema describe --tags --exact-match HEAD` 输出 `v0.4.0` |

### Task #2: 接入 v0.4.0 migration

**状态：** Finished

| 项目 | 内容 |
| --- | --- |
| 文件 | `src/repositories/database.rs` |
| 功能 | 启动迁移执行 `004_add_hierarchical_tags.sql` |
| 实现说明 | 新增 migration 常量；迁移流程中检查 `schema_migrations` 是否已有 `0.4.0`；已有结构化 tags 字段时补写版本记录，否则执行 shared migration |
| 预期验证结果 | 新库和旧库迁移后都有 `schema_migrations.version = '0.4.0'` |

## Stage #2: 兼容层级标签写入和读取

### Task #3: 更新 taxonomy repository

**状态：** Finished

| 项目 | 内容 |
| --- | --- |
| 文件 | `src/repositories/taxonomy.rs`, `src/models/tag.rs` |
| 功能 | 新建标签时写入 `parent_tag_id/path/depth`，读取时返回完整路径 |
| 实现说明 | 标签字符串按 `/` 拆分为非空段；逐层按 `(workspace_id, parent_tag_id, name)` 或 root name 查找/创建；最终返回叶子节点，`TagRecord.name` 映射为 `path` |
| 预期验证结果 | `rust/web` 会创建 `rust` 和 `rust/web` 两个节点，笔记关联叶子节点，API 返回 `rust/web` |

### Task #4: 更新 notes 标签查询

**状态：** Finished

| 项目 | 内容 |
| --- | --- |
| 文件 | `src/repositories/notes/core.rs`, `src/repositories/notes/tags.rs`, `src/repositories/sync/apply.rs` |
| 功能 | notes 相关标签查询和远端 tag apply 兼容 v0.4.0 |
| 实现说明 | 标签 SELECT 使用 `path AS name`；按 tag name 删除时改为按 `path` 查找；远端 tag insert payload 读取层级字段并写入 v0.4.0 必填列 |
| 预期验证结果 | 现有 note/tag 关联、替换、随机标签测试保持通过 |

## Stage #3: 验证和记录

### Task #5: 补充测试并运行验证

**状态：** Finished

| 项目 | 内容 |
| --- | --- |
| 文件 | `src/repositories/database.rs`, `src/repositories/notes/tests.rs`, `docs/PROGRESS.md` |
| 功能 | 覆盖 migration 记录、层级标签创建和基础回归 |
| 实现说明 | 增加 v0.4.0 迁移断言和层级标签断言；完成后更新执行计划状态和 PROGRESS |
| 预期验证结果 | `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings` 全部通过 |

## 执行记录

- 2026-05-31：确认 `vendor/zembra-schema` 远端存在 `v0.4.0`，该版本新增结构化层级标签。
- 2026-05-31：完成 `004_add_hierarchical_tags.sql` 接入；新建标签按路径逐层创建，notes/taxonomy 查询继续通过 `TagRecord.name` 返回完整路径。
- 2026-05-31：已通过 `git -C vendor/zembra-schema describe --tags --exact-match HEAD`、`cargo fmt --check`、`cargo check`、`cargo test` 和 `cargo clippy -- -D warnings` 验证。
