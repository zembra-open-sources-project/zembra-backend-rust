# r024 Recent Notes Role Filter 开发计划

日期：2026-05-29

关联需求澄清：`docs/request-clarify/r024-recent-notes-role-filter.md`
设计文档：`docs/design-docs/r024-recent-notes-role-filter.md`

## 关联设计文档

`docs/design-docs/r024-recent-notes-role-filter.md`

## Stage #1: Role Filter 查询链路落地

### Task #1: 扩展 RecentNotesRequest 和 role filter 枚举

**Status:** Finished

**Files:** Modify `src/dto/notes.rs`, `src/services/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `RecentNotesRequest.role`、`RecentNotesRoleFilter`、role 解析 helper |
| Implementation Notes | 在 request body 中新增 `role: Option<String>`；新增专用枚举表达 `Human`、`Agent`、`Both`；解析逻辑大小写不敏感，非法值返回 `ApiError::Validation`；不传 role 时默认 `Both`；新增函数和枚举成员按项目规则补充注释 |
| Expected Verification Result | service 层不再向 repository 传递原始 role 字符串，非法 role 能映射为 validation error |

### Task #2: 扩展 recent notes repository 查询

**Status:** Finished

**Files:** Modify `src/repositories/notes/core.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `NotesRepository::list_recent_notes` |
| Implementation Notes | 方法签名增加已解析 role filter 参数；`Both` 保持当前 SQL 行为；`Human` 和 `Agent` 在无游标和有游标两条查询路径中追加 `role = ?` 条件；保留 `deleted_at IS NULL`、`archived_at IS NULL`、`updated_at DESC, id DESC` 和现有游标规则 |
| Expected Verification Result | repository 可按 Human、Agent 或 Both 返回最近可见 notes，游标查询仍只返回更旧记录 |

### Task #3: 串接 service、handler schema 和 OpenAPI 合同

**Status:** Finished

**Files:** Modify `src/services/notes.rs`, Verify `src/handlers/notes.rs`, Verify `src/api_doc.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `NotesService::recent_notes`、`RecentNotesRequest` OpenAPI schema |
| Implementation Notes | `recent_notes` 保持默认 limit 50；解析 role 后调用 repository；handler 继续依赖 request validate 和 service 错误映射；`RecentNotesRequest` 已在 `ApiDoc` components 中注册，确认新增字段会进入 OpenAPI JSON |
| Expected Verification Result | `POST /notes/recent` 接受 role 字段且 OpenAPI JSON 暴露该字段 |

## Stage #2: 自动化测试补齐

### Task #1: 增加 repository role filter 测试

**Status:** Finished

**Files:** Modify `src/repositories/notes/tests.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `list_recent_notes` role filter 单元测试 |
| Implementation Notes | 构造 Human 和 Agent notes；覆盖 Human 过滤、Agent 过滤、Both 与无 role 等价、role 与 limit 组合、role 与 cursor 组合；继续覆盖隐藏笔记不返回 |
| Expected Verification Result | `cargo test repositories::notes::tests::list_recent_notes` 相关测试能证明 repository 行为稳定 |

### Task #2: 增加 HTTP route role filter 测试

**Status:** Finished

**Files:** Modify `tests/notes_query_routes.rs`, Modify `tests/support/notes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | `/notes/recent` role 请求集成测试、test note builder role helper |
| Implementation Notes | 给 `TestNoteBuilder` 增加设置 role 的 builder 方法；覆盖 `role: "human"`、`role: "agent"`、`role: "both"`、不传 role、大小写不敏感和非法 role validation error |
| Expected Verification Result | HTTP 层能正确解析 JSON role 字段，非法 role 返回 `422 validation_error` |

### Task #3: 增加 OpenAPI role 字段测试

**Status:** Finished

**Files:** Modify `tests/openapi_routes.rs`

| 项目 | 内容 |
| --- | --- |
| Function | OpenAPI schema regression test |
| Implementation Notes | 在现有 OpenAPI 测试中断言 `RecentNotesRequest` schema 暴露 `role` 属性；保持 `/notes/recent` path 断言 |
| Expected Verification Result | `/api-docs/openapi.json` 包含 `/notes/recent` 和 `RecentNotesRequest.role` |

## Stage #3: 整体验证和计划回写

### Task #1: 运行格式、测试和静态检查

**Status:** Finished

**Files:** Verify repository

| 项目 | 内容 |
| --- | --- |
| Function | build/regression verification |
| Implementation Notes | 运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`；如果 clippy 当前项目命令约定不同，以仓库现有验证方式为准 |
| Expected Verification Result | 四项验证通过，无新增 warning 或失败测试 |

### Task #2: 更新执行记录并准备提交

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r024-recent-notes-role-filter.md`

| 项目 | 内容 |
| --- | --- |
| Function | progress record、commit readiness |
| Implementation Notes | 将已完成任务状态更新为 `Finished`，记录验证命令和结果；完成 Stage 后按项目要求进行原子提交，commit message 需满足 Conventional Commits 白名单和防火墙规则 |
| Expected Verification Result | 计划状态与实际代码一致，提交信息客观描述 technical change 且 description 超过 10 个字符 |

## 执行记录

- 2026-05-29：完成需求澄清，确认复用 `POST /notes/recent`，新增大小写不敏感 `role` 过滤，默认条数保持 50。
- 2026-05-29：完成设计文档和开发计划，等待进入编码阶段。
- 2026-05-29：完成 Stage #1，新增 `RecentNotesRequest.role`、`RecentNotesRoleFilter` 专用枚举解析，并扩展 `NotesRepository::list_recent_notes` 支持 role 过滤。
- 2026-05-29：完成 Stage #2，补齐 repository、HTTP route 和 OpenAPI role 字段测试，覆盖大小写不敏感、非法 role、Human/Agent/Both、limit 和 cursor 组合。
- 2026-05-29：完成 Stage #3 验证：`cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings` 全部通过。

## 约束

- 不修改 `GET /notes` 行为。
- 不新增 API path。
- 不修改默认 limit 50 和 limit 范围 1 到 100。
- 不修改 note 创建接口的 `Human`、`Agent` 存储值。
- 新增或修改 HTTP 请求 DTO 时必须同步确认 OpenAPI JSON 暴露字段。
- 完成每个 Stage 后，如果修改了代码，需要执行一次原子提交。
- 提交信息必须满足 `^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert): .{10,}$`，禁止 `git commit --amend`。
