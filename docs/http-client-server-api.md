# HTTP Client 接入 Server API 需求清单

日期：2026.05.02

## 需求背景

当前 `zembra-cli add` 每次新增笔记都会加载配置、检查 SQLite 数据库、打开数据库连接、创建 `ZembraRepository`，再执行 note、revision、field、tag、note_tags 的写入和查询。普通一次性 CLI 调用会反复创建数据库会话并提交事务，交互式 `run` 虽然复用连接，但仍依赖本地 Repository 写入链路。

目标是在 CLI 侧后续增加 HTTP client 模式，把高频新增笔记请求交给常驻 HTTP server 处理。server 需要复用现有 Repository 语义，对外提供稳定 API，让 CLI 不再直接为每条 note 建立本地数据库会话。

## 当前功能基线

| 功能 | 现有入口 | 当前行为 |
| --- | --- | --- |
| 初始化数据库 | `zembra-cli init` | 创建或复用本地 SQLite，并写入配置 |
| 新增笔记 | `zembra-cli add CONTENT --field FIELD --tags TAGS --role ROLE` | 创建 note、初始 revision、field、tags、note_tags，返回 JSON |
| 交互式新增 | `zembra-cli run` | 复用一个 Repository 循环新增 note，默认 field 为 `inbox` |
| 列出 tags | `zembra-cli list tags` | 按名称升序返回 tag 名称 |
| 列出 fields | `zembra-cli list fields` | 按名称升序返回 field 名称 |
| note ID 解析 | Repository | 支持完整 32 位 hex ID 或至少 4 位唯一前缀 |
| note 更新 | Repository | 更新 content，写入新 revision |
| note 归档/删除 | Repository | 设置 `archived_at` 或 `deleted_at`，删除为软删除 |
| revision 查询 | Repository | 按创建时间返回指定 note 的 revisions |

## Server API 设计原则

| 原则 | 要求 |
| --- | --- |
| Repository 语义一致 | server 行为必须对齐 `ZembraRepository`，包括 field/tag 自动创建、revision 写入、软删除过滤 |
| 写入优先优化 | 第一阶段优先实现新增 note，解决 CLI 高频写入性能问题 |
| JSON 合同稳定 | 请求和响应使用 JSON，字段名沿用现有 Pydantic record 模型 |
| 错误可映射 | server 错误需要能映射到 CLI 的自然语言失败信息和非 0 退出码 |
| 服务端持有数据库连接 | server 启动时加载配置并管理数据库连接或连接池，单条请求不要求 CLI 打开 SQLite |
| 保留本地兼容路径 | API 不要求改变 SQLite schema，不引入与现有本地模式冲突的数据结构 |

## 必须实现 API

### 1. 健康检查

| 项目 | 说明 |
| --- | --- |
| Method | `GET` |
| Path | `/health` |
| 用途 | CLI 判断 server 是否可用 |
| 成功状态 | `200 OK` |

响应：

```json
{
  "status": "ok",
  "service": "zembra-server",
  "database_initialized": true
}
```

### 2. 创建笔记

| 项目 | 说明 |
| --- | --- |
| Method | `POST` |
| Path | `/notes?workspace_id={workspace_id}` |
| 用途 | 替代当前 `zembra-cli add` 的本地写入链路 |
| 成功状态 | `201 Created` |

所有 notes CRUD 和 notes 派生查询接口都必须通过 URL query 显式传入完整 `workspace_id`。客户端应先调用 `GET /workspaces` 获取完整 workspace id，再把该值作为 `workspace_id` 传给 notes 接口。缺失、非法、不存在、已归档或已删除的 workspace 都返回 `404 record_not_found`；服务端不会自动选择默认 workspace，也不会使用 legacy fixed workspace fallback。

请求：

```json
{
  "content": "note body",
  "field": "work",
  "tags": ["python", "cli"],
  "role": "Human",
  "device_id": null
}
```

字段要求：

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `content` | `string` | 是 | note 正文，允许空字符串以外的任意文本 |
| `field` | `string \| null` | 否 | field 名称；CLI `add` 当前必传，交互式默认 `inbox` |
| `tags` | `string[]` | 否 | 已由 client 去重、去空白后的 tag 名称列表；server 仍需防御性去重 |
| `role` | `"Human" \| "Agent"` | 否 | 默认 `Human`，对齐现有 schema |
| `device_id` | `string \| null` | 否 | 写入初始 revision 的设备 ID，当前 CLI 可先不传 |

响应：

```json
{
  "note": {
    "id": "32-char-hex",
    "content": "note body",
    "role": "Human",
    "field_id": "field-id",
    "created_at": 123,
    "updated_at": 123,
    "archived_at": null,
    "deleted_at": null,
    "current_revision_id": "revision-id"
  },
  "metadata": {
    "field": "work",
    "tags": ["python", "cli"],
    "role": "Human"
  }
}
```

服务端行为：

| 步骤 | 要求 |
| --- | --- |
| 1 | 校验 role，只接受 `Human` 和 `Agent` |
| 2 | field 非空时按 name 查询或创建 field |
| 3 | 创建 note，并写入初始 note_revision |
| 4 | 为每个 tag 查询或创建 tag，并写入 note_tags，重复关联保持幂等 |
| 5 | 返回 `NoteRecord.model_dump()` 等价结构和用户语义 metadata |

## 推荐实现 API

### 3. 批量创建笔记

| 项目 | 说明 |
| --- | --- |
| Method | `POST` |
| Path | `/notes/batch?workspace_id={workspace_id}` |
| 用途 | 为交互式输入、脚本批量导入和后续 agent 批量写入降低 HTTP 往返成本 |
| 成功状态 | `201 Created` |

请求：

```json
{
  "items": [
    {
      "content": "first note",
      "field": "inbox",
      "tags": ["idea"],
      "role": "Human",
      "device_id": null
    }
  ]
}
```

响应：

```json
{
  "notes": [
    {
      "note": {},
      "metadata": {
        "field": "inbox",
        "tags": ["idea"],
        "role": "Human"
      }
    }
  ]
}
```

行为要求：同一请求内建议使用单个事务；任一 item 校验失败时默认整批失败，避免部分写入造成 CLI 重试语义复杂。

### 4. 列出 fields

| 项目 | 说明 |
| --- | --- |
| Method | `GET` |
| Path | `/fields` |
| 用途 | 替代 `zembra-cli list fields` |
| Query | `limit` 可选，默认 5；`all` 可选，默认 false |
| 成功状态 | `200 OK` |

响应：

```json
{
  "fields": [
    {
      "id": "field-id",
      "name": "work",
      "created_at": 123
    }
  ],
  "names": ["work"]
}
```

### 5. 列出 tags

| 项目 | 说明 |
| --- | --- |
| Method | `GET` |
| Path | `/tags` |
| 用途 | 替代 `zembra-cli list tags` |
| Query | `limit` 可选，默认 5；`all` 可选，默认 false |
| 成功状态 | `200 OK` |

响应：

```json
{
  "tags": [
    {
      "id": "tag-id",
      "name": "python",
      "created_at": 123
    }
  ],
  "names": ["python"]
}
```

## 后续兼容 API

这些接口不是解决新增笔记性能的首要条件，但建议 server agent 保留路由设计，方便 CLI 后续完全切换到 HTTP 模式。

| Method | Path | 用途 | 对应 Repository |
| --- | --- | --- | --- |
| `GET` | `/notes?workspace_id={workspace_id}` | 按更新时间列出指定 workspace 下的可见 notes | `list_notes` |
| `POST` | `/notes/recent?workspace_id={workspace_id}` | 按更新时间列出指定 workspace 下的 recent notes | `list_recent_notes` |
| `GET` | `/notes/stats/daily-counts?workspace_id={workspace_id}` | 统计指定 workspace 下最近 30 天可见 notes 数量 | `daily_note_counts_since` |
| `GET` | `/notes/by-date?date={date}&workspace_id={workspace_id}` | 按本地日期列出指定 workspace 下的可见 notes | `list_visible_notes_created_between` |
| `GET` | `/random/notes?n={n}&workspace_id={workspace_id}` | 随机返回指定 workspace 下的可见 notes | `list_random_notes` |
| `GET` | `/random/tags?n={n}&count={count}&workspace_id={workspace_id}` | 随机返回指定 workspace 下的 tag 分组 notes | `random_tags` |
| `GET` | `/random/fields?n={n}&count={count}&workspace_id={workspace_id}` | 随机返回指定 workspace 下的 field 分组 notes | `random_fields` |
| `GET` | `/notes/{note_ref}?workspace_id={workspace_id}` | 在指定 workspace 内按完整 ID 或唯一前缀读取 note | `get_note_by_ref` |
| `PATCH` | `/notes/{note_ref}?workspace_id={workspace_id}` | 在指定 workspace 内更新 note content 并写入 revision | `update_note` |
| `POST` | `/notes/{note_ref}/archive?workspace_id={workspace_id}` | 在指定 workspace 内归档 note | `archive_note` |
| `DELETE` | `/notes/{note_ref}?workspace_id={workspace_id}` | 在指定 workspace 内软删除 note | `delete_note` |
| `GET` | `/notes/{note_ref}/tags?workspace_id={workspace_id}` | 查询指定 workspace 内 note 关联 tags | `list_note_tags` |
| `PUT` | `/notes/{note_ref}/tags/{tag_name}?workspace_id={workspace_id}` | 在指定 workspace 内给 note 添加 tag | `add_tag_to_note` |
| `DELETE` | `/notes/{note_ref}/tags/{tag_name}?workspace_id={workspace_id}` | 在指定 workspace 内移除 note tag 关联 | `remove_tag_from_note` |
| `GET` | `/notes/{note_ref}/revisions?workspace_id={workspace_id}` | 查询指定 workspace 内 note revisions | `list_note_revisions` |

## 错误响应合同

所有错误响应建议统一为：

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
| 请求 JSON 格式错误 | `400` | `invalid_json` |
| 字段校验失败 | `422` | `validation_error` |
| note ref 少于 4 位 | `422` | `note_reference_too_short` |
| note ref 非 hex | `422` | `invalid_note_reference` |
| workspace 缺失、非法、不存在、已归档或已删除 | `404` | `record_not_found` |
| note 不存在 | `404` | `record_not_found` |
| note ref 匹配多个 note | `409` | `ambiguous_note_reference` |
| 数据库未初始化 | `503` | `database_not_initialized` |
| SQLite 写入失败 | `500` | `database_error` |

## 验收标准

| 编号 | 标准 |
| --- | --- |
| A1 | `POST /notes?workspace_id={workspace_id}` 创建结果与当前 `zembra-cli add` 本地写入结果结构一致 |
| A2 | 创建 note 时自动写入一条 `note_revisions`，并更新 `current_revision_id` |
| A3 | field/tag 不存在时自动创建，重复 tag 不产生重复关联 |
| A4 | `role` 默认值和校验规则与 CLI 当前 `parse_role_value` 输出一致 |
| A5 | `GET /fields` 和 `GET /tags` 的排序与当前 Repository 一致，均按 name 升序 |
| A6 | server 启动后由 server 管理数据库连接，CLI 调用创建 note 时不直接打开 SQLite |
| A7 | 错误响应可被 CLI 映射为当前风格的自然语言错误 |
| A8 | 批量创建接口在同一请求内保持事务一致性，失败时不部分写入 |

## 第一阶段推荐交付范围

推荐 server 开发 agent 第一阶段只交付：

1. `GET /health`
2. `POST /notes?workspace_id={workspace_id}`
3. `POST /notes/batch?workspace_id={workspace_id}`
4. `GET /fields`
5. `GET /tags`
6. 统一错误响应结构

这个范围能覆盖 CLI http client 的新增笔记主链路，也能支撑现有 list 子命令迁移到 HTTP。
