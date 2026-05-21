# r017 PATCH Note Taxonomy API 需求澄清

日期：2026-05-21

## 背景

当前 `PATCH /notes/{note_ref}` 只支持修改 note content，并会写入新的 revision。用户发现编辑笔记时还需要同步修改 note 的 tags 和 field，因此本需求希望扩展现有 PATCH API，让客户端可以一次性提交笔记正文、field 和 tags 的编辑结果。

## 仓库现状

| 项目 | 当前结论 |
| --- | --- |
| 已有编辑 API | `PATCH /notes/{note_ref}` |
| 当前请求体 | `UpdateNoteRequest { content, device_id }` |
| 当前编辑行为 | 更新 `notes.content`，新增 `note_revisions`，更新 `current_revision_id`，记录 note 和 revision sync changes |
| 当前 field 能力 | 创建 note 时可传 `field`，服务端按 name 查询或创建 field |
| 当前 tags 能力 | 创建 note 时可传 `tags`，服务端 trim、去空、去重，不存在则创建 tag |
| 当前单独 tag API | `PUT /notes/{note_ref}/tags/{tag_name}` 添加 tag，`DELETE /notes/{note_ref}/tags/{tag_name}` 移除 tag |
| 当前缺口 | `PATCH /notes/{note_ref}` 不能修改 `field_id`，也不能一次性整体替换 note tags |
| 当前同步范围 | sync changes 已覆盖 `note`、`note_revision`、`field`、`tag`、`note_tag` |
| 当前 OpenAPI | 已注册 `PATCH /notes/{note_ref}`，但 request body 未声明 field/tags |

## 需求理解

扩展现有 `PATCH /notes/{note_ref}`，让它在严格编辑正文的同时支持附带更新 field 和 tags。客户端必须继续提交 content；如提交 field 或 tags，后端在同一事务中更新 note 的 field 归属和 tag 关联。实现继续复用现有 note ref 解析、field/tag 自动创建、tag 清洗去重、revision 写入和 sync changes 机制。

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| API 路径 | 继续使用 `PATCH /notes/{note_ref}` |
| content | 严格必选，复用当前更新正文并写入 revision 的语义 |
| tags | 使用整体替换语义，请求传入 tags 后 note 最终 tags 等于清洗后的请求 tags |
| field 未传 | 不修改当前 field |
| field 为字符串 | 按 field 名称查询或创建，并更新 note.field_id |
| field 为 null | 使用默认 field `inbox`，禁止清空 field |
| 返回 body | 不改变，继续返回当前 `NoteRecord` |
| 事务边界 | content、field、tags 更新必须在同一事务中完成 |

## 建议纳入范围

- 扩展 `UpdateNoteRequest`，支持 `field` 和 `tags` 字段。
- `field` 使用名称传入，非空时按现有创建语义查询或创建 field。
- `tags` 使用字符串数组传入，按现有创建语义 trim、去空、去重，不存在则创建 tag。
- `tags` 采用整体替换语义：请求传入 tags 后，note 最终 tags 等于请求中的清洗后 tags。
- `field` 采用可选设置语义：不传不修改，传字符串则设置为该 field，传 `null` 则设置为默认 `inbox`。
- `content` 保持必填，继续写入新的 revision。
- field/tags 变化和 content 变化在同一事务中完成。
- 同步维护 OpenAPI 标注、`src/api_doc.rs` schema、repository/service/API 测试。

## 建议不纳入范围

- 不新增新的 HTTP path。
- 不移除现有 `PUT/DELETE /notes/{note_ref}/tags/{tag_name}`。
- 不改变 note ref 解析规则。
- 不新增 field 或 tag 的重命名 API。
- 不删除不再关联的 tag 或 field 实体，只维护 note 关联关系。
- 不允许通过 PATCH 清空 note field。
- 不调整随机 notes/tags/fields、recent notes、sync config 等无关接口。

## 已决策问题

| 编号 | 问题 | 推荐方案 | 备选方案 |
| --- | --- | --- | --- |
| Q1 | `tags` 在 PATCH 中采用什么语义？ | 已确认：整体替换 | 不采用增量语义 |
| Q2 | `field: null` 如何处理？ | 已确认：设置为默认 `inbox` | 不清空 field |
| Q3 | `content` 是否继续必填？ | 已确认：严格必选 | 不支持只更新 field/tags |
| Q4 | 响应体是否需要返回 metadata？ | 已确认：保持 `NoteRecord` | 不改为 `NoteResponse` |
| Q5 | 只改 field/tags 时是否写 revision？ | 已确认：不支持只改 field/tags，content 必选并复用已有 revision 语义 | 不引入 content 可选分支 |

## 初步验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `PATCH /notes/{note_ref}` 传入 `content`、`field`、`tags` 后同时更新正文、field 和 tags |
| A2 | field 不存在时自动创建，并写入 note 的 `field_id` |
| A3 | tags 不存在时自动创建，并写入 `note_tags` |
| A4 | tags 会 trim、去空、去重 |
| A5 | tags 整体替换时会移除请求中不存在的旧关联 |
| A6 | 未传 field 时不修改当前 field |
| A7 | `field: null` 时设置为默认 field `inbox` |
| A8 | content 为空或纯空白时返回 validation error |
| A9 | note ref 不存在、格式错误、冲突时沿用现有错误合同 |
| A10 | field/tags/content 更新在同一事务中完成 |
| A11 | sync changes 记录 note、field/tag 创建和 note_tag attach/detach |
| A12 | OpenAPI JSON 中 `UpdateNoteRequest` 包含新增字段 |
