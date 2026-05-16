# r014 Random Fields API 设计文档

日期：2026-05-16

关联需求澄清：`docs/request-clarify/r014-random-fields-api.md`

## 核心功能（WHAT）

新增 `GET /random/fields`，通过 query 参数 `n` 指定随机 field 数量，通过 `count` 指定所有 field 分组累计返回的 note 数量上限。接口每次请求从现有 fields 中实时随机抽取 `n` 个 field，并按 field 分组返回未删除、未归档 notes。顶级响应字段固定为 `field_notes`。

### 需求背景（WHY）

`GET /random/tags` 已提供 tag-based 随机发现能力。新接口补齐 field-based 随机发现，让客户端可以围绕 field 维度随机浏览笔记，同时通过 `count` 控制单次响应体规模。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 新增随机 field 接口 | 提供 `GET /random/fields?n=N&count=CNT` |
| 限制随机规模 | `n` 合法范围为 1 到 20，默认 3 |
| 限制 note 总量 | `count` 合法范围为 1 到 100，默认 20 |
| field 不足兼容 | 可用 field 少于 `n` 时返回现有数量 |
| 按 field 分组 | 顶级字段为 `field_notes`，每项包含 field 和 notes |
| 过滤不可展示内容 | notes 只返回未删除、未归档记录 |
| 保持随机性 | 每次请求实时随机，不支持 seed，不保证可复现 |
| 同步 API 合同 | 补齐 DTO、handler OpenAPI 标注和 `ApiDoc` 注册 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| DTO | 新增 random fields query 和响应 DTO，包含 `n`、`count` 校验、field 分组响应 |
| Repository | 新增随机 field 查询和按 field ID 查询可见 notes 的读取方法 |
| Service | 组装随机 field 和对应 notes，并控制累计 notes 不超过 `count` |
| Handler | 新增 `GET /random/fields` handler，解析 query、校验并返回响应 |
| Routes | 注册 `/random/fields` 路由 |
| OpenAPI | 注册 path、query 参数、response schema 和错误响应 |
| Tests | 覆盖参数校验、field 不足、累计 count、软删除过滤、归档过滤、空 field 和 OpenAPI 暴露 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 可复现随机 | 不支持 seed，不保证多次请求结果一致 |
| 分页能力 | 不实现分页、游标或总数统计 |
| 修改现有接口 | 不改变 `/fields`、`/notes`、`/notes/recent`、`/random/tags` 行为 |
| 数据库 schema | 不新增表或字段 |
| 认证授权 | 不新增鉴权逻辑 |

## 实现流程（HOW）

### 架构落点

| 层级 | 文件/模块 | 设计 |
| --- | --- | --- |
| DTO | `src/dto/notes.rs` | 新增 `RandomFieldsQuery`、`FieldNotesResponse`、`FieldNotesGroup` |
| Repository | `src/repositories/notes.rs` | 新增随机 field 读取和按 field ID 查询可见 notes 方法 |
| Service | `src/services/notes.rs` | 新增 `random_field_notes(query)`，处理默认值、累计 count 和分组组装 |
| Handler | `src/handlers/notes.rs` | 新增 `random_field_notes` handler，解析 `Query<RandomFieldsQuery>` |
| Routes | `src/routes/notes.rs` | 注册 `GET /random/fields` |
| OpenAPI | `src/api_doc.rs` | 注册 handler path 和新增 response schema |

### API 合同

| 项目 | 内容 |
| --- | --- |
| Method | `GET` |
| Path | `/random/fields` |
| Query | `n`、`count` |
| Response Body | `FieldNotesResponse` |
| Tag | `notes` |

Query 参数：

| 字段 | 类型 | 必填 | 默认值 | 约束 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `n` | integer | 否 | 3 | 1 到 20 | 随机抽取的 field 数量 |
| `count` | integer | 否 | 20 | 1 到 100 | 所有 field 分组累计返回 notes 上限 |

响应体：

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

### 查询规则

| 规则 | 设计 |
| --- | --- |
| 默认 field 数 | `n` 为 `None` 时使用 3 |
| 默认 note 数 | `count` 为 `None` 时使用 20 |
| field 随机 | 使用 SQLite `ORDER BY RANDOM() LIMIT ?` 从当前 workspace fields 中抽取 |
| field 数不足 | SQL 返回多少就组装多少，不额外报错 |
| notes 过滤 | 查询 notes 时使用 `deleted_at IS NULL` 和 `archived_at IS NULL` |
| notes 随机 | 每个 field 下 notes 使用 `ORDER BY RANDOM()` 获取，提升发现随机性 |
| 累计 count | service 按随机 field 顺序逐组分配剩余额度，所有 notes 总数不超过 `count` |
| 空 field | field 没有可见 notes 时返回空数组 |

### 错误响应

| 场景 | HTTP 状态 | code |
| --- | --- | --- |
| `n` 校验失败 | `422` | `validation_error` |
| `count` 校验失败 | `422` | `validation_error` |
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
| 默认参数 | `GET /random/fields` 使用默认 `n = 3`、`count = 20` |
| 合法参数 | `GET /random/fields?n=2&count=3` 返回最多 2 个 field group，notes 累计最多 3 条 |
| 下界校验 | `n = 0` 或 `count = 0` 返回 `422 validation_error` |
| 上界校验 | `n = 21` 或 `count = 101` 返回 `422 validation_error` |
| field 不足 | 可用 field 少于 `n` 时请求成功并返回现有 field 数 |
| 软删除过滤 | 软删除 note 不出现在任何 group 的 notes 中 |
| 归档过滤 | 已归档 note 不出现在任何 group 的 notes 中 |
| 空 field | 无可见 notes 的 field 返回 `notes: []` |
| OpenAPI | `/api-docs/openapi.json` 包含 `/random/fields` |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| curl 调用随机接口 | 返回 `200 OK` 和 `{ "field_notes": [...] }` |
| 多次 curl 调用 | 返回 field 和 notes 组合不要求一致 |
| Swagger UI 查看 | notes tag 下能看到 `GET /random/fields` |
