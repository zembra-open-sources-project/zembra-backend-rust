# r015 Random Notes API 需求澄清

日期：2026-05-16

## 背景

在 random tags 和 random fields 的探索接口基础上，需要新增一个更直接的随机笔记接口。客户端通过指定数量 `n`，直接随机获取未删除、未归档的 notes。

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| API 路径 | 新增 `GET /random/notes` |
| 请求参数 | Query 参数使用 `n` 表示随机 note 数量 |
| `n` 是否必填 | 必填 |
| `n` 范围 | `n` 必须在 `1..=50` |
| 随机对象 | 从未删除、未归档 notes 中随机抽取 `n` 条 |
| notes 不足 | 当可用 notes 数量小于 `n` 时，有多少返回多少，不报错 |
| 无笔记 | 没有可用 notes 时返回空数组 |
| 响应结构 | 复用 notes 列表结构，顶级字段为 `notes` |
| 随机策略 | 每次请求实时随机，不需要 seed，不要求结果可复现 |

## 响应形态

```json
{
  "notes": []
}
```

## 非目标

- 不做可复现随机，不支持 seed。
- 不做分页、游标或总数统计。
- 不返回软删除或归档 notes。
- 不修改现有 `/notes`、`/notes/recent`、`/random/tags`、`/random/fields` 行为。
- 不新增数据库 schema。

## 验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `GET /random/notes?n=5` 返回顶级字段 `notes` |
| A2 | `n` 在 `1..=50` 内时请求成功 |
| A3 | `n` 小于 1 或大于 50 时返回 validation error |
| A4 | 可用 notes 数小于 `n` 时返回现有 notes 数量，不报错 |
| A5 | 没有可用 notes 时返回 `notes: []` |
| A6 | 返回 notes 不包含软删除笔记 |
| A7 | 返回 notes 不包含归档笔记 |
| A8 | OpenAPI 文档包含 `/random/notes` path、query 参数、response 和错误响应 |
