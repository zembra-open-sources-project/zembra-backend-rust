# r003 共享数据库 Schema 引入执行计划

日期：2026-04-27

需求澄清文档：`docs/request-clarify/r003-shared-schema-submodule.md`
设计文档：`docs/design-docs/r003-shared-schema-submodule.md`

## Stage 1 Schema Submodule 接入

| Task | 状态 | 功能 | 实现要点 | 预期测试结果 |
| --- | --- | --- | --- | --- |
| T1 | Finished | 引入共享 schema submodule | 新增 `.gitmodules`，将 `zembra-schema` 放到 `vendor/zembra-schema` | submodule 路径和 URL 正确 |
| T2 | Finished | 固定 schema 版本 | 将 submodule HEAD 固定到 `v0.2.0` | `git describe --tags --exact-match HEAD` 输出 `v0.2.0` |
| T3 | Finished | 对齐配置样例 | 将 `.env.example` 调整为 `database.path` 对应的 TOML 结构 | 配置样例和当前 `Settings` 结构一致 |

## 进度记录

- 2026-04-27：完成需求澄清与设计，开始引入共享 schema submodule。
- 2026-04-27：已新增 `vendor/zembra-schema` submodule，并固定到 tag `v0.2.0`。
- 2026-04-27：已将 `.env.example` 调整为当前 `~/.zembra.env` 使用的 TOML 配置结构。
- 2026-04-27：已通过 `git -C vendor/zembra-schema describe --tags --exact-match HEAD`、`cargo fmt --check`、`cargo check`、`cargo test` 和 `cargo clippy` 验证，等待用户验收。
