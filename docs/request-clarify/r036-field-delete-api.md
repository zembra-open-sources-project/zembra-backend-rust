# r036 field 删除接口需求澄清

日期：2026-06-27

本次需求目标是新增一个 field 删除接口，让客户端可以删除不再被可见 note 使用的 field。接口使用 request body 传递参数，不使用 path 参数或 query 参数传递删除对象。删除前后端必须先判断目标 field 在同一 workspace 下对应的 note 数量是否为 0；如果数量为 0，则删除该 field；如果数量大于 0，则不删除并返回异常状态码。

接口路径确认为 `POST /fields/delete`。请求 body 确认为 `{ "workspace_id": "...", "field_id": "..." }`，其中 `workspace_id` 是 active workspace 的完整 id，`field_id` 是要删除的 field id。该接口不支持通过 field name 删除，不支持 query 参数传参，也不支持批量删除。

field 删除必须限定在 body 指定的 active workspace 内。`workspace_id` 缺失、格式非法、不存在、已归档或已删除时，按既有 workspace 请求口径返回 `404`。`field_id` 缺失或为空属于请求参数错误；`field_id` 格式是否需要 UUID 校验由实现阶段沿用现有 id 校验方式。目标 field 在指定 workspace 下不存在时返回 `404`。

判断 note 数量时使用本仓库 notes 默认可见性口径，只统计同一 workspace 下 `field_id` 匹配且 `deleted_at IS NULL`、`archived_at IS NULL` 的 notes。可见 note 数量为 0 时允许删除 field；可见 note 数量大于 0 时禁止删除 field，并返回 `409 Conflict`。已删除或已归档 note 不阻止 field 删除。

本次删除是对 `fields` 记录的删除，不新增数据库 schema，不修改 migration，不移动 note 到其他 field，不实现 tag 删除，不实现空 field 自动清理任务。由于当前 SQLite 复合外键删除 field 时会触发 `ON DELETE SET NULL` 并可能尝试置空 `notes.workspace_id`，实现允许在删除 field 前只清理同 workspace 下已删除或已归档 notes 的 `field_id`；删除前只允许可见 note 数量为 0，因此接口不会改变当前可见 note 的 field 归属。

本次需求必须同步维护 OpenAPI、DTO、路由、handler、repository 和自动化测试。OpenAPI 需要暴露 `POST /fields/delete`、request body、`200`、`404`、`409`、参数错误和数据库错误响应。测试需要覆盖删除成功、有关联 note 时返回 `409` 且 field 保留、field 不存在返回 `404`、workspace 无效返回 `404`、已归档或已删除 note 不阻止删除，以及 OpenAPI JSON 包含新增 path。

验收标准：`POST /fields/delete` 接收 body 参数并按指定 workspace 删除目标 field；目标 field 下没有可见 note 时返回成功且数据库中 field 被删除；目标 field 下存在至少 1 条可见 note 时返回 `409 Conflict` 且数据库中 field 保留；目标 field 不存在返回 `404`；无效 workspace 返回 `404`；OpenAPI 与测试覆盖新增接口合同。
