# r019 Note Links API 开发计划

日期：2026-05-21

## 关联需求澄清

`docs/request-clarify/r019-note-links-api.md`

## 关联设计文档

`docs/design-docs/r019-note-links-api.md`

## 阶段 #1: Link DTO 与模型基础

### 任务 #1: 定义 note link model 和 DTO

**Status:** Finished

**Files:** Create `src/models/note_link.rs`; Modify `src/models/mod.rs`, `src/dto/notes.rs`, `src/api_doc.rs`

| 项目 | 内容 |
| --- | --- |
| 功能 | 建立 `note_links` 的 Rust 表达和 API 合同 |
| 实现说明 | 新增 `NoteLinkRecord` 映射 `id/source_note_id/target_note_id/anchor_text/position/created_at`；新增 `NoteLinkRequest` 和 `NoteLinkResponse`；扩展 `NoteMetadata` 增加 `outgoing_links`、`backlinks` |
| 预期验证结果 | OpenAPI components 能生成 link request/response schema；现有 DTO 编译通过 |

### 任务 #2: 扩展 create/update request 和响应类型

**Status:** Finished

**Files:** Modify `src/dto/notes.rs`, `src/handlers/notes.rs`

| 项目 | 内容 |
| --- | --- |
| 功能 | `POST /notes` 和 `PATCH /notes/{note_ref}` 接收结构化 links，`GET/PATCH` 返回扩展 `NoteResponse` |
| 实现说明 | `CreateNoteRequest.links` 默认空数组；`UpdateNoteRequest.links` 使用 `Option<Vec<NoteLinkRequest>>` 表达未传、清空、替换三态；handler 的 OpenAPI response 从 `NoteRecord` 更新为 `NoteResponse` |
| 预期验证结果 | create/update/get 的请求和响应合同与需求澄清一致 |

## 阶段 #2: Repository link 写入与查询

### 任务 #1: 实现 link target 校验和查询

**Status:** Finished

**Files:** Modify `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| 功能 | 解析 `target_note_ref`，并只匹配未删除、未归档 note |
| 实现说明 | 新增可见 note ref 查询 helper，固定过滤 `deleted_at IS NULL` 和 `archived_at IS NULL`；自引用返回 validation error；target 不存在或隐藏返回 not found |
| 预期验证结果 | 已删除、已归档、无效 ref、冲突 ref 和自引用都按约定错误返回 |

### 任务 #2: 创建 note 时写入 links

**Status:** Finished

**Files:** Modify `src/repositories/notes.rs`, `src/services/notes.rs`

| 项目 | 内容 |
| --- | --- |
| 功能 | `POST /notes` 在创建事务内写入 outgoing links |
| 实现说明 | 扩展 `CreateNoteInput` 携带 normalized links；`create_note_in_transaction` 创建 note/revision/field/tags 后写入 `note_links`，并记录 `note_link` sync changes；创建响应 metadata 包含 outgoing links，backlinks 为空 |
| 预期验证结果 | 创建 note 带 links 时数据库关系和响应 metadata 正确；target 校验失败时整笔创建回滚 |

### 任务 #3: 修改 note 时整体维护 links

**Status:** Finished

**Files:** Modify `src/repositories/notes.rs`, `src/services/notes.rs`

| 项目 | 内容 |
| --- | --- |
| 功能 | `PATCH /notes/{note_ref}` 支持保持、清空和整体替换 outgoing links |
| 实现说明 | 扩展 `UpdateNoteInput` 携带 `Option<Vec<NormalizedNoteLinkInput>>`；未传 links 不改关系；传空数组删除所有 outgoing links；传非空数组删除旧关系并插入新关系；新增和删除均记录 sync changes |
| 预期验证结果 | PATCH 三态行为稳定，content/revision/field/tags/links 在同一事务内更新 |

### 任务 #4: 查询 outgoing links 和 backlinks

**Status:** Finished

**Files:** Modify `src/repositories/notes.rs`, `src/services/notes.rs`

| 项目 | 内容 |
| --- | --- |
| 功能 | 为 `GET/PATCH/POST` 响应组装 link metadata |
| 实现说明 | outgoing 查询过滤 target note 未删除、未归档；backlinks 查询过滤 source note 未删除、未归档；service 统一将 `CreatedNote` 或 `NoteRecord` 转为扩展 `NoteResponse` |
| 预期验证结果 | GET note 能同时返回该 note link 了谁、被谁 link，且隐藏 note 关系不出现在结果中 |

## 阶段 #3: Sync 与 API 注册

### 任务 #1: 支持 note_link sync changes 重放

**Status:** Finished

**Files:** Modify `src/repositories/sync.rs`, `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| 功能 | 本地 link 变化可进入 sync changes，远端 `note_link` 变化可重放到 `note_links` |
| 实现说明 | 为 link 新增 payload helper 和 entity id 规则；remote apply 增加 `note_link` attach/detach 或 insert/delete 分支，operation 必须与本地记录一致 |
| 预期验证结果 | sync repository 测试能验证新增 link 和删除 link 的远端变更重放 |

### 任务 #2: 注册 OpenAPI schema 和 path 更新

**Status:** Finished

**Files:** Modify `src/api_doc.rs`, `src/handlers/notes.rs`

| 项目 | 内容 |
| --- | --- |
| 功能 | OpenAPI JSON 暴露 create/update/get 的 links 合同 |
| 实现说明 | 注册新增 DTO/model schema；确保 `GET /notes/{note_ref}`、`PATCH /notes/{note_ref}` 响应 body 为 `NoteResponse`；`CreateNoteRequest` 和 `UpdateNoteRequest` schema 包含 links |
| 预期验证结果 | `/api-docs/openapi.json` 包含 links request/response 字段 |

## 阶段 #4: 自动化测试与回归验证

### 任务 #1: 补充 repository 测试

**Status:** Finished

**Files:** Modify `src/repositories/notes.rs`

| 项目 | 内容 |
| --- | --- |
| 功能 | 覆盖 link 写入、替换、清空、查询和错误场景 |
| 实现说明 | 使用 in-memory SQLite，构造 visible/archived/deleted notes；验证 duplicate target 不同 position 保留多条；验证事务回滚 |
| 预期验证结果 | repository 层完整覆盖 link 数据一致性 |

### 任务 #2: 补充 API 测试

**Status:** Finished

**Files:** Modify `src/app.rs`

| 项目 | 内容 |
| --- | --- |
| 功能 | 覆盖 HTTP create/update/get 的 links 合同 |
| 实现说明 | 复用现有 test helpers；验证 `POST /notes`、`PATCH /notes/{note_ref}`、`GET /notes/{note_ref}` 的响应 metadata；验证 hidden target 错误和 OpenAPI schema |
| 预期验证结果 | API 行为与需求澄清一致，现有 notes 行为不回归 |

### 任务 #3: 全量验证与计划回写

**Status:** Finished

**Files:** Verify full workspace; Modify `docs/exec-plans/active/r019-note-links-api.md`

| 项目 | 内容 |
| --- | --- |
| 功能 | 完成实现后的最终质量确认 |
| 实现说明 | 运行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`；如启动服务验证 OpenAPI，则确认 `/api-docs/openapi.json` 包含 links 字段 |
| 预期验证结果 | 所有验证通过；任务状态按实际推进更新；每个阶段完成后按项目纪律执行一次 Conventional Commit |

## 决策记录

- 2026-05-21：确认 WebUI / CLI 负责解析正文，后端只接收结构化 links。
- 2026-05-21：确认所有 link 相关操作只处理未删除、未归档 note。
- 2026-05-21：确认创建、修改、读取接口返回扩展 `NoteResponse`，metadata 包含 link 信息。
- 2026-05-21：完成实现，新增 note link DTO/model、create/update/get 响应 metadata、repository link 写入/替换/查询、note_link sync 重放和自动化测试。
- 2026-05-21：验证通过 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`。
