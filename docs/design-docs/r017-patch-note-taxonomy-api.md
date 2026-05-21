# r017 PATCH Note Taxonomy API 设计文档

日期：2026-05-21

关联需求澄清：`docs/request-clarify/r017-patch-note-taxonomy-api.md`

## 核心功能（WHAT）

扩展现有 `PATCH /notes/{note_ref}`，在保持 content 严格必选和当前返回体不变的前提下，允许请求附带更新 note 的 `field` 和 `tags`。`tags` 使用整体替换语义；`field` 未传时不修改，传字符串时设置到对应 field，传 `null` 时设置为默认 field `inbox`。

### 需求背景（WHY）

当前后端已经支持通过 `PATCH /notes/{note_ref}` 修改正文，并写入新的 revision；也支持创建 note 时设置 field/tags，以及通过独立接口增删单个 tag。客户端编辑笔记时需要一次性提交正文、field 和 tags 的最终结果，避免多次请求造成 UI 状态和后端状态短暂不一致。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 保留 PATCH 编辑入口 | 不新增 path，不改变 `PATCH /notes/{note_ref}` 的基本语义 |
| content 严格必选 | 请求必须继续包含非空 content，并复用现有 revision 写入逻辑 |
| 支持 field 更新 | field 未传不修改；字符串设置到对应 field；`null` 设置为默认 `inbox` |
| 支持 tags 整体替换 | 请求传入 tags 后，note 最终 tags 等于清洗后的请求 tags |
| 保持返回体不变 | 响应继续返回 `NoteRecord` |
| 保持事务一致性 | content、field、tags 更新在同一事务内完成 |
| 保持同步一致性 | 正确生成 note、note_revision、field、tag、note_tag sync changes |
| 同步 API 合同 | 更新 DTO、OpenAPI schema 和行为测试 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| DTO | 扩展 `UpdateNoteRequest`，新增 `field` 和 `tags` |
| Service | 复用 content 校验、field/tag 清洗与默认 `inbox` 规则 |
| Repository | 扩展 note 更新事务，支持 field 设置和 tags 整体替换 |
| Handler | 保持原 handler path 和返回体，继续校验 request body |
| OpenAPI | 更新 `UpdateNoteRequest` schema，保持 `PATCH /notes/{note_ref}` path |
| Tests | 覆盖 field 三态、tags 替换、事务和 OpenAPI schema |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 新增 HTTP path | 不新增 `POST /notes/{note_uuid}` 或其他编辑接口 |
| tag 增量编辑 | 不新增 `add_tags`、`remove_tags` 请求字段 |
| field 清空 | 不允许把 note field 清空；`null` 代表默认 `inbox` |
| 返回体扩展 | 不改为 `NoteResponse`，不附加 metadata |
| 实体清理 | tags/fields 不再被 note 引用时不删除实体 |
| 无 content 更新 | 不支持只更新 field/tags，content 仍严格必选 |
| 数据库 schema | 不新增表或字段 |

## 实现流程（HOW）

### 架构落点

| 层级 | 文件/模块 | 设计 |
| --- | --- | --- |
| DTO | `src/dto/notes.rs` | 扩展 `UpdateNoteRequest`，保留 `content` 和 `device_id`，新增 field/tags 表达 |
| Service | `src/services/notes.rs` | 归一化 content、field 和 tags，构造 repository 输入 |
| Repository | `src/repositories/notes.rs` | 在单个事务内更新 content/revision、field_id、note_tags 和 sync changes |
| Handler | `src/handlers/notes.rs` | 沿用 `update_note`，OpenAPI request body 自动反映新增字段 |
| OpenAPI | `src/api_doc.rs` | 已注册 `UpdateNoteRequest`，需确保 schema 包含新增字段 |
| Tests | `src/app.rs`、`src/repositories/notes.rs` | 扩展现有 inline tests，覆盖 API 和 repository 行为 |

### API 合同

| 项目 | 内容 |
| --- | --- |
| Method | `PATCH` |
| Path | `/notes/{note_ref}` |
| Path 参数 | 完整 32 位 hex ID 或至少 4 位唯一前缀 |
| Request Body | `UpdateNoteRequest` |
| Response Body | `NoteRecord` |
| Tag | `notes` |

Request body：

| 字段 | 类型 | 必填 | 语义 |
| --- | --- | --- | --- |
| `content` | string | 是 | 新正文；trim 后不能为空；写入新 revision |
| `device_id` | string/null | 否 | 写入 revision 的设备标识，沿用现有语义 |
| `field` | string/null/absent | 否 | absent 不修改；string 设置对应 field；null 设置默认 `inbox` |
| `tags` | string[]/absent | 否 | absent 不修改；array 整体替换为清洗后的 tags |

### DTO 表达

`field` 需要区分 absent 与 null。实现时建议用 `Option<Option<String>>` 表达：

| JSON 输入 | Rust 表达 | 行为 |
| --- | --- | --- |
| 字段不存在 | `None` | 不修改 field |
| `"field": null` | `Some(None)` | 设置为 `inbox` |
| `"field": "work"` | `Some(Some("work"))` | 设置为 `work` |

`tags` 使用 `Option<Vec<String>>` 表达：

| JSON 输入 | Rust 表达 | 行为 |
| --- | --- | --- |
| 字段不存在 | `None` | 不修改 tags |
| `"tags": []` | `Some(vec![])` | 移除所有 tag 关联 |
| `"tags": ["a", "b"]` | `Some(vec![...])` | 整体替换为清洗去重后的 tags |

### Repository 事务设计

| 步骤 | 设计 |
| --- | --- |
| 1 | 通过 `get_note_by_ref` 解析目标 note，沿用现有错误合同 |
| 2 | 开启 SQLite transaction |
| 3 | 生成新 revision，插入 `note_revisions` |
| 4 | 解析 field 更新：absent 保留旧 `field_id`；null 查询或创建 `inbox`；string 查询或创建对应 field |
| 5 | 更新 `notes.content`、`updated_at`、`current_revision_id` 和必要的 `field_id` |
| 6 | 如请求包含 tags，查询或创建目标 tags，计算当前关联与目标关联差异 |
| 7 | 对新增 tag 关联写入 `note_tags` 和 `note_tag attach` sync change |
| 8 | 对移除 tag 关联删除 `note_tags` 和写入 `note_tag detach` sync change |
| 9 | 写入 `note_revision insert` 和 `note update` sync changes |
| 10 | commit 并返回更新后的 `NoteRecord` |

### 默认 field 规则

| 场景 | 规则 |
| --- | --- |
| field 未传 | 不查询、不创建、不更新 field |
| field 为 null | 查询或创建名为 `inbox` 的 field，并把 note 归属到该 field |
| field 为字符串 | trim 后不能为空；查询或创建对应 field |
| field 为纯空白 | 返回 `422 validation_error` |

### tags 替换规则

| 场景 | 规则 |
| --- | --- |
| tags 未传 | 不修改当前关联 |
| tags 为空数组 | 删除该 note 的所有 tag 关联 |
| tags 包含空白 | trim 后为空的项丢弃 |
| tags 包含重复 | 保留首次出现顺序，去重后替换 |
| tag 不存在 | 在同一事务中创建 tag，并记录 tag insert sync change |

### 错误响应

| 场景 | HTTP 状态 | code |
| --- | --- | --- |
| JSON 格式错误 | `400` | `invalid_json` |
| content 为空或纯空白 | `422` | `validation_error` |
| field 字符串为空或纯空白 | `422` | `validation_error` |
| note ref 少于 4 位 | `422` | `note_reference_too_short` |
| note ref 非 hex | `422` | `invalid_note_reference` |
| note 不存在 | `404` | `record_not_found` |
| note ref 匹配多个 note | `409` | `ambiguous_note_reference` |
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
| 只传 content | 保持现有更新正文和 revision 行为 |
| 传 field 字符串 | note.field_id 更新到对应 field |
| field 不存在 | 自动创建 field 并关联 note |
| field 未传 | 原 field_id 不变 |
| field 为 null | note.field_id 更新到默认 `inbox` |
| field 为空白字符串 | 返回 `422 validation_error` |
| 传 tags 数组 | note tags 整体替换为清洗后 tags |
| tags 为空数组 | note 所有 tag 关联被移除 |
| tags 未传 | 原 tag 关联不变 |
| tags 含重复和空白 | trim、去空、去重后替换 |
| 新 tag | 自动创建 tag 并关联 note |
| sync changes | 生成 note_revision、note update、field/tag insert、note_tag attach/detach |
| OpenAPI | `/api-docs/openapi.json` 中 `UpdateNoteRequest` 包含 field/tags |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| curl PATCH 同时传 content、field、tags | 返回 `200 OK` 和更新后的 `NoteRecord` |
| curl PATCH 不传 field/tags | 行为与当前 content 更新一致 |
| Swagger UI 查看 | notes tag 下 `PATCH /notes/{note_ref}` body 展示新增字段 |
