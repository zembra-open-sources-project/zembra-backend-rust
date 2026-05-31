# r025 数据库 schema 升级到 v0.4.0

日期：2026-05-31

需求澄清文档：`docs/request-clarify/r025-schema-v040-upgrade.md`

## 设计结论

升级共享 schema submodule 到 `v0.4.0`，并让后端启动迁移显式执行 `004_add_hierarchical_tags.sql`。现有标签字符串继续作为 API 层输入输出；repository 在写入时把斜杠路径拆成层级节点，最终返回并关联叶子标签。

## 关键设计

| 关注点 | 方案 |
| --- | --- |
| schema 固定 | 更新 `vendor/zembra-schema` submodule 指针到 `v0.4.0` |
| migration | 依次执行 `001_initial_schema.sql`、`002_add_note_role.sql`、`003_add_bidirectional_sync.sql`、`004_add_hierarchical_tags.sql` |
| 兼容旧库 | 对已有 `parent_tag_id/path/depth` 字段但缺少迁移记录的库补写 `0.4.0` |
| 标签写入 | `get_or_create_tag_in_transaction` 按 `/` 拆分路径，逐层创建 tag 节点 |
| 标签读取 | `TagRecord.name` 保持现有响应字段，查询使用 `COALESCE(tags.path, tags.name)` 返回完整路径 |
| 同步 payload | tag insert payload 增加 `parent_tag_id`、`path`、`depth`，保持已有字段兼容 |

## 文件影响

| 文件 | 变更 |
| --- | --- |
| `vendor/zembra-schema` | submodule 指针升级到 `v0.4.0` |
| `src/repositories/database.rs` | 加载并执行 v0.4.0 migration，补齐迁移记录检测 |
| `src/repositories/taxonomy.rs` | 层级创建标签，按路径查询和列出标签 |
| `src/repositories/notes/*.rs` | 标签查询按完整路径返回 |
| `docs/exec-plans/active/r025-schema-v040-upgrade.md` | 记录执行计划和进度 |

## 验证策略

- 检查 submodule tag：`git -C vendor/zembra-schema describe --tags --exact-match HEAD`。
- 运行定向 migration/taxonomy 测试，覆盖 `0.4.0` 迁移记录和层级标签写入。
- 运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`。
