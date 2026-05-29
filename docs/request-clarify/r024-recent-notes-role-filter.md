# r024 Recent Notes Role Filter 需求澄清

日期：2026-05-29

## 背景

现有 `POST /notes/recent` 已用于返回最近笔记，并支持 `limit` 和 `note_uuid` 游标。当前新增需求是在复用该接口的基础上，按笔记创建角色筛选最近笔记。

## 仓库现状

| 项目 | 现状 |
| --- | --- |
| 现有接口 | `POST /notes/recent` |
| 请求 DTO | `RecentNotesRequest` 当前包含 `limit` 和 `note_uuid` |
| 查询链路 | `NotesService::recent_notes` 调用 `NotesRepository::list_recent_notes` |
| 内部 role 值 | 创建笔记时只接受 `Human` 和 `Agent` |
| 默认返回条数 | 不传 `limit` 时默认返回 50 条 |
| 可见性过滤 | recent 查询已过滤软删除和归档笔记 |

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| 接口复用 | 继续使用 `POST /notes/recent`，不新增接口 |
| 新增请求字段 | 在 request body 中新增可选 `role` |
| 支持的 role 入参 | `human`、`agent`、`both` |
| 大小写规则 | role 入参大小写不敏感 |
| 默认 role 行为 | 不传 `role` 时等价于 `both` |
| `human` 行为 | 只返回 `role == "Human"` 的笔记 |
| `agent` 行为 | 只返回 `role == "Agent"` 的笔记 |
| `both` 行为 | 返回 Human 和 Agent 两类笔记 |
| 默认条数 | 保持现有默认 50 条，不改为 30 条 |
| 排序规则 | 保持 `updated_at DESC, id DESC` |
| 游标能力 | 保留现有 `note_uuid` 游标能力 |
| 可见性过滤 | 继续只返回未删除、未归档笔记 |

## 非目标

- 不修改 `GET /notes` 行为。
- 不新增 `/notes/recent/by-role` 或其他新路径。
- 不调整 note 创建接口的 role 存储值。
- 不改变 `limit` 范围和默认值。
- 不增加按 tag、field、日期等维度的组合筛选。

## 验收点

| 编号 | 验收内容 |
| --- | --- |
| A1 | `POST /notes/recent` 不传 `role` 时保持现有行为，返回最近最多 50 条可见笔记 |
| A2 | `role: "human"` 返回最近 Human 笔记 |
| A3 | `role: "agent"` 返回最近 Agent 笔记 |
| A4 | `role: "both"` 返回 Human 和 Agent 两类最近笔记 |
| A5 | `role` 大小写不敏感，例如 `Human`、`AGENT`、`Both` 均可匹配 |
| A6 | 非法 `role` 返回 validation error |
| A7 | `limit` 与 `role` 同时存在时，按 role 过滤后的结果应用 limit |
| A8 | `note_uuid` 与 `role` 同时存在时，基于游标返回更旧且符合 role 的笔记 |
| A9 | 已软删除或已归档笔记不会出现在任何 role 过滤结果中 |
| A10 | OpenAPI 文档包含 `RecentNotesRequest.role` |

## 待进入设计阶段的建议

推荐把 role 入参解析为专用枚举或轻量 helper，再传给 repository，避免 SQL 层直接处理大小写和外部入参值。repository 内部继续使用现有存储值 `Human`、`Agent` 查询。
