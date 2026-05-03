# r005 OpenAPI 运行时文档执行计划

日期：2026-05-03

需求澄清文档：`docs/request-clarify/r005-openapi-runtime-docs.md`
设计文档：`docs/design-docs/r005-openapi-runtime-docs.md`

## Stage 1：OpenAPI 运行时入口

| Task | 状态 | 功能 | 实现要点 | 预期测试结果 |
| --- | --- | --- | --- | --- |
| T1 | Finished | 定义 OpenAPI 聚合文档 | 新增 `src/api_doc.rs`，聚合 handler paths 和 schemas | OpenAPI JSON 可生成 |
| T2 | Finished | 标注 handler | 为 health、notes、taxonomy handler 增加 `#[utoipa::path]` | JSON paths 覆盖现有 API |
| T3 | Finished | 注册文档路由 | 在 app router 注册 `/api-docs/openapi.json` 和 `/swagger-ui` | 运行时可访问文档入口 |

## Stage 2：仓库约束和验证

| Task | 状态 | 功能 | 实现要点 | 预期测试结果 |
| --- | --- | --- | --- | --- |
| T1 | Finished | 更新 AGENTS 规则 | 增加 handler/OpenAPI 同步维护约束 | 后续 agent 有明确规则 |
| T2 | Finished | 增加测试 | 验证 OpenAPI JSON 和 Swagger UI 可访问 | `cargo test` 通过 |
| T3 | Finished | 完整验证 | 运行 fmt/check/test/clippy 并记录结果 | 四项验证通过 |

## 执行记录

- 2026-05-03：确认当前仅有 `ToSchema`/`IntoParams`，未暴露运行时 OpenAPI JSON 和 Swagger UI，开始实现 r005。
- 2026-05-03：完成 OpenAPI 聚合文档、handler 标注、Swagger UI 注册和 AGENTS OpenAPI 维护规则。
- 2026-05-03：新增测试验证 `/api-docs/openapi.json` 包含关键 path，`/swagger-ui/` 可访问，`cargo test` 已通过 13 个测试。
- 2026-05-03：完整验证通过：`cargo fmt --check`、`cargo check`、`cargo test`（13 passed）、`cargo clippy`。
