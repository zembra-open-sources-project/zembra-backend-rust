# r013 Random Tags API 需求澄清

日期：2026-05-16

## 背景

需要新增一个偏探索和发现体验的读取接口，让客户端可以通过随机 tag 找到一组相关笔记。接口核心不是稳定翻页或精确检索，而是每次请求都尽量产生随机结果。

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| API 路径 | 新增 `GET /random/tags` |
| 请求参数 | Query 参数使用 `n` 表示随机 tag 数量 |
| 数量范围 | `n` 必须在 `1..=20` |
| 随机对象 | 从已有 tags 中随机抽取 `n` 个 tag |
| tag 数不足 | 当可用 tag 数量小于 `n` 时，有多少返回多少，不报错 |
| notes 范围 | 返回每个随机 tag 下关联的所有未删除、未归档 notes |
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
| A4 | 可用 tag 数小于 `n` 时返回现有 tag 数量，不报错 |
| A5 | 每个结果项包含 tag 信息和该 tag 下的 notes |
| A6 | 返回 notes 不包含软删除笔记 |
| A7 | 返回 notes 不包含归档笔记 |
| A8 | 同一 note 关联多个被抽中的 tag 时，可在多个分组中出现 |
| A9 | 多次请求不要求结果稳定，随机 tag 每次实时抽取 |
| A10 | OpenAPI 文档包含 `/random/tags` path、query 参数、response 和错误响应 |
