# r020 Notes By Date API 设计文档

日期：2026-05-22

关联需求澄清：`docs/request-clarify/r020-notes-by-date-api.md`

## 核心功能（WHAT）

新增 `GET /notes/by-date?date=YYYY-MM-DD`，返回服务器本地日期口径下，当天创建的所有可见笔记。接口只读取默认 workspace 下 `deleted_at IS NULL` 且 `archived_at IS NULL` 的 notes。

### 需求背景（WHY）

前端已有每日创建数统计后，需要进一步点击或选择某一天查看当天创建的笔记明细。当前 notes 模块已有按更新时间列表、recent、random 和每日 count 聚合，缺少按创建日期直接返回笔记列表的查询能力。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 新增单日笔记查询接口 | 提供 `GET /notes/by-date?date=YYYY-MM-DD` |
| 标准日期参数 | `date` 必填，只接受 `YYYY-MM-DD` |
| 本地日期口径 | 与 `GET /notes/stats/daily-counts` 保持一致，按服务器本地时区解释日期 |
| 可见笔记口径 | 固定过滤 `deleted_at IS NULL` 与 `archived_at IS NULL` |
| 稳定排序 | 按 `created_at DESC, id DESC` 返回 |
| 同步 API 合同 | 补齐 DTO、handler OpenAPI 标注、`ApiDoc` 注册和测试 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| DTO | 新增 query DTO 和响应 DTO，派生 `Deserialize`、`Validate`、`IntoParams`、`Serialize`、`ToSchema` |
| Repository | 新增按创建时间戳闭区间起点和开区间终点查询可见 notes 的方法 |
| Service | 校验并解析 `YYYY-MM-DD`，计算服务器本地日开始和下一日开始时间戳 |
| Handler | 新增 `GET /notes/by-date` handler |
| Routes | 在 notes router 注册新路由，放在 `/{note_ref}` 动态路由之前 |
| OpenAPI | 注册 path、query 参数、response schema 和错误响应 |
| Tests | 覆盖成功查询、空结果、可见性过滤、非法日期和 OpenAPI path |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 分页 | 本轮返回当天全部可见笔记，不新增 limit/cursor |
| 日期区间 | 不支持 from/to |
| 多维度筛选 | 不支持 tag、field、role 过滤 |
| Schema 变更 | 不新增表、索引或迁移 |
| 前端页面 | 不实现 UI |

## 实现流程（HOW）

### 架构落点

| 层级 | 文件/模块 | 设计 |
| --- | --- | --- |
| DTO | `src/dto/notes.rs` | 新增 `NotesByDateQuery` 和 `NotesByDateResponse` |
| Repository | `src/repositories/notes.rs` | 新增 `list_visible_notes_created_between(start_timestamp, end_timestamp)` |
| Service | `src/services/notes.rs` | 新增 `notes_by_date(query)`，复用本地日期转 timestamp 的逻辑 |
| Handler | `src/handlers/notes.rs` | 新增 `notes_by_date` handler，校验 query |
| Routes | `src/routes/notes.rs` | 注册 `GET /notes/by-date` |
| OpenAPI | `src/api_doc.rs` | 注册 handler path 和 response schema |
| Tests | `src/app.rs` | 增加 API 行为测试和 OpenAPI path 断言 |

### API 合同

| 项目 | 内容 |
| --- | --- |
| Method | `GET` |
| Path | `/notes/by-date` |
| Query | `date=YYYY-MM-DD` |
| Response Body | `NotesByDateResponse` |
| Tag | `notes` |

响应体：

```json
{
  "date": "2026-05-22",
  "notes": []
}
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `date` | string | 请求日期，格式 `YYYY-MM-DD` |
| `notes` | array | 当天创建的可见 `NoteRecord` 列表 |

### 查询规则

| 规则 | 设计 |
| --- | --- |
| 日期解析 | service 使用 `NaiveDate::parse_from_str(date, "%Y-%m-%d")` |
| 开始时间 | `local_start_of_day_timestamp(date)` |
| 结束时间 | `local_start_of_day_timestamp(date + 1 day)` |
| SQL 条件 | `created_at >= start_timestamp AND created_at < end_timestamp` |
| 可见性过滤 | 固定过滤 `deleted_at IS NULL` 与 `archived_at IS NULL` |
| workspace | 继续使用 `DEFAULT_WORKSPACE_ID` |
| 排序 | `ORDER BY created_at DESC, id DESC` |

### 错误响应

| 场景 | HTTP 状态 | code |
| --- | --- | --- |
| 缺少 `date` | `422` | `validation_error` |
| `date` 不是 `YYYY-MM-DD` | `422` | `validation_error` |
| SQLite 访问失败 | `500` | `database_error` |

## 测试用例

### 编译检查

| 用例 | 预期 |
| --- | --- |
| `cargo fmt --check` | 格式检查通过 |
| `cargo check` | 类型检查通过 |
| `cargo test` | 单元测试和集成测试通过 |
| `cargo clippy -- -D warnings` | 无 warning |

### 自动化行为检查

| 用例 | 预期 |
| --- | --- |
| 指定日期有笔记 | 返回该日期创建的所有可见笔记 |
| 指定日期无笔记 | 返回 `notes: []` |
| 排序 | 结果按 `created_at DESC, id DESC` |
| 可见性过滤 | 已删除、已归档笔记不返回 |
| 非法日期 | `date=2026-13-40` 返回 `422` |
| 缺少日期 | `/notes/by-date` 返回 `422` |
| OpenAPI | `/api-docs/openapi.json` 包含 `/notes/by-date` |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| curl 调用单日 API | 返回 `200 OK` 和 `{ "date": "...", "notes": [...] }` |
| Swagger UI 查看 | notes tag 下能看到 `GET /notes/by-date` |
