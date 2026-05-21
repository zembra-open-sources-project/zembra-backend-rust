# r018 Daily Note Counts API 设计文档

日期：2026-05-21

关联需求澄清：`docs/request-clarify/r018-daily-note-counts-api.md`

## 核心功能（WHAT）

新增 `GET /notes/stats/daily-counts`，返回服务器本地日期口径下，过去 30 天每天创建的可见笔记数量。响应固定包含 30 个日期桶，没有数据的日期返回 `0`。

### 需求背景（WHY）

前端需要一个轻量统计接口直接绘制最近 30 天笔记创建趋势。当前 notes 模块已有 CRUD、recent 和 random 查询接口，但没有按日期聚合的统计能力。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 新增统计接口 | 提供 `GET /notes/stats/daily-counts` |
| 固定统计窗口 | 统计过去 30 天，包含今天 |
| 本地日期口径 | 日期按服务器本地时区生成 |
| 可见笔记口径 | 只统计 `deleted_at IS NULL` 且 `archived_at IS NULL` 的笔记 |
| 补齐空日期 | 日期区间内无笔记时返回 `count = 0` |
| 同步 API 合同 | 补齐 DTO、handler OpenAPI 标注和 `ApiDoc` 注册 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| DTO | 新增每日统计项和响应结构，派生 `Serialize`、`ToSchema` |
| Repository | 查询过去 30 天可见笔记，按服务器本地日期聚合 |
| Service | 生成完整 30 天日期序列，合并 repository 聚合结果 |
| Handler | 新增 `GET /notes/stats/daily-counts` |
| Routes | 在 notes router 注册统计路由 |
| OpenAPI | 注册 path 和 response schema |
| Tests | 覆盖 30 天窗口、空日期、过滤删除归档、OpenAPI path |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 可配置时间窗口 | 不新增 `days`、`from`、`to` 参数 |
| 多维度分组 | 不按 tag、field、role 分组 |
| Schema 变更 | 不新增表、索引或迁移 |
| 前端页面 | 不实现图表 UI |

## 实现流程（HOW）

### 架构落点

| 层级 | 文件/模块 | 设计 |
| --- | --- | --- |
| DTO | `src/dto/notes.rs` | 新增 `DailyNoteCount` 和 `DailyNoteCountsResponse` |
| Repository | `src/repositories/notes.rs` | 新增 `daily_note_counts_since(start_timestamp)`，SQL 使用 SQLite `date(created_at, 'unixepoch', 'localtime')` 聚合 |
| Service | `src/services/notes.rs` | 新增 `daily_note_counts()`，用服务器本地时间计算 30 天日期桶并补 0 |
| Handler | `src/handlers/notes.rs` | 新增 `daily_note_counts` handler |
| Routes | `src/routes/notes.rs` | 注册 `GET /notes/stats/daily-counts` |
| OpenAPI | `src/api_doc.rs` | 注册 handler path 和 response schema |

### API 合同

| 项目 | 内容 |
| --- | --- |
| Method | `GET` |
| Path | `/notes/stats/daily-counts` |
| Request | 无 query 参数、无 request body |
| Response Body | `DailyNoteCountsResponse` |
| Tag | `notes` |

响应体：

```json
{
  "days": [
    {
      "date": "2026-04-22",
      "count": 3
    }
  ]
}
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `days` | array | 固定 30 条每日统计 |
| `days[].date` | string | 服务器本地日期，格式 `YYYY-MM-DD` |
| `days[].count` | integer | 当天创建的可见笔记数量 |

### 查询规则

| 规则 | 设计 |
| --- | --- |
| 时间窗口 | 从本地今天往前 29 天开始，到今天结束 |
| 日期转换 | SQLite 聚合使用 `date(created_at, 'unixepoch', 'localtime')` |
| 开始时间 | service 计算窗口首日本地 00:00:00 对应 timestamp，repository 查询 `created_at >= start_timestamp` |
| 可见性过滤 | 固定过滤 `deleted_at IS NULL` 与 `archived_at IS NULL` |
| workspace | 继续使用 `DEFAULT_WORKSPACE_ID` |
| 排序 | service 按生成日期自然升序返回 |

### 错误响应

| 场景 | HTTP 状态 | code |
| --- | --- | --- |
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
| 30 天窗口 | 响应固定返回 30 条 |
| 日期格式 | 每条 `date` 为 `YYYY-MM-DD` |
| 空日期补 0 | 无笔记日期返回 `count = 0` |
| 创建数统计 | 同一天多条笔记被正确累计 |
| 可见性过滤 | 已删除、已归档笔记不计入数量 |
| OpenAPI | `/api-docs/openapi.json` 包含 `/notes/stats/daily-counts` |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| curl 调用统计 API | 返回 `200 OK` 和 `{ "days": [...] }` |
| Swagger UI 查看 | notes tag 下能看到 `GET /notes/stats/daily-counts` |

