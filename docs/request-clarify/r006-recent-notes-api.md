# r006 Recent Notes API 需求澄清

日期：2026-05-03

## 背景

Web 前端需要一个专用 API 展示最近笔记内容。现有 `GET /notes?limit=` 已支持按更新时间倒序列出笔记，但新需求要求通过 POST 请求体传参，并且最近笔记列表不返回归档笔记。

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| API 路径 | 新增 `POST /notes/recent` |
| 请求体 | JSON body，字段为 `limit` |
| 默认条数 | 不传 `limit` 时默认返回 50 条 |
| 条数范围 | `limit` 最小 1，最大 100 |
| 排序规则 | 按 `updated_at DESC` 从新到旧返回 |
| 删除过滤 | 不返回软删除笔记，即过滤 `deleted_at IS NOT NULL` |
| 归档过滤 | 不返回归档笔记，即过滤 `archived_at IS NOT NULL` |
| 响应结构 | 复用现有列表响应结构 `{ "notes": [...] }` |
| 使用场景 | Web 前端最近笔记展示 |

## 非目标

- 不改动现有 `GET /notes` 的行为。
- 不新增分页游标或 offset 分页。
- 不调整 note 创建、更新、归档、删除接口语义。

## 验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `POST /notes/recent` 不传 `limit` 时返回最多 50 条最近笔记 |
| A2 | `POST /notes/recent` 传入合法 `limit` 时按该数量限制返回 |
| A3 | 返回顺序为 `updated_at DESC` |
| A4 | 响应不包含软删除笔记 |
| A5 | 响应不包含归档笔记 |
| A6 | `limit` 小于 1 或大于 100 时返回 validation error |
| A7 | OpenAPI 文档包含该 path、request body、response 和错误响应 |
