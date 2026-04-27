# r003 共享数据库 Schema 引入

日期：2026-04-27

## 需求背景

后端需要接入共享数据库 schema，作为后续 SQLite migration、模型映射和 CRUD 数据访问的唯一契约来源。

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| Schema 来源 | `https://github.com/gawainx/zembra-schema.git` |
| 本仓库落点 | `vendor/zembra-schema` |
| 固定版本 | `v0.2.0` |
| 契约内容 | 数据表说明、SQLite DDL、JSON Schema 和 migration |
| 维护方式 | 本仓库不复制维护 schema 正文，通过 submodule 指针固定版本 |
| 初始化方式 | 首次拉取后执行 `git submodule update --init --recursive` |

## 验收标准

- 仓库包含 `.gitmodules`，并指向 `vendor/zembra-schema`。
- `vendor/zembra-schema` 固定到 tag `v0.2.0`。
- 共享 schema 中的 migration 文件可被后续 SQLx 接入使用。
