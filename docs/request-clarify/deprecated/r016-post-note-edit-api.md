# r016 POST Note Edit API 需求澄清

日期：2026-05-21

## 背景

当前后端已经具备笔记内容编辑能力，但公开 HTTP API 使用 `PATCH /notes/{note_ref}`。本轮需求期望提供 `POST /notes/{note_uuid}` 来编辑笔记内容，并且路径参数支持 8 位短 ID 以及完整 ID。

## 仓库现状

| 项目 | 当前结论 |
| --- | --- |
| 已有编辑 API | 已实现 `PATCH /notes/{note_ref}` |
| 当前路由位置 | `src/routes/notes.rs` 注册 `/notes/{note_ref}` 的 `patch(update_note)` |
| 当前 handler | `src/handlers/notes.rs::update_note` |
| 当前 service | `src/services/notes.rs::update_note` |
| 当前 repository | `src/repositories/notes.rs::update_note_content` |
| 当前请求体 | `UpdateNoteRequest { content, device_id }` |
| 当前编辑行为 | 更新 `notes.content`，写入新的 `note_revisions`，更新 `current_revision_id`，记录 sync changes |
| 当前 ID 支持 | `note_ref` 支持完整 32 位 hex ID 或唯一 hex 前缀 |
| 当前最短前缀 | 4 位 |
| 当前冲突行为 | 前缀匹配多条 note 时返回 `409 ambiguous_note_reference` |
| 当前不存在行为 | 未匹配 note 返回 `404 record_not_found` |
| 当前格式错误 | 非 hex 或长度不足返回 `422` |
| 当前 OpenAPI | 已注册 `PATCH /notes/{note_ref}`，未注册 `POST /notes/{note_uuid}` 编辑接口 |

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| 目标 API | 新增或调整为 `POST /notes/{note_uuid}` |
| 核心能力 | 编辑笔记内容 |
| ID 形态 | 支持 8 位短 ID 和完整 ID |
| 请求体 | 复用现有 `UpdateNoteRequest` 的可能性最高，即 `{ "content": "...", "device_id": "..." }` |
| 响应体 | 复用当前编辑接口返回的 `NoteRecord` |
| 写入语义 | 复用当前 `update_note_content`，保持 revision 和 sync changes 行为 |

## 建议纳入范围

- 新增 `POST /notes/{note_uuid}` 作为编辑笔记内容入口。
- `note_uuid` 接受完整 32 位 hex ID 或 8 位 hex 短 ID。
- 8 位短 ID 必须唯一匹配，否则返回 `409 ambiguous_note_reference`。
- 请求体继续使用 `content` 和可选 `device_id`。
- 同步维护路由、handler OpenAPI 标注、`src/api_doc.rs` 注册和自动化测试。
- 保持已有 `PATCH /notes/{note_ref}` 行为不变，避免破坏已有客户端。

## 建议不纳入范围

- 不修改 note 创建、删除、归档、tag、revision 查询接口。
- 不改变数据库 schema。
- 不做字段、标签、角色等元数据编辑。
- 不改变已有短前缀解析能力在其他接口中的规则。
- 不移除或废弃现有 `PATCH /notes/{note_ref}`。

## 需要决策的问题

| 编号 | 问题 | 推荐方案 | 备选方案 |
| --- | --- | --- | --- |
| Q1 | `POST /notes/{note_uuid}` 是新增兼容入口，还是替换现有 `PATCH /notes/{note_ref}`？ | 新增兼容入口，保留 PATCH | 将 PATCH 标记为非推荐，但仍保留一段时间 |
| Q2 | `note_uuid` 是否只允许 8 位或 32 位？ | 只允许 8 位和 32 位，契合本次 API 命名 | 继续复用现有 4 位以上唯一前缀能力 |
| Q3 | 请求体是否复用现有 `UpdateNoteRequest`？ | 复用 `{ content, device_id }` | 新建 DTO，仅保留 `content` |
| Q4 | 8 位短 ID 匹配多条 note 时如何处理？ | 返回 `409 ambiguous_note_reference` | 返回 `422 invalid_note_reference`，强制客户端改用完整 ID |

## 初步验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `POST /notes/{full_id}` 可更新 note content |
| A2 | `POST /notes/{8_char_id}` 在唯一匹配时可更新 note content |
| A3 | 更新后会生成新的 `note_revisions` 记录 |
| A4 | 更新后 `notes.current_revision_id` 指向新 revision |
| A5 | 请求体 content 为空或空白时返回 validation error |
| A6 | `note_uuid` 非 hex 时返回 `422 invalid_note_reference` |
| A7 | `note_uuid` 不是 8 位或 32 位时按决策结果返回错误 |
| A8 | `note_uuid` 不存在时返回 `404 record_not_found` |
| A9 | 8 位短 ID 匹配多条 note 时返回约定错误 |
| A10 | OpenAPI JSON 包含 `POST /notes/{note_uuid}` |
