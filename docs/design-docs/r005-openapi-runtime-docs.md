# r005 OpenAPI 运行时文档设计

日期：2026-05-03

需求澄清文档：`docs/request-clarify/r005-openapi-runtime-docs.md`

## 设计目标

让后端在运行时提供标准 OpenAPI 合同和 Swagger UI，供人类和其他仓库 agent 动态查看 API。后续新增或修改 handler 时，必须同步维护 OpenAPI 标注，避免代码行为和 API 合同漂移。

## 实现方案

| 模块 | 设计 |
| --- | --- |
| `src/api_doc.rs` | 定义 `ApiDoc`，使用 `#[derive(OpenApi)]` 聚合 paths、schemas 和 tags |
| handlers | 给每个 HTTP handler 增加 `#[utoipa::path]`，声明 method、path、params、request_body、responses |
| DTO/model | 复用现有 `ToSchema`，缺失的 query 参数补 `IntoParams` |
| router | 在 `build_router` 中 merge `SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi())` |
| AGENTS | 增加 OpenAPI 维护规则，约束 handler 变更必须同步更新标注和验证 |

## API 文档入口

| Path | 用途 |
| --- | --- |
| `/api-docs/openapi.json` | 机器可读 OpenAPI JSON，供 agent 和 client generator 使用 |
| `/swagger-ui` | 人类可视化浏览和调试 API |

## 其他仓库 agent 使用方式

其他仓库 agent 应以运行中的后端 OpenAPI JSON 为准：

```text
启动 zembra-backend-rust 后，从 http://127.0.0.1:3000/api-docs/openapi.json 拉取最新接口定义。
开发 client 时根据 OpenAPI JSON 校验 method、path、request body、response schema 和错误响应。
人类调试可访问 http://127.0.0.1:3000/swagger-ui。
```

## 测试用例

| 用例 | 预期 |
| --- | --- |
| `GET /api-docs/openapi.json` | 返回 `200 OK`，包含 OpenAPI JSON |
| OpenAPI paths 检查 | JSON 中包含 CRUD 关键 paths |
| `GET /swagger-ui` | 返回可访问响应 |
| 编译验证 | `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy` 通过 |
