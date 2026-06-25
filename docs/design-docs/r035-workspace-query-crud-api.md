# r035 CRUD API workspace query 参数设计文档

日期：2026-06-25

需求澄清文档：`docs/request-clarify/r035-workspace-query-crud-api.md`

## 核心功能（WHAT）

将 notes CRUD 和 notes 派生查询 REST API 的 workspace 上下文从后端隐式固定常量改为客户端显式 query 参数。所有纳入范围的 API 都要求 `workspace_id`，并在 handler、service、repository、OpenAPI、测试和 REST API 文档中统一执行该合同。

### 需求背景（WHY）

r033 已经引入真实 workspace 隔离机制，文件数据库启动时会把 legacy fixed workspace 迁移为随机 UUID，并通过 `GET /workspaces` 暴露可用 workspace。当前 CRUD API 继续绑定 `DEFAULT_WORKSPACE_ID` 会让 API 与真实 workspace 身份脱节，表现为旧数据查不到、新数据写回 legacy workspace、同步 payload 带错 workspace id。workspace 隔离已经成为 API 的业务边界，因此 REST API 必须由请求显式指定 workspace。

### 需求目标（GOAL）

所有 notes CRUD 和 notes 派生查询接口都通过 URL query 接收 required `workspace_id`。后端校验该 workspace 必须存在且未归档、未删除；任何缺失、非法、不存在、归档或删除 workspace 都返回 `404`。通过校验后，请求全链路只使用该 `workspace_id` 进行 note_ref 解析、读写、关联、revision、taxonomy 和 sync_change payload 生成。

### 范围边界

纳入范围：notes REST handler、notes service、notes repository、taxonomy helper、note tag helper、note link helper、note revision helper、sync payload 生成、OpenAPI path params、`docs/http-client-server-api.md` 和相关自动化测试。接口范围包括 `GET /notes`、`POST /notes`、`POST /notes/batch`、`POST /notes/recent`、`GET /notes/stats/daily-counts`、`GET /notes/by-date`、`GET /random/notes`、`GET /random/tags`、`GET /random/fields`、`GET /notes/{note_ref}`、`PATCH /notes/{note_ref}`、`DELETE /notes/{note_ref}`、`POST /notes/{note_ref}/archive`、`GET /notes/{note_ref}/tags`、`PUT /notes/{note_ref}/tags/{tag_name}`、`DELETE /notes/{note_ref}/tags/{tag_name}` 和 `GET /notes/{note_ref}/revisions`。

不纳入范围：`GET /workspaces`、`/sync/*`、`/sync/config`、`/health`、OpenAPI/Swagger 自身接口、workspace 创建 API、workspace 重命名 API、跨 workspace 移动 note、旧默认 workspace fallback、旧测试兼容路径。`DEFAULT_WORKSPACE_ID` 可以继续服务于非本次范围内的 legacy schema 或 sync 内部路径，但不能作为 CRUD API 请求上下文。

## 实现流程（HOW）

新增一个明确的 workspace 请求上下文类型，推荐命名为 `WorkspaceQuery` 和 `WorkspaceContext`。`WorkspaceQuery` 负责从 URL query 读取 `workspace_id`；`WorkspaceContext` 保存已验证的 active workspace id。handler 层对每个纳入接口提取 `Query<WorkspaceQuery>`，调用 workspace repository 或 service 校验 `workspaces.id = ? AND archived_at IS NULL AND deleted_at IS NULL`。校验失败统一映射为 `ApiError::RecordNotFound`，最终 HTTP 状态为 `404`。

`workspace_id` 的格式校验应与 schema 的 TEXT id 现实兼容，同时满足“完整 workspace id”的 API 合同。推荐使用 UUID 解析校验，因为 r033 后真实 workspace id 是 UUID，OpenAPI 和文档也应声明为完整 UUID 字符串。格式非法时不返回 validation 状态，而是按需求映射为 `404`。如果未来 schema 允许非 UUID workspace id，必须先更新需求和 API 合同。

`NotesService` 的所有入口方法增加 workspace 参数，推荐传 `&WorkspaceContext` 或 `&str workspace_id`。service 不再自己推断 workspace，也不读取 `DEFAULT_WORKSPACE_ID`。所有 normalize create/update 请求的逻辑保持只处理 note 内容、role、field、tags、links、device_id，不混入 workspace fallback。

`NotesRepository` 的所有 CRUD 和派生查询方法增加 `workspace_id` 参数，包括 list、recent、random、daily counts、by-date、tag/field 分组、get by ref、update、archive、delete、field name lookup、note tags、note revisions、note links。所有 SQL 中已有的 `workspace_id = ?` 继续保留，但 bind 值从 `DEFAULT_WORKSPACE_ID` 改为请求传入的 workspace id。创建路径中的 `INSERT INTO notes`、`INSERT INTO note_revisions`、`INSERT INTO note_tags`、`INSERT INTO note_links` 也全部使用请求 workspace id。

taxonomy helper 需要去除对固定默认 workspace 的依赖。`get_or_create_field_in_transaction`、`get_or_create_tag_in_transaction`、字段和 tag 查询、层级 tag 创建、sync payload 生成都接收 workspace id。note tag 和 note link helper 同步接收 workspace id，保证 attach、detach、replace 和 link target 解析都只在同一个 workspace 内进行。

sync payload 生成函数不能再从 `DEFAULT_WORKSPACE_ID` 读取 workspace。`note_payload`、`note_tag_payload`、`note_link_payload` 以及 inline note_revision payload 都必须接收 workspace id 或从包含 workspace 字段的记录中读取。当前 `NoteRecord`、`TagRecord` 等模型如果不含 workspace_id，不建议为了本次改造扩大所有响应模型；推荐在 payload helper 参数中显式传入 workspace id，保持 API response 结构不变。

OpenAPI 维护方式为所有纳入 handler 的 `#[utoipa::path]` 增加 required query 参数 `workspace_id`。如果多个 handler 共享同一个 query DTO，则 DTO 必须派生或实现 `IntoParams` 并准确表达 required 字段；如果现有 query DTO 已经承载 `limit`、`date`、`n` 等参数，可以将 `workspace_id` 合并进对应 query DTO，或者使用公共 `WorkspaceQuery` 加现有 query DTO 的双 query 提取。推荐合并进接口对应 query DTO，减少 Axum 多个 `Query` extractor 对同一 query string 的重复解析风险。

错误处理保持用户已确认的严格口径。缺失 `workspace_id`、UUID 格式非法、active workspace 不存在、workspace 已归档、workspace 已删除，都返回 `404`。note_ref 在指定 workspace 内查不到也返回 `404`；指定 workspace 内前缀匹配多条仍返回现有 ambiguous note reference 错误。列表类接口也不返回空结果来代表无效 workspace，必须先校验 active workspace。

`docs/http-client-server-api.md` 要同步更新所有受影响 URL 示例和参数说明，明确 `workspace_id` 是 required query 参数，且客户端应先调用 `GET /workspaces` 获取完整 workspace id。测试按新合同重写：旧的无 `workspace_id` 请求应覆盖为 `404`；正常路径必须显式 seed active workspace 并传入该 id；跨 workspace 数据隔离应至少覆盖列表、详情、更新和删除；归档或删除 workspace 应覆盖为 `404`。

## 测试用例

| 场景 | 预期 |
| --- | --- |
| 任一纳入接口缺失 `workspace_id` | 返回 `404` |
| 任一纳入接口传入非法 UUID workspace id | 返回 `404` |
| 任一纳入接口传入不存在 workspace id | 返回 `404` |
| 任一纳入接口传入 `archived_at IS NOT NULL` 的 workspace | 返回 `404` |
| 任一纳入接口传入 `deleted_at IS NOT NULL` 的 workspace | 返回 `404` |
| `POST /notes?workspace_id=active` | note、initial revision、field、tag、link 和 sync_change payload 均使用该 workspace id |
| `GET /notes?workspace_id=active` | 只返回该 workspace 下未删除、未归档 notes |
| `GET /notes/{note_ref}?workspace_id=active` | note_ref 只在该 workspace 内解析，其他 workspace 的同前缀 note 不参与歧义判断 |
| `PATCH /notes/{note_ref}?workspace_id=active` | 只更新该 workspace 下的 note，并写入该 workspace 的 revision 和 sync_change |
| `DELETE /notes/{note_ref}?workspace_id=active` | 只软删除该 workspace 下的 note |
| tag、revision、link 相关接口 | 只读取或修改 query 指定 workspace 下的数据 |
| recent、by-date、daily-counts、random notes/tags/fields | 只统计或抽取 query 指定 workspace 下的数据 |
| OpenAPI JSON | 所有纳入接口都暴露 required `workspace_id` query 参数 |
| REST API Markdown 文档 | 所有受影响示例和参数说明都使用 `workspace_id` query |
| 完整回归 | `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` 通过 |
