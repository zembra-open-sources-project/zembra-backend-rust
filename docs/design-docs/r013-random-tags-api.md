# r013 Random Tags API 设计文档

日期：2026-05-16

关联需求澄清：`docs/request-clarify/r013-random-tags-api.md`

## 核心功能（WHAT）

新增 `GET /random/tags`，通过 query 参数 `n` 指定随机 tag 数量。接口每次请求从现有 tags 中实时随机抽取 `n` 个 tag，并按 tag 分组返回这些 tag 下所有未删除、未归档 notes。顶级响应字段固定为 `tagged_notes`。

### 需求背景（WHY）

当前后端已经提供 notes CRUD、recent notes 和 taxonomy tags 查询，但缺少一个面向探索体验的随机发现接口。这个接口的目标是让客户端可以通过随机 tag 发现笔记内容，结果不需要稳定复现，重点是足够随机和直接可用。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 新增随机发现接口 | 提供 `GET /random/tags?n=N` |
| 限制随机规模 | `n` 合法范围为 1 到 20 |
| tag 不足兼容 | 可用 tag 少于 `n` 时返回现有数量 |
| 按 tag 分组 | 顶级字段为 `tagged_notes`，每项包含 tag 和 notes |
| 过滤不可展示内容 | notes 只返回未删除、未归档记录 |
| 保持随机性 | 每次请求实时随机，不支持 seed，不保证可复现 |
| 同步 API 合同 | 补齐 DTO、handler OpenAPI 标注和 `ApiDoc` 注册 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| DTO | 新增 random tags query 和响应 DTO，包含 `n` 校验、tag 分组响应 |
| Repository | 新增随机 tag 查询和按 tag 查询可见 notes 的读取方法 |
| Service | 组装随机 tag 和对应 notes，返回 `tagged_notes` |
| Handler | 新增 `GET /random/tags` handler，解析 query、校验并返回响应 |
| Routes | 注册 `/random/tags` 路由 |
| OpenAPI | 注册 path、query 参数、response schema 和错误响应 |
| Tests | 覆盖参数校验、tag 不足、软删除过滤、归档过滤、重复 note 分组和 OpenAPI 暴露 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 可复现随机 | 不支持 seed，不保证多次请求结果一致 |
| 分页能力 | 不实现分页、游标或总数统计 |
| 跨 tag 去重 | 同一 note 可在多个 tag 分组重复出现 |
| 修改现有接口 | 不改变 `/tags`、`/notes`、`/notes/recent` 行为 |
| 数据库 schema | 不新增表或字段 |
| 认证授权 | 不新增鉴权逻辑 |

## 实现流程（HOW）

### 架构落点

| 层级 | 文件/模块 | 设计 |
| --- | --- | --- |
| DTO | `src/dto/notes.rs` | 新增 `RandomTagsQuery`、`TaggedNotesResponse`、`TaggedNotesGroup` |
| Repository | `src/repositories/notes.rs` | 新增随机 tag 读取和按 tag ID 查询可见 notes 方法 |
| Service | `src/services/notes.rs` | 新增 `random_tagged_notes(query)`，处理默认值和分组组装 |
| Handler | `src/handlers/notes.rs` | 新增 `random_tagged_notes` handler，解析 `Query<RandomTagsQuery>` |
| Routes | `src/routes/notes.rs` | 注册 `GET /random/tags` |
| OpenAPI | `src/api_doc.rs` | 注册 handler path 和新增 response schema |

### API 合同

| 项目 | 内容 |
| --- | --- |
| Method | `GET` |
| Path | `/random/tags` |
| Query | `n` |
| Response Body | `TaggedNotesResponse` |
| Tag | `notes` |

Query 参数：

| 字段 | 类型 | 必填 | 默认值 | 约束 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `n` | integer | 否 | 3 | 1 到 20 | 随机抽取的 tag 数量 |

响应体：

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

响应字段：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `tagged_notes` | array | 按随机 tag 分组的笔记集合 |
| `tagged_notes[].tag` | `TagRecord` | 被随机抽中的 tag |
| `tagged_notes[].notes` | `NoteRecord[]` | 该 tag 下未删除、未归档 notes |

### 查询规则

| 规则 | 设计 |
| --- | --- |
| 默认数量 | `n` 不传时使用 3 |
| 数量范围 | 小于 1 或大于 20 返回 `422 validation_error` |
| 随机 tag | 使用 SQLite `ORDER BY RANDOM() LIMIT ?` 从当前 workspace tags 中抽取 |
| tag 数不足 | SQL 返回多少就组装多少，不额外报错 |
| notes 过滤 | 查询 notes 时使用 `deleted_at IS NULL` 和 `archived_at IS NULL` |
| 分组规则 | 每个随机 tag 独立查询并生成一个 group |
| 重复 note | 不做跨 group 去重 |
| notes 排序 | 每个 tag 下的 notes 按 `updated_at DESC, id DESC` 返回，保证组内稳定可读 |

### 错误响应

| 场景 | HTTP 状态 | code |
| --- | --- | --- |
| `n` 校验失败 | `422` | `validation_error` |
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
| 默认参数 | `GET /random/tags` 使用默认 `n = 3` |
| 合法参数 | `GET /random/tags?n=2` 返回最多 2 个 tag group |
| 下界校验 | `n = 0` 返回 `422 validation_error` |
| 上界校验 | `n = 21` 返回 `422 validation_error` |
| tag 不足 | 可用 tag 少于 `n` 时请求成功并返回现有 tag 数 |
| 软删除过滤 | 软删除 note 不出现在任何 group 的 notes 中 |
| 归档过滤 | 已归档 note 不出现在任何 group 的 notes 中 |
| 重复 note | 同一 note 关联多个抽中 tag 时，可出现在多个 group 中 |
| OpenAPI | `/api-docs/openapi.json` 包含 `/random/tags` |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| curl 调用随机接口 | 返回 `200 OK` 和 `{ "tagged_notes": [...] }` |
| 多次 curl 调用 | 返回 tag 组合不要求一致 |
| Swagger UI 查看 | notes tag 下能看到 `GET /random/tags` |
