# r003 共享数据库 Schema 引入设计

日期：2026-04-27

需求澄清文档：`docs/request-clarify/r003-shared-schema-submodule.md`

## 设计目标

通过 Git submodule 引入共享 schema 仓库，并固定到 `v0.2.0`。后端继续保持不复制 schema 正文的约束，后续数据访问、migration 执行和模型映射都以 `vendor/zembra-schema` 为准。

## Submodule 设计

| 项目 | 设计 |
| --- | --- |
| 路径 | `vendor/zembra-schema` |
| URL | `https://github.com/gawainx/zembra-schema.git` |
| 固定版本 | tag `v0.2.0` |
| 当前 commit | `cd37a7e` |
| 版本升级方式 | 在 submodule 内切换 tag 或 commit，再提交主仓 submodule 指针 |

## Schema 内容入口

| 路径 | 用途 |
| --- | --- |
| `vendor/zembra-schema/migrations/` | 后续 SQLx migration 接入入口 |
| `vendor/zembra-schema/sqlite/` | SQLite DDL 参考 |
| `vendor/zembra-schema/json/` | JSON Schema 参考 |
| `vendor/zembra-schema/note_schema.md` | 数据表说明参考 |

## 预期改动范围

| 文件 | 改动 |
| --- | --- |
| `.gitmodules` | 新增 `vendor/zembra-schema` submodule 配置 |
| `vendor/zembra-schema` | 固定 submodule 指针到 `v0.2.0` |
| `.env.example` | 对齐当前 `database.path` 配置字段 |
| `docs/request-clarify/r003-shared-schema-submodule.md` | 记录需求澄清结果 |
| `docs/exec-plans/active/r003-shared-schema-submodule.md` | 记录执行计划与进度 |

## 验证方式

- `git -C vendor/zembra-schema describe --tags --exact-match HEAD`
- `git status --short`
- `cargo fmt --check`
- `cargo check`
