# r005 OpenAPI 运行时文档需求澄清

日期：2026-05-03

## 背景

当前后端已实现 CRUD HTTP API，并引入 `utoipa` 与 `utoipa-swagger-ui` 依赖，部分 DTO 和模型已经派生 `ToSchema`。但服务运行时尚未暴露机器可读 OpenAPI JSON，也没有 Swagger UI，其他仓库的 agent 无法动态读取真实 API 合同来开发 client。

## 已确认目标

| 目标 | 说明 |
| --- | --- |
| OpenAPI JSON | 启动后端后暴露 `GET /api-docs/openapi.json` |
| Swagger UI | 启动后端后暴露 `/swagger-ui` |
| Handler 约束 | 在 `AGENTS.md` 中要求新增或修改 handler 时同步维护 OpenAPI 标注 |
| Client 协作 | 其他仓库 agent 可从运行中的后端动态读取 OpenAPI JSON，不再手抄 API 文档 |

## 范围边界

| 范围 | 结论 |
| --- | --- |
| API 行为 | 不改变现有 CRUD API 行为 |
| 文档生成 | 使用现有 `utoipa` / `utoipa-swagger-ui` |
| 静态文档 | `docs/http-client-server-api.md` 保留为需求说明，运行时合同以 OpenAPI JSON 为准 |
| Client 实现 | 本需求不实现其他仓库 client |

## 验收标准

| 编号 | 标准 |
| --- | --- |
| A1 | `GET /api-docs/openapi.json` 返回 `200 OK` |
| A2 | OpenAPI JSON 包含 `/notes`、`/notes/batch`、`/fields`、`/tags`、`/health` |
| A3 | `/swagger-ui` 可访问 Swagger UI |
| A4 | `AGENTS.md` 明确 handler 与 OpenAPI 标注同步约束 |
| A5 | `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy` 通过 |
