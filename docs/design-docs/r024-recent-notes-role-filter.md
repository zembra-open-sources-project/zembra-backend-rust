# r024 Recent Notes Role Filter 设计文档

日期：2026-05-29

关联需求澄清：`docs/request-clarify/r024-recent-notes-role-filter.md`

## 核心功能（WHAT）

扩展现有 `POST /notes/recent`，在请求体中新增可选 `role` 字段，用于按笔记创建角色筛选最近笔记。`role` 入参解析为专用枚举，支持大小写不敏感的 `human`、`agent`、`both`，不传时等价于 `both`。接口默认返回条数保持 50，继续支持 `limit`、`note_uuid` 游标、未删除未归档过滤和 `updated_at DESC, id DESC` 排序。

### 需求背景（WHY）

前端需要在最近笔记列表中分别展示人类创建、Agent 创建或两者混合的笔记。当前 recent notes 查询已经具备最近列表、limit、游标和可见性过滤能力，最小改动路径是在该接口上增加 role 过滤，避免新增重复路径和重复查询链路。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 复用 recent API | 保持 `POST /notes/recent` 作为唯一入口 |
| 增加 role 过滤 | 请求体新增可选 `role` 字段 |
| 专用枚举解析 | 将外部 role 入参解析为内部枚举，避免字符串在 service 和 repository 间散传 |
| 大小写不敏感 | `human`、`Human`、`HUMAN` 等写法都能匹配 |
| 保持默认行为 | 不传 `role` 时等价于 `both`，不传 `limit` 时仍返回最多 50 条 |
| 保持游标能力 | `note_uuid` 与 `role` 可组合使用，返回游标之后更旧且符合 role 的笔记 |
| 同步 API 合同 | OpenAPI 中暴露 `RecentNotesRequest.role` |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| DTO | 扩展 `RecentNotesRequest`，新增可选 `role` 字段 |
| Role 枚举 | 新增 recent notes 专用 role filter 枚举，负责大小写不敏感解析和内部 SQL 值映射 |
| Service | `NotesService::recent_notes` 解析 role，默认 `both`，继续使用默认 limit 50 |
| Repository | `NotesRepository::list_recent_notes` 支持可选 role filter，并与游标条件组合 |
| OpenAPI | `RecentNotesRequest` schema 包含 role 字段和枚举说明 |
| Tests | 覆盖 human、agent、both、默认 both、大小写不敏感、非法 role、limit 和 cursor 组合 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 新增路径 | 不新增 `/notes/recent/by-role` 或其他接口 |
| 默认条数 | 不把默认返回条数从 50 改成 30 |
| 创建 role 值 | 不修改现有 `Human`、`Agent` 存储值和创建校验 |
| 其他筛选维度 | 不增加 tag、field、日期等组合筛选 |
| 数据库 schema | 不新增表、字段、索引或迁移 |

## 实现流程（HOW）

### 架构落点

| 层级 | 文件/模块 | 设计 |
| --- | --- | --- |
| DTO | `src/dto/notes.rs` | `RecentNotesRequest` 新增 `role: Option<String>`，保留 validator 对 `limit` 和 `note_uuid` 的校验 |
| Role 枚举 | `src/dto/notes.rs` 或 `src/services/notes.rs` | 新增 `RecentNotesRoleFilter`，枚举值为 `Human`、`Agent`、`Both` |
| Service | `src/services/notes.rs` | 在 `recent_notes` 中调用枚举解析；默认 `Both`；将 role filter 传给 repository |
| Repository | `src/repositories/notes/core.rs` | `list_recent_notes(limit, note_uuid, role_filter)` 在 SQL 中追加 role 条件 |
| Tests | `src/repositories/notes/tests.rs`、`tests/notes_query_routes.rs` | 增加仓储和路由级行为覆盖 |

### API 合同

| 项目 | 内容 |
| --- | --- |
| Method | `POST` |
| Path | `/notes/recent` |
| Request Body | `RecentNotesRequest` |
| Response Body | `ListNotesResponse` |
| Tag | `notes` |

请求体示例：

```json
{
  "role": "agent",
  "limit": 50,
  "note_uuid": "0123456789abcdef0123456789abcdef"
}
```

字段说明：

| 字段 | 类型 | 必填 | 默认值 | 约束 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `limit` | integer | 否 | 50 | 1 到 100 | 最大返回笔记条数 |
| `note_uuid` | string | 否 | 无 | 完整 32 位 hex note ID | 翻页游标，返回该 note 之后更旧且符合 role 的笔记 |
| `role` | string | 否 | `both` | 大小写不敏感的 `human`、`agent`、`both` | 笔记创建角色过滤 |

### Role 枚举设计

| 枚举值 | 外部入参 | SQL 过滤值 | 查询语义 |
| --- | --- | --- | --- |
| `Human` | `human`，大小写不敏感 | `Human` | 只返回 Human 笔记 |
| `Agent` | `agent`，大小写不敏感 | `Agent` | 只返回 Agent 笔记 |
| `Both` | `both`，大小写不敏感或不传 | 无 | 不追加 role 条件 |

实现约束：

| 约束 | 说明 |
| --- | --- |
| 解析位置 | 在 service 层将 request role 解析为专用枚举 |
| 非法值 | 返回 `422 validation_error` |
| SQL 入参 | repository 不接收原始外部字符串，只接收已解析枚举 |
| 大小写处理 | 使用 ASCII 小写归一化即可，因为支持值均为 ASCII |
| 文档字符串 | 新增函数和枚举成员按项目规则补充注释 |

### 查询规则

| 场景 | 查询规则 |
| --- | --- |
| 无 role | 保持当前 recent notes 查询行为 |
| `role = both` | 与无 role 一致，不追加 role 条件 |
| `role = human` | 在现有可见性条件和游标条件基础上追加 `role = 'Human'` |
| `role = agent` | 在现有可见性条件和游标条件基础上追加 `role = 'Agent'` |
| `role + limit` | 先按 role 过滤，再按排序截断到 limit |
| `role + note_uuid` | 游标 note 仍必须是可见 note；结果只返回比游标更旧且符合 role 的 note |

### 错误响应

| 场景 | HTTP 状态 | code |
| --- | --- | --- |
| JSON 格式错误 | `400` | `invalid_json` |
| `limit` 校验失败 | `422` | `validation_error` |
| `note_uuid` 格式无效 | `422` | `validation_error` |
| `role` 不属于 human、agent、both | `422` | `validation_error` |
| `note_uuid` 不存在、已删除或已归档 | `404` | `record_not_found` |
| SQLite 访问失败 | `500` | `database_error` |

## 测试用例

### 编译检查

| 用例 | 预期 |
| --- | --- |
| `cargo fmt --check` | 格式检查通过 |
| `cargo check` | 类型检查通过 |
| `cargo test` | 单元测试和集成测试通过 |
| `cargo clippy -- -D warnings` | 无新增 warning |

### 自动化行为检查

| 用例 | 预期 |
| --- | --- |
| 默认 role | `POST /notes/recent {}` 保持最近最多 50 条可见笔记 |
| human 过滤 | `role: "human"` 只返回 `Human` note |
| agent 过滤 | `role: "agent"` 只返回 `Agent` note |
| both 过滤 | `role: "both"` 返回 Human 和 Agent |
| 大小写不敏感 | `Human`、`AGENT`、`Both` 均可解析 |
| 非法 role | `role: "robot"` 返回 `422 validation_error` |
| role + limit | limit 应用于 role 过滤后的结果 |
| role + cursor | 返回游标之后更旧且符合 role 的笔记 |
| 隐藏笔记过滤 | 软删除和归档 note 不出现在任何 role 过滤结果 |
| OpenAPI | `/api-docs/openapi.json` 中 `RecentNotesRequest` 包含 `role` |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| curl 调用 `role=agent` | 返回 `200 OK`，notes 中均为 `Agent` |
| Swagger UI 查看 | `/notes/recent` request body 显示 `role` 字段 |
