# r014 Random Fields API 需求澄清

日期：2026-05-16

## 背景

在 `GET /random/tags` 的随机发现体验基础上，需要新增一个同风格的 field 随机接口。客户端可以随机抽取若干 field，并从这些 field 的所有可见笔记中累计返回指定数量的 notes。

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| API 路径 | 新增 `GET /random/fields` |
| 请求参数 | Query 参数使用 `n` 表示随机 field 数量，使用 `count` 表示累计 note 数量 |
| `n` 范围 | `n` 必须在 `1..=20` |
| `n` 默认值 | 不传 `n` 时默认 3 |
| `count` 范围 | `count` 必须在 `1..=100` |
| `count` 默认值 | 不传 `count` 时默认 20 |
| 随机对象 | 从已有 fields 中随机抽取 `n` 个 field |
| field 数不足 | 当可用 field 数量小于 `n` 时，有多少返回多少，不报错 |
| notes 范围 | 从随机 fields 下所有未删除、未归档 notes 中累计返回最多 `count` 条 |
| 过滤规则 | 过滤 `deleted_at IS NOT NULL` 和 `archived_at IS NOT NULL` 的 notes |
| 响应结构 | 按 field 分组返回，顶级字段为 `field_notes` |
| 空 field | 随机抽中的 field 没有可见 notes 时仍返回该 field，`notes` 为空数组 |
| 随机策略 | 每次请求实时随机，不需要 seed，不要求结果可复现 |
| 使用目标 | 提供与 random tags 风格一致的 field-based note discovery API |

## 推荐响应形态

```json
{
  "field_notes": [
    {
      "field": {
        "id": "field-id",
        "name": "work"
      },
      "notes": []
    }
  ]
}
```

## 非目标

- 不做可复现随机，不支持 seed。
- 不做分页、游标或总数统计。
- 不修改现有 `/fields`、`/tags`、`/notes`、`/notes/recent`、`/random/tags` 行为。
- 不返回软删除或归档 notes。
- 不新增数据库 schema。

## 验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `GET /random/fields?n=3&count=20` 返回顶级字段 `field_notes` |
| A2 | `n` 在 `1..=20` 内时请求成功 |
| A3 | `n` 小于 1 或大于 20 时返回 validation error |
| A4 | `count` 在 `1..=100` 内时请求成功 |
| A5 | `count` 小于 1 或大于 100 时返回 validation error |
| A6 | 可用 field 数小于 `n` 时返回现有 field 数量，不报错 |
| A7 | 所有分组内 notes 累计数量不超过 `count` |
| A8 | 返回 notes 不包含软删除笔记 |
| A9 | 返回 notes 不包含归档笔记 |
| A10 | 抽中的 field 没有可见 notes 时仍返回该 field，`notes` 为空数组 |
| A11 | OpenAPI 文档包含 `/random/fields` path、query 参数、response 和错误响应 |
