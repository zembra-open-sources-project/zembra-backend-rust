# r006 Recent Notes API 设计文档

日期：2026-05-03

关联需求澄清：`docs/request-clarify/r006-recent-notes-api.md`

## 核心功能（WHAT）

新增 `POST /notes/recent`，用于 Web 前端获取最近笔记内容。接口通过 JSON body 传入可选 `limit`，默认返回最近 50 条，最多返回 100 条，并按 `updated_at DESC` 从新到旧返回未删除、未归档笔记。

### 需求背景（WHY）

Web 前端需要一个面向首页或最近内容列表的专用读取接口。当前 `GET /notes?limit=` 已能按更新时间倒序返回未删除笔记，但它使用 query 参数，并且会包含归档笔记；新接口需要使用 POST body 传参，并排除归档内容。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 新增 Web 查询接口 | 提供 `POST /notes/recent` 给前端调用 |
| 支持 body 传参 | 请求体支持可选 `limit` 字段 |
| 固定默认行为 | 未传 `limit` 时默认返回 50 条 |
| 限制查询规模 | `limit` 合法范围为 1 到 100 |
| 保持倒序展示 | 按 `updated_at DESC` 返回最近更新的笔记 |
| 过滤不可展示内容 | 不返回软删除和归档笔记 |
| 同步 API 合同 | 补齐 DTO、handler OpenAPI 标注和 `ApiDoc` 注册 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| DTO | 新增 recent notes request body，包含 `limit` 校验 |
| Repository | 新增或扩展查询方法，过滤 `deleted_at` 和 `archived_at` |
| Service | 暴露 recent notes 查询语义，默认 limit 由请求 DTO 或 service 统一处理 |
| Handler | 新增 `POST /notes/recent`，处理 JSON body、校验和响应 |
| Routes | 注册 `/notes/recent` 路由 |
| OpenAPI | 注册 path 和 request schema，声明成功、JSON 错误、validation、database error |
| Tests | 覆盖默认 limit、传参 limit、排序、归档过滤、软删除过滤和 validation error |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 修改现有列表接口 | 不改变 `GET /notes` 的返回规则 |
| 分页能力 | 不实现 cursor、offset 或总数统计 |
| 认证授权 | 不新增鉴权逻辑 |
| 前端页面 | 不实现 Web UI |
| 数据库 schema | 不新增表或字段 |

## 实现流程（HOW）

### 架构落点

| 层级 | 文件/模块 | 设计 |
| --- | --- | --- |
| DTO | `src/dto/notes.rs` | 新增 `RecentNotesRequest`，派生 `Deserialize`、`Validate`、`ToSchema` |
| Repository | `src/repositories/notes.rs` | 新增 `list_recent_notes(limit)`，SQL 过滤 `deleted_at IS NULL` 和 `archived_at IS NULL` |
| Service | `src/services/notes.rs` | 新增 `recent_notes(limit)`，调用 repository 并返回 `Vec<NoteRecord>` |
| Handler | `src/handlers/notes.rs` | 新增 `recent_notes` handler，解析 JSON body 并返回 `ListNotesResponse` |
| Routes | `src/routes/notes.rs` | 注册 `POST /notes/recent` |
| OpenAPI | `src/api_doc.rs` | 注册 handler path 和 `RecentNotesRequest` schema |

### API 合同

| 项目 | 内容 |
| --- | --- |
| Method | `POST` |
| Path | `/notes/recent` |
| Request Body | `RecentNotesRequest` |
| Response Body | `ListNotesResponse` |
| Tag | `notes` |

请求体：

```json
{
  "limit": 50
}
```

字段说明：

| 字段 | 类型 | 必填 | 默认值 | 约束 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `limit` | integer | 否 | 50 | 1 到 100 | 最大返回笔记条数 |

响应体复用：

```json
{
  "notes": []
}
```

### 查询规则

| 规则 | 设计 |
| --- | --- |
| 默认数量 | `limit` 为 `None` 时使用 50 |
| 数量范围 | 小于 1 或大于 100 返回 `422 validation_error` |
| 排序 | `ORDER BY updated_at DESC` |
| 软删除过滤 | `deleted_at IS NULL` |
| 归档过滤 | `archived_at IS NULL` |
| 返回模型 | 直接返回 `NoteRecord` 列表 |

### 错误响应

| 场景 | HTTP 状态 | code |
| --- | --- | --- |
| JSON 格式错误 | `400` | `invalid_json` |
| `limit` 校验失败 | `422` | `validation_error` |
| SQLite 访问失败 | `500` | `database_error` |

## 测试用例

### 编译检查

| 用例 | 预期 |
| --- | --- |
| `cargo fmt --check` | 格式检查通过 |
| `cargo check` | 类型检查通过 |
| `cargo test` | 单元测试和集成测试通过 |
| `cargo clippy` | 无新增 warning |

### 自动化行为检查

| 用例 | 预期 |
| --- | --- |
| 默认 limit | `POST /notes/recent` body 为 `{}` 时最多返回 50 条 |
| 自定义 limit | 传入 `{"limit": 2}` 时最多返回 2 条 |
| 排序 | 多条 note 按 `updated_at DESC` 返回 |
| 软删除过滤 | 软删除 note 不出现在响应中 |
| 归档过滤 | 已归档 note 不出现在响应中 |
| limit 下界 | `limit = 0` 返回 `422 validation_error` |
| limit 上界 | `limit = 101` 返回 `422 validation_error` |
| OpenAPI | `/api-docs/openapi.json` 包含 `/notes/recent` |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| curl 调用 recent API | 返回 `200 OK` 和 `{ "notes": [...] }` |
| Swagger UI 查看 | notes tag 下能看到 `POST /notes/recent` |
