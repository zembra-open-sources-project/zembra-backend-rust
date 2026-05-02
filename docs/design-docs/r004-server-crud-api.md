# r004 Server CRUD API 设计文档

日期：2026-05-02

需求来源：`docs/http-client-server-api.md`
关联需求澄清：`docs/request-clarify/r001-backend-tech-stack.md`、`docs/request-clarify/r003-shared-schema-submodule.md`

## 核心功能（WHAT）

在 Rust 后端中落地基于 SQLite + SQLx 的服务端 CRUD 能力，让常驻 HTTP server 复用数据库连接池，对外提供 notes、fields、tags 的 JSON API。第一阶段优先覆盖 CLI HTTP client 迁移需要的新增 note、批量新增、fields/tags 查询和基础 note 读写删改能力。

### 需求背景（WHY）

当前 CLI 每次 `add` 都会加载配置、检查 SQLite、打开连接并创建 Repository。普通一次性调用会反复创建数据库会话，交互式调用虽然复用连接，但数据访问仍在本地 CLI 进程内完成。后端需要承担常驻数据库访问职责，降低高频写入开销，并为后续客户端统一接入提供稳定 API。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 服务端持有连接池 | 启动时根据 `database.path` 创建 SQLite 连接池，并注入 Axum state |
| 接入 schema 契约 | migration 和模型映射以 `vendor/zembra-schema` 为唯一数据结构来源 |
| 保持 Repository 语义 | field/tag 自动创建、note revision 写入、软删除过滤、note ref 解析与现有 CLI 语义一致 |
| 提供 HTTP CRUD | 暴露新增、批量新增、查询、更新、软删除、归档、tags/revisions 查询等接口 |
| 统一错误响应 | 数据库、校验、not found、冲突和未初始化错误统一映射为 JSON error |
| 可测试落地 | Repository、Service、Handler 和 Router 均有自动化验证覆盖 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| 数据库初始化 | 创建连接池，执行或挂接 shared schema migration，启动失败时返回明确错误 |
| Models | 定义 notes、fields、tags、revisions 等数据库记录结构 |
| DTO | 定义 API request/response 和统一 error response |
| Repository | 使用 SQLx 显式 SQL 实现 note、field、tag、revision 的数据访问 |
| Service | 组合事务、校验 role、处理 tag 去重和 note ref 解析 |
| HTTP API | 实现 `GET /health` 扩展、`POST /notes`、`POST /notes/batch`、`GET /fields`、`GET /tags` 和 note CRUD |
| 测试 | 使用临时 SQLite 数据库验证 repository/service/API 行为 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 独立设计表结构 | 不复制或改写共享 schema 正文 |
| 认证授权 | 本轮不实现 token、用户系统或权限模型 |
| CLI HTTP client | 本轮只实现 server CRUD，不修改 CLI |
| 前端页面 | 不新增 UI |
| 复杂同步冲突 | 只保留 revision 查询和写入，不实现多端冲突合并 |

## 实现流程（HOW）

### 架构落点

| 层级 | 文件/模块 | 职责 |
| --- | --- | --- |
| App State | `src/app.rs` | 持有 `SqlitePool` 和服务实例，注册路由 |
| Database | `src/repositories/` | 初始化连接池、执行 migration、封装 SQLx 查询 |
| Models | `src/models/` | 映射数据库记录，字段对齐 shared schema |
| DTO | `src/dto/` | 定义请求、响应、错误 JSON 结构 |
| Services | `src/services/` | 承载事务和业务语义，避免 handler 直接写 SQL |
| Handlers | `src/handlers/` | 接收 Axum extractor，调用 service，返回 HTTP response |
| Routes | `src/routes/` | 注册 notes、fields、tags、health 路由 |
| Error | `src/error.rs` | 统一 `AppError`、`ApiError` 和 `IntoResponse` 映射 |

### 数据库启动策略

| 步骤 | 设计 |
| --- | --- |
| 1 | `Settings::load()` 读取 `database.path` |
| 2 | 转换为 SQLx SQLite URL |
| 3 | 创建 `SqlitePool`，连接失败直接阻止 server 启动 |
| 4 | 执行 migration；migration 来源优先使用 `vendor/zembra-schema/migrations/`，如 SQLx 宏无法直接引用 submodule 路径，则在构建期或代码中建立明确桥接 |
| 5 | 将 pool 放入 `AppState`，所有请求复用 |

### API 设计

| Method | Path | 功能 | 状态 |
| --- | --- | --- | --- |
| `GET` | `/health` | 健康检查，返回服务和数据库初始化状态 | 必须实现 |
| `POST` | `/notes` | 创建单条 note、初始 revision、field、tags | 必须实现 |
| `POST` | `/notes/batch` | 批量创建 notes，同一请求内使用事务 | 必须实现 |
| `GET` | `/fields` | 按名称升序列出 fields | 必须实现 |
| `GET` | `/tags` | 按名称升序列出 tags | 必须实现 |
| `GET` | `/notes` | 按更新时间列出未软删除 notes | 后续兼容 |
| `GET` | `/notes/{note_ref}` | 按完整 ID 或唯一前缀读取 note | 后续兼容 |
| `PATCH` | `/notes/{note_ref}` | 更新 note content 并写入 revision | 后续兼容 |
| `POST` | `/notes/{note_ref}/archive` | 归档 note | 后续兼容 |
| `DELETE` | `/notes/{note_ref}` | 软删除 note | 后续兼容 |
| `GET` | `/notes/{note_ref}/tags` | 查询 note tags | 后续兼容 |
| `PUT` | `/notes/{note_ref}/tags/{tag_name}` | 添加 note tag | 后续兼容 |
| `DELETE` | `/notes/{note_ref}/tags/{tag_name}` | 移除 note tag | 后续兼容 |
| `GET` | `/notes/{note_ref}/revisions` | 查询 note revisions | 后续兼容 |

### 关键业务规则

| 规则 | 设计 |
| --- | --- |
| Role 校验 | 只接受 `Human` 和 `Agent`，默认 `Human` |
| Content 校验 | 创建和更新时拒绝空字符串或纯空白 |
| Field 处理 | field 非空时按 name 查询，不存在则创建 |
| Tag 处理 | server 防御性 trim、去空、去重；不存在则创建 |
| Note 创建 | note、初始 revision、current_revision_id 更新和 note_tags 写入在同一事务中完成 |
| Batch 创建 | 同一批次使用同一事务，任一 item 失败则整体回滚 |
| Note ref | 支持 32 位完整 hex ID 或至少 4 位唯一前缀 |
| 删除 | DELETE 为软删除，设置 `deleted_at`；查询默认排除软删除记录 |
| 归档 | 设置 `archived_at`，不影响 revision 历史 |

### 错误响应

统一响应：

```json
{
  "error": {
    "code": "record_not_found",
    "message": "Note reference \"abcd\" did not match any note.",
    "details": {}
  }
}
```

| 场景 | HTTP 状态 | code |
| --- | --- | --- |
| JSON 格式错误 | `400` | `invalid_json` |
| 字段校验失败 | `422` | `validation_error` |
| note ref 少于 4 位 | `422` | `note_reference_too_short` |
| note ref 非 hex | `422` | `invalid_note_reference` |
| note 不存在 | `404` | `record_not_found` |
| note ref 匹配多个 note | `409` | `ambiguous_note_reference` |
| 数据库未初始化 | `503` | `database_not_initialized` |
| SQLite 访问失败 | `500` | `database_error` |

## 测试用例

### 编译检查

| 用例 | 预期 |
| --- | --- |
| `cargo fmt --check` | 格式检查通过 |
| `cargo check` | 类型检查通过 |
| `cargo clippy` | 无新增 warning |
| `cargo test` | 单元测试和集成测试通过 |

### 自动化行为检查

| 用例 | 预期 |
| --- | --- |
| 创建 note | 返回 `201`，写入 note、revision、field、tags、note_tags |
| 批量创建 | 成功时全部写入，失败时事务回滚 |
| 列出 fields/tags | 按 name 升序返回，支持默认 limit 和 all |
| note ref 查询 | 完整 ID 和唯一前缀均可查询 |
| note ref 冲突 | 多个匹配返回 `409 ambiguous_note_reference` |
| 更新 note | content 更新，新增 revision，current_revision_id 更新 |
| 软删除 note | 设置 deleted_at，默认列表不返回 |
| 归档 note | 设置 archived_at，查询仍可按 ref 返回 |
| tag 关联 | 添加幂等，删除只移除关联不删除 tag 实体 |
| 错误响应 | 所有业务错误返回统一 JSON error |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| 启动 server | 日志输出监听地址和数据库路径，连接池初始化成功 |
| 调用 `/health` | 返回 `service = "zembra-server"` 和数据库状态 |
| 用 curl 创建 note | 响应结构可被 CLI HTTP client 直接消费 |

