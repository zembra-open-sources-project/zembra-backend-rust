# r020 Notes By Date API 需求澄清

日期：2026-05-22

## 背景

前端需要按某个具体日期获取当天创建的所有可见笔记。当前后端已有 notes CRUD、recent、random 查询，以及 `GET /notes/stats/daily-counts` 每日创建数量统计，但还没有按单日返回笔记列表的接口。

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| API 路径 | 新增 `GET /notes/by-date?date=YYYY-MM-DD` |
| 请求参数 | Query 参数 `date` 必填 |
| 日期格式 | `YYYY-MM-DD`，标准化日期，具体到天 |
| 日期口径 | 沿用 `daily-counts`，按服务器本地时区解释日期 |
| 查询字段 | 使用 `notes.created_at` 判断笔记创建日期 |
| 笔记范围 | 只返回未删除、未归档笔记 |
| workspace | 沿用默认 workspace |
| 排序 | 按 `created_at DESC, id DESC` 返回，当天较新创建的笔记排前面 |
| 响应结构 | 返回 `{ "date": "YYYY-MM-DD", "notes": [...] }` |
| 非法日期 | 返回现有统一 `422 validation_error` |

## 非目标

- 不支持日期区间查询。
- 不新增分页参数。
- 不按 tag、field、role 分组。
- 不返回已删除或已归档笔记。
- 不新增数据库表、索引、缓存或后台任务。
- 不修改现有 `/notes`、`/notes/recent`、`/notes/stats/daily-counts`、`/random/*` 接口行为。

## 验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `GET /notes/by-date?date=YYYY-MM-DD` 返回 `200 OK` |
| A2 | 响应顶级字段包含请求日期 `date` 和笔记列表 `notes` |
| A3 | 只返回 `created_at` 落在该服务器本地日期内的笔记 |
| A4 | 同一天多条笔记按 `created_at DESC, id DESC` 排序 |
| A5 | 没有匹配笔记时返回空数组 |
| A6 | 已删除、已归档笔记不出现在结果中 |
| A7 | 日期缺失或格式非法时返回 `422 validation_error` |
| A8 | OpenAPI 文档包含该 path、query 参数、response 和错误响应 |
