# r025 数据库 schema 升级到 v0.4.0

日期：2026-05-31

## 需求结论

将后端使用的共享数据库 schema 从 `v0.3.0` 升级到 `v0.4.0`，继续以 `vendor/zembra-schema` submodule 作为唯一 schema 契约来源。

## 范围

| 项目 | 结论 |
| --- | --- |
| schema 来源 | `vendor/zembra-schema` |
| 目标版本 | `v0.4.0` |
| migration | 新增执行 `004_add_hierarchical_tags.sql` |
| 后端兼容策略 | 新建标签按层级标签字段写入，既有 notes/taxonomy API 保持可用 |
| 旧数据兼容 | 使用 shared migration 将旧斜杠路径标签拆成层级节点，并保留叶子 tag id |

## 验收标准

- `vendor/zembra-schema` 固定到 tag `v0.4.0`。
- 启动迁移会记录 `schema_migrations.version = '0.4.0'`。
- 新建普通标签和斜杠路径标签在 `tags` 表写入 `parent_tag_id`、`path`、`depth`。
- 现有 notes 和 taxonomy API 测试继续通过。

## 非范围

- 不新增层级标签管理 API。
- 不改变现有 HTTP 请求/响应结构。
- 不复制维护 shared schema 正文。
