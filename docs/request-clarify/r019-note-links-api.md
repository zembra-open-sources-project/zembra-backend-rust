# r019 Note Links API 需求澄清

日期：2026-05-21

## 背景

为实现真双链笔记，创建、修改和读取笔记接口需要开始维护 `note_links` 关系。WebUI 和 CLI 负责从正文中解析链接，并通过 request body 把结构化链接结果提交给后端；后端不解析 `content`，只负责校验结构化 links、更新数据库关系、维护同步变更和返回链接 metadata。

## 仓库现状

| 项目 | 当前结论 |
| --- | --- |
| 数据库表 | shared schema 已有 `note_links` 表，包含 `source_note_id`、`target_note_id`、`anchor_text`、`position`、`created_at` |
| 关系查询基础 | schema 已为 `source_note_id` 和 `target_note_id` 建索引，可支持 outgoing links 和 backlinks 查询 |
| 同步枚举 | `sync_changes.entity_type` 已包含 `note_link` |
| 当前后端缺口 | notes 创建、修改、读取链路未维护或返回 `note_links` |
| 当前响应 | `POST /notes` 返回 `NoteResponse`，`GET /notes/{note_ref}` 和 `PATCH /notes/{note_ref}` 返回 `NoteRecord` |
| 当前事务 | 创建和修改 note 已在事务内维护 note、revision、field、tags 和 sync changes |
| 可见性纪律 | notes 相关接口默认只处理未删除、未归档笔记，即 `deleted_at IS NULL` 且 `archived_at IS NULL` |

## 需求理解

本轮需求是升级 note 创建、修改和读取接口，让后端接收客户端解析好的结构化 links，并以未删除、未归档 note 为唯一有效范围维护 `note_links`。创建和修改接口需要返回扩展后的 `NoteResponse`，metadata 包含落库后的 outgoing links；读取接口也返回扩展后的 `NoteResponse`，metadata 同时包含该笔记 link 了谁，以及谁 link 了该笔记。

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| 解析责任 | WebUI / CLI 解析正文链接，后端不解析 `content` |
| 后端责任 | 后端只接收结构化 `links`，负责校验、落库、事务一致性和 sync changes |
| note 范围 | 所有 link 相关创建、修改、查询、校验只处理未删除、未归档 note |
| link target | 使用 `target_note_ref`，支持完整 32 位 note id 或当前唯一前缀规则 |
| target 校验 | target 必须匹配已存在、未删除、未归档 note |
| 自引用 | 不支持自引用，遵守 schema `source_note_id <> target_note_id` 约束 |
| 自动创建 | 不自动创建被引用 note |
| 重复引用 | 同一个 target 在不同 `position` 出现时保留多条 `note_links` |
| 创建语义 | `POST /notes` 的 `links` 未传按空链接处理 |
| 修改语义 | `PATCH /notes/{note_ref}` 的 `links` 未传则保持现有 outgoing links 不变 |
| 清空语义 | `PATCH /notes/{note_ref}` 传 `links: []` 时清空该 note 的 outgoing links |
| 替换语义 | `PATCH /notes/{note_ref}` 传入 links 时整体替换该 note 的 outgoing links |
| 创建响应 | `POST /notes` 返回扩展 `NoteResponse`，metadata 包含 outgoing links，backlinks 正常为空 |
| 修改响应 | `PATCH /notes/{note_ref}` 返回扩展 `NoteResponse`，metadata 包含 outgoing links 和 backlinks |
| 读取响应 | `GET /notes/{note_ref}` 返回扩展 `NoteResponse`，metadata 包含 outgoing links 和 backlinks |
| link 标识 | 响应中的 source/target note 均使用完整 uuid |

## 建议纳入范围

- 扩展 `CreateNoteRequest`，增加结构化 `links` 字段。
- 扩展 `UpdateNoteRequest`，增加可选结构化 `links` 字段。
- 新增 note link DTO / model，用于请求、响应和 OpenAPI schema。
- 创建 note 时，在同一事务内写入 note、initial revision、field/tags、note_links 和 sync changes。
- 修改 note 时，在同一事务内更新 content/revision、field/tags、整体替换 links 和 sync changes。
- 读取 note 时查询 outgoing links 和 backlinks，并组装为 `NoteResponse`。
- 创建、修改和读取接口统一只返回未删除、未归档 note 之间的 link 关系。
- 扩展 sync remote apply 逻辑，支持 `note_link` 的关系变更重放。
- 补充 repository、service/API 和 OpenAPI 自动化测试。

## 建议不纳入范围

- 不在后端解析 `[[...]]`、Markdown、富文本或任何编辑器语法。
- 不根据正文自动推导 links。
- 不自动创建 target note。
- 不支持引用已删除或已归档 note。
- 不支持自引用。
- 不改变 note ref 的基础解析规则。
- 不实现前端或 CLI 解析逻辑。
- 不实现独立的 link 编辑页面。
- 不清理无引用的 note、tag 或 field 实体。

## API 输入建议

创建请求示例：

```json
{
  "content": "正文内容",
  "field": "work",
  "tags": ["rust"],
  "role": "Human",
  "links": [
    {
      "target_note_ref": "abcd1234",
      "anchor_text": "相关笔记",
      "position": 12
    }
  ]
}
```

修改请求示例：

```json
{
  "content": "更新后的正文",
  "links": []
}
```

## API 响应建议

`NoteResponse.metadata` 扩展为：

```json
{
  "field": "work",
  "tags": ["rust"],
  "role": "Human",
  "outgoing_links": [
    {
      "id": "link-row-uuid",
      "source_note_id": "source-note-uuid",
      "target_note_id": "target-note-uuid",
      "anchor_text": "相关笔记",
      "position": 12,
      "created_at": 2
    }
  ],
  "backlinks": [
    {
      "id": "link-row-uuid",
      "source_note_id": "other-note-uuid",
      "target_note_id": "source-note-uuid",
      "anchor_text": "引用当前笔记的文本",
      "position": 8,
      "created_at": 2
    }
  ]
}
```

## 验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `POST /notes` 可接收 links 并写入 `note_links` |
| A2 | `POST /notes` 未传 links 时创建成功且 outgoing links 为空 |
| A3 | `PATCH /notes/{note_ref}` 未传 links 时保留原 outgoing links |
| A4 | `PATCH /notes/{note_ref}` 传 `links: []` 时清空 outgoing links |
| A5 | `PATCH /notes/{note_ref}` 传 links 时整体替换 outgoing links |
| A6 | link target 不存在、已删除或已归档时返回错误，不写入部分数据 |
| A7 | 自引用 link 返回错误，不写入部分数据 |
| A8 | 同一个 target 不同 position 会保留多条 link |
| A9 | `GET /notes/{note_ref}` 返回 `NoteResponse`，metadata 包含 outgoing links 和 backlinks |
| A10 | 创建、修改、读取响应中的 link source/target 均使用完整 uuid |
| A11 | links 与 note content/revision/field/tags 更新保持事务一致 |
| A12 | sync changes 能记录并重放 note_link 关系变化 |
| A13 | OpenAPI JSON 包含新增 request/response schema |

