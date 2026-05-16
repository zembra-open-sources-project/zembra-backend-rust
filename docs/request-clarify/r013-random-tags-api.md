# r013 Random Tags API 需求澄清

日期：2026-05-16

## 背景

需要新增一个偏探索和发现体验的读取接口，让客户端可以通过随机 tag 找到一组相关笔记。接口核心不是稳定翻页或精确检索，而是每次请求都尽量产生随机结果。

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| API 路径 | 新增 `GET /random/tags` |
| 请求参数 | Query 参数使用 `n` 表示随机 tag 数量，使用 `count` 表示累计 note 数量 |
| `n` 范围 | `n` 必须在 `1..=20` |
| `n` 默认值 | 不传 `n` 时默认 3 |
| `count` 范围 | `count` 必须在 `1..=100` |
| `count` 默认值 | 不传 `count` 时默认 20 |
| 随机对象 | 从已有 tags 中随机抽取 `n` 个 tag |
| tag 数不足 | 当可用 tag 数量小于 `n` 时，有多少返回多少，不报错 |
| notes 范围 | 从随机 tags 下所有未删除、未归档 notes 中累计返回最多 `count` 条 |
| 过滤规则 | 过滤 `deleted_at IS NOT NULL` 和 `archived_at IS NOT NULL` 的 notes |
| 响应结构 | 按 tag 分组返回，顶级字段为 `tagged_notes` |
| 重复 note | 同一篇 note 命中多个随机 tag 时，允许出现在多个 tag 分组中 |
| 随机策略 | 每次请求实时随机，不需要 seed，不要求结果可复现 |
| 使用目标 | 提供足够随机的 tag-based note discovery API |

## 推荐响应形态

```json
{
  "tagged_notes": [
    {
      "tag": {
        "id": "tag-id",
        "name": "rust"
      },
      "notes": []
    }
  ]
}
```

## 非目标

- 不做可复现随机，不支持 seed。
- 不做分页、游标或总数统计。
- 不去重跨 tag 分组重复出现的 note。
- 不修改现有 `/tags`、`/notes`、`/notes/recent` 行为。
- 不返回软删除或归档 notes。

## 验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `GET /random/tags?n=3` 返回顶级字段 `tagged_notes` |
| A2 | `n` 在 `1..=20` 内时请求成功 |
| A3 | `n` 小于 1 或大于 20 时返回 validation error |
| A4 | `count` 在 `1..=100` 内时请求成功 |
| A5 | `count` 小于 1 或大于 100 时返回 validation error |
| A6 | 可用 tag 数小于 `n` 时返回现有 tag 数量，不报错 |
| A7 | 所有分组内 notes 累计数量不超过 `count` |
| A8 | 每个结果项包含 tag 信息和该 tag 下的 notes |
| A9 | 返回 notes 不包含软删除笔记 |
| A10 | 返回 notes 不包含归档笔记 |
| A11 | 同一 note 关联多个被抽中的 tag 时，可在多个分组中出现，但整体累计仍不超过 `count` |
| A12 | 多次请求不要求结果稳定，随机 tag 每次实时抽取 |
| A13 | OpenAPI 文档包含 `/random/tags` path、query 参数、response 和错误响应 |
