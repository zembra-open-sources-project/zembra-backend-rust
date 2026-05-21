# r019 Note Links API 设计文档

日期：2026-05-21

关联需求澄清：`docs/request-clarify/r019-note-links-api.md`

## 核心功能（WHAT）

升级 note 创建、修改和读取接口，让后端接收客户端解析好的结构化 links，维护 `note_links` 表，并在响应 metadata 中返回 outgoing links 和 backlinks。后端不解析正文，只负责校验目标 note、维护事务一致性、记录 sync changes，并严格只处理未删除、未归档 note。

### 需求背景（WHY）

真双链笔记需要数据库中有可靠的 note-to-note 关系。当前 shared schema 已有 `note_links` 表和相关索引，但后端创建、修改和读取接口尚未维护该表。WebUI 和 CLI 会负责内容解析，因此后端应保持职责单一：接收结构化 links 并更新关系表。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 创建时写 links | `POST /notes` 支持 body.links，创建 note 后写入 outgoing links |
| 修改时维护 links | `PATCH /notes/{note_ref}` 支持 body.links，传入时整体替换 outgoing links |
| 读取时返回双链信息 | `GET /notes/{note_ref}` 返回扩展 `NoteResponse`，metadata 包含 outgoing links 和 backlinks |
| 后端不解析正文 | 后端不理解 Markdown、`[[...]]` 或编辑器语法 |
| 只处理可见 note | link source 和 target 均必须未删除、未归档 |
| 保持事务一致性 | note、revision、field/tags、note_links 和 sync changes 在同一事务中完成 |
| 同步 API 合同 | 更新 DTO、models、OpenAPI 和测试 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| DTO | 新增 link request/response DTO，扩展 create/update request 和 metadata |
| Models | 新增 `NoteLinkRecord` 映射 `note_links` |
| Repository | 写入、整体替换、查询 outgoing links/backlinks，并记录 sync changes |
| Service | 校验和归一化 links，组装扩展 `NoteResponse` |
| Handler | 创建、修改、读取接口返回扩展 `NoteResponse` |
| Sync | 支持本地记录和远端重放 `note_link` 关系变化 |
| OpenAPI | 注册新增 schema 和更新接口响应 |
| Tests | 覆盖创建、修改、读取、可见性过滤、事务和 OpenAPI |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 内容解析 | 不在后端解析 content |
| 前端/CLI | 不实现 WebUI 或 CLI 解析逻辑 |
| 自动建 note | 不自动创建被引用 note |
| 已归档/已删除 note | 不允许作为 link source 或 target |
| 自引用 | 不支持 `source_note_id = target_note_id` |
| 独立 link 编辑 API | 不新增单独的 `/links` 管理接口 |

## 实现流程（HOW）

### 架构落点

| 层级 | 文件/模块 | 设计 |
| --- | --- | --- |
| Model | `src/models/note_link.rs`, `src/models/mod.rs` | 新增 `NoteLinkRecord`，字段对齐 shared schema |
| DTO | `src/dto/notes.rs` | 新增 `NoteLinkRequest`、`NoteLinkResponse`，扩展 `CreateNoteRequest`、`UpdateNoteRequest`、`NoteMetadata` |
| Repository | `src/repositories/notes.rs` | 增加 link 写入、替换、查询、payload 和 sync change 辅助函数 |
| Service | `src/services/notes.rs` | 归一化 links，调用 repository，统一组装 `NoteResponse` |
| Handler | `src/handlers/notes.rs` | `create_note`、`update_note`、`get_note` 返回 `NoteResponse` |
| OpenAPI | `src/api_doc.rs` | 注册 link DTO/model schema 并确保 path 响应更新 |
| Sync | `src/repositories/sync.rs` | 增加 `note_link` attach/detach 或 insert/delete 的 remote apply |

### API 合同

#### `POST /notes`

| 项目 | 内容 |
| --- | --- |
| Request Body | `CreateNoteRequest` 增加 `links` |
| `links` 未传 | 按空 links 处理 |
| Response Body | 扩展 `NoteResponse` |

#### `PATCH /notes/{note_ref}`

| 项目 | 内容 |
| --- | --- |
| Request Body | `UpdateNoteRequest` 增加 `links` |
| `links` 未传 | 保持当前 outgoing links 不变 |
| `links: []` | 清空当前 outgoing links |
| `links` 非空 | 整体替换当前 outgoing links |
| Response Body | 扩展 `NoteResponse` |

#### `GET /notes/{note_ref}`

| 项目 | 内容 |
| --- | --- |
| Response Body | 扩展 `NoteResponse` |
| Metadata | 包含 `outgoing_links` 和 `backlinks` |

### 请求字段

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `target_note_ref` | string | 是 | 目标 note 完整 32 位 id 或唯一前缀 |
| `anchor_text` | string/null | 否 | 客户端解析出的链接文本 |
| `position` | integer/null | 否 | 链接在正文中的位置，必须大于等于 0 |

### 响应字段

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | `note_links.id` |
| `source_note_id` | string | 发起引用的完整 note uuid |
| `target_note_id` | string | 被引用的完整 note uuid |
| `anchor_text` | string/null | 链接文本 |
| `position` | integer/null | 正文位置 |
| `created_at` | integer | 关系创建时间 |

### Repository 规则

| 场景 | 设计 |
| --- | --- |
| source note 查询 | 创建后使用新 note；修改和读取使用可见 note 查询，即未删除、未归档 |
| target note 查询 | 使用 note ref 解析，且必须未删除、未归档 |
| 自引用 | 在写入前返回 validation error |
| 重复 target | 不按 target 去重；不同 position 保留多条 |
| 创建 links | 为每条 link 生成独立 `note_links.id` |
| 替换 links | 查询当前 outgoing links，删除旧关系并插入新关系 |
| backlinks 查询 | 查询 `target_note_id = 当前 note id`，并过滤 source note 未删除、未归档 |
| outgoing 查询 | 查询 `source_note_id = 当前 note id`，并过滤 target note 未删除、未归档 |

### Sync 规则

| 场景 | 设计 |
| --- | --- |
| 新增 link | 记录 `note_link` 关系新增 sync change |
| 删除 link | 记录 `note_link` 关系删除 sync change |
| payload | 包含 `id`、`workspace_id`、`source_note_id`、`target_note_id`、`anchor_text`、`position`、`created_at` |
| remote apply | 按 payload 插入或删除 `note_links` |

具体 operation 名称在实现时优先沿用关系表语义：`attach` / `detach`。如果现有远端同步实现更适合实体语义，可在设计评审中固定为 `insert` / `delete`，但必须保持本地记录和远端重放一致。

### 错误响应

| 场景 | HTTP 状态 | code |
| --- | --- | --- |
| JSON 格式错误 | `400` | `invalid_json` |
| link target 格式无效 | `422` | `invalid_note_reference` 或 `validation_error` |
| link target 不存在、已删除或已归档 | `404` | `record_not_found` |
| link target 前缀冲突 | `409` | `ambiguous_note_reference` |
| 自引用 | `422` | `validation_error` |
| position 小于 0 | `422` | `validation_error` |
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
| 创建 note 带 links | 写入 `note_links` 并返回 outgoing links |
| 创建 note 不带 links | 创建成功，outgoing links 为空 |
| PATCH 不带 links | 保留原 outgoing links |
| PATCH links 为空数组 | 清空 outgoing links |
| PATCH links 非空 | 整体替换 outgoing links |
| GET note | 返回 outgoing links 和 backlinks |
| 已删除 target | 创建或修改返回错误，事务回滚 |
| 已归档 target | 创建或修改返回错误，事务回滚 |
| 已删除/已归档 source | GET/PATCH 不匹配该 note |
| 自引用 | 返回 validation error |
| 重复 target 不同 position | 保留多条 link |
| sync changes | 新增和删除 links 均有对应 sync change |
| OpenAPI | request/response schema 包含 links 字段 |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| curl 创建带 links note | 返回 `201 Created` 和 metadata.outgoing_links |
| curl 修改 links | 返回 `200 OK`，再次 GET 可看到 outgoing/backlinks 更新 |
| Swagger UI 查看 | notes tag 下 create/update/get 展示扩展 schema |

