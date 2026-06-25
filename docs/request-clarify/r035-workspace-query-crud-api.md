# r035 CRUD API workspace query 参数需求澄清

日期：2026-06-25

当前 notes CRUD REST API 仍在 repository 层隐式绑定固定 `DEFAULT_WORKSPACE_ID = 00000000-0000-4000-8000-000000000300`。r033 已经把真实用户文件数据库中的 legacy fixed workspace 迁移为本地随机 UUID，并新增 `GET /workspaces` 作为 workspace 发现入口；在这个状态下，CRUD API 继续使用固定默认 workspace 会破坏 workspace 隔离，导致真实 workspace 下的旧数据查不到，新写入数据又落回固定 legacy workspace。

本次需求目标是对 notes CRUD 和 notes 派生查询 REST API 做严格正向修改：客户端必须在 URL query 中显式传入完整 `workspace_id`，后端必须把这个 `workspace_id` 作为本次请求的唯一 workspace 上下文。后端不再自动选择默认 workspace，不再使用 legacy fixed workspace fallback，不再为了旧测试或旧调用方式保留无 `workspace_id` 的通过路径。任何继续隐式读取 `DEFAULT_WORKSPACE_ID` 作为 CRUD API workspace 上下文的路径都视为未完成。

纳入范围包括所有 notes 资源和 notes 派生查询接口：`GET /notes`、`POST /notes`、`POST /notes/batch`、`POST /notes/recent`、`GET /notes/stats/daily-counts`、`GET /notes/by-date`、`GET /random/notes`、`GET /random/tags`、`GET /random/fields`、`GET /notes/{note_ref}`、`PATCH /notes/{note_ref}`、`DELETE /notes/{note_ref}`、`POST /notes/{note_ref}/archive`、`GET /notes/{note_ref}/tags`、`PUT /notes/{note_ref}/tags/{tag_name}`、`DELETE /notes/{note_ref}/tags/{tag_name}` 和 `GET /notes/{note_ref}/revisions`。这些接口都必须要求 query 参数 `workspace_id`，并且所有 note_ref 解析、note 查询、note 创建、note 更新、归档、删除、revision、tag、field、note_link、sync_change payload 都必须使用 query 中的 `workspace_id`。

不纳入范围包括 `GET /workspaces`、`/sync/*`、`/sync/config`、`/health`、`/api-docs/openapi.json` 和 `/swagger-ui`。`GET /workspaces` 是 workspace 发现入口，不绑定单个 workspace；sync、health 和 OpenAPI/Swagger 自身接口不是本次 CRUD API workspace query 改造对象。

`workspace_id` 的校验规则为严格 required。缺失、格式非法、格式合法但不存在、workspace 已归档或已删除，都返回 `404`。当前 `zembra-schema` 的 `workspaces` 表确实包含 `archived_at` 和 `deleted_at` 字段，因此 CRUD API 的 workspace 上下文必须限定为存在且 `archived_at IS NULL AND deleted_at IS NULL` 的 active workspace。归档或删除的 workspace 在 CRUD API 中按不存在处理。

`note_ref` 的 full id 或 prefix 只在 query 指定的 workspace 内解析。跨 workspace 的同前缀 note 不参与歧义判断；同一个 workspace 内匹配多条才返回 ambiguous note reference。创建 note 时不会自动创建 workspace；只有 query 指定的 active workspace 已存在时，才允许创建 note、field、tag、note_revision、note_tag、note_link 和对应 sync_change。

本次需求必须同步更新 OpenAPI 和 REST API Markdown 文档。所有纳入范围的 handler 的 `#[utoipa::path]` 必须把 `workspace_id` 标为 required query 参数；`docs/http-client-server-api.md` 必须把调用方式更新为带 `workspace_id` 的 URL query。测试必须按新合同重写，不保留旧默认 workspace 调用方式的兼容断言。

验收标准：所有纳入范围的 REST API 在缺失、非法、不存在、归档或删除 workspace 时返回 `404`；传入 active workspace 时，所有读写和关联操作只作用于该 workspace；note_ref 只在指定 workspace 内解析；sync_change payload 中的 `workspace_id` 来自请求 query；OpenAPI JSON 暴露 required `workspace_id` query 参数；`docs/http-client-server-api.md` 与新接口合同一致；旧的无 `workspace_id` CRUD 调用不再通过。
