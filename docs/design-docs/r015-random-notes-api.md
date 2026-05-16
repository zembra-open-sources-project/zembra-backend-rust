# r015 Random Notes API 设计文档

日期：2026-05-16

关联需求澄清：`docs/request-clarify/r015-random-notes-api.md`

## 核心功能（WHAT）

新增 `GET /random/notes`，通过必填 query 参数 `n` 指定随机 note 数量。接口每次请求从未删除、未归档 notes 中实时随机抽取最多 `n` 条，响应顶级字段为 `notes`。

### 需求背景（WHY）

已有 random tags 和 random fields 分组式探索接口。新接口提供更直接的随机笔记能力，适合客户端做快速漫游、灵感浏览或随机回看。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 新增随机 notes 接口 | 提供 `GET /random/notes?n=N` |
| 限制随机规模 | `n` 合法范围为 1 到 50，必填 |
| notes 不足兼容 | 可用 notes 少于 `n` 时返回现有数量 |
| 无笔记兼容 | 没有可用 notes 时返回空数组 |
| 过滤不可展示内容 | notes 只返回未删除、未归档记录 |
| 保持随机性 | 每次请求实时随机，不支持 seed，不保证可复现 |
| 同步 API 合同 | 补齐 DTO、handler OpenAPI 标注和 `ApiDoc` 注册 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| DTO | 新增 random notes query DTO，包含 `n` 校验 |
| Repository | 新增随机可见 notes 查询方法 |
| Service | 暴露随机 notes 查询语义 |
| Handler | 新增 `GET /random/notes` handler，解析 query、校验并返回 `ListNotesResponse` |
| Routes | 注册 `/random/notes` 路由 |
| OpenAPI | 注册 path、query 参数、response schema 和错误响应 |
| Tests | 覆盖参数校验、数量不足、无笔记、软删除过滤、归档过滤和 OpenAPI 暴露 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 可复现随机 | 不支持 seed，不保证多次请求结果一致 |
| 分页能力 | 不实现分页、游标或总数统计 |
| 修改现有接口 | 不改变 `/notes`、`/notes/recent`、`/random/tags`、`/random/fields` 行为 |
| 数据库 schema | 不新增表或字段 |
| 认证授权 | 不新增鉴权逻辑 |

## 实现流程（HOW）

### 架构落点

| 层级 | 文件/模块 | 设计 |
| --- | --- | --- |
| DTO | `src/dto/notes.rs` | 新增 `RandomNotesQuery` |
| Repository | `src/repositories/notes.rs` | 新增 `list_random_notes(limit)` |
| Service | `src/services/notes.rs` | 新增 `random_notes(query)` |
| Handler | `src/handlers/notes.rs` | 新增 `random_notes` handler，解析 `Query<RandomNotesQuery>` |
| Routes | `src/routes/notes.rs` | 注册 `GET /random/notes` |
| OpenAPI | `src/api_doc.rs` | 注册 handler path |

### API 合同

| 项目 | 内容 |
| --- | --- |
| Method | `GET` |
| Path | `/random/notes` |
| Query | `n` |
| Response Body | `ListNotesResponse` |
| Tag | `notes` |

Query 参数：

| 字段 | 类型 | 必填 | 默认值 | 约束 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `n` | integer | 是 | 无 | 1 到 50 | 随机抽取的 note 数量 |

### 查询规则

| 规则 | 设计 |
| --- | --- |
| 数量范围 | `n` 小于 1 或大于 50 返回 `422 validation_error` |
| 随机 notes | 使用 SQLite `ORDER BY RANDOM() LIMIT ?` 从当前 workspace notes 中抽取 |
| notes 过滤 | 查询 notes 时使用 `deleted_at IS NULL` 和 `archived_at IS NULL` |
| notes 不足 | SQL 返回多少就响应多少，不额外报错 |
| 无笔记 | 返回 `notes: []` |

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
| 合法参数 | `GET /random/notes?n=2` 返回最多 2 条 notes |
| 下界校验 | `n = 0` 返回 `422 validation_error` |
| 上界校验 | `n = 51` 返回 `422 validation_error` |
| notes 不足 | 可用 notes 少于 `n` 时请求成功并返回现有 notes 数 |
| 无笔记 | 没有可用 notes 时返回 `notes: []` |
| 软删除过滤 | 软删除 note 不出现在响应中 |
| 归档过滤 | 已归档 note 不出现在响应中 |
| OpenAPI | `/api-docs/openapi.json` 包含 `/random/notes` |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| curl 调用随机接口 | 返回 `200 OK` 和 `{ "notes": [...] }` |
| 多次 curl 调用 | 返回 notes 组合不要求一致 |
| Swagger UI 查看 | notes tag 下能看到 `GET /random/notes` |
