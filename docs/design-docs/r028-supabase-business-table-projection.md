# r028 Supabase 业务表投影同步

日期：2026-06-19

需求澄清文档：`docs/request-clarify/r028-supabase-business-table-projection.md`

## 核心功能（WHAT）

在当前 Supabase push 链路中增加业务表当前状态投影。后端 push 本地 pending `sync_changes` 时，先根据每条 change 的 payload 写入或删除远端业务表，再写入远端 `sync_changes`，最后推进本地 push cursor 并标记本地 change 已提交到 Supabase。

### 需求背景（WHY）

当前后端已经能把本地 `sync_changes` 推送到 Supabase，也能从远端 `sync_changes` 拉取并应用到本地业务表。实际验证中，远端 `sync_changes` 已经有数据，但远端 `notes`、`fields`、`tags` 等业务表仍为空，导致 Supabase 控制台和后续远端查询只能看到同步日志，看不到业务当前状态。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 双层远端状态 | `sync_changes` 作为同步协议事实日志，业务表作为当前状态投影 |
| push 原子语义 | 同一批 push 中任一业务表投影失败时，不写对应远端 `sync_changes`，不推进本地 push cursor |
| 幂等重试 | 业务表 upsert/delete 和 `sync_changes` upsert 均可重复执行，失败后下一次 push 可重试 |
| 可观测调试 | Supabase 控制台可直接看到 `fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links` 当前状态 |
| 契约归属稳定 | 本仓只消费 `vendor/zembra-schema` 已有 Postgres 契约，不新增、复制或演化 schema |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| 投影实体 | `fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links` |
| 投影触发点 | `SyncService::push` 获取 pending changes 后，写 `sync_changes` 前 |
| 远端访问方式 | 使用 r028 已确认的 Supabase REST/PostgREST |
| note 软状态 | `note insert/update/delete/restore/archive` 都 upsert 远端 `notes`，通过 `deleted_at`、`archived_at` 表达状态 |
| 关系解除 | `note_tag detach` 删除远端 `note_tags` 行，`note_link detach` 删除远端 `note_links` 行 |
| 失败处理 | 业务表投影失败时记录 push error，保留本地 pending 状态，等待下一次重试 |
| 自动化测试 | 覆盖 Supabase client 请求构造和 sync service 投影失败不推进游标 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| schema 变更 | 不修改 `zembra-schema`，不在本仓新增表、字段、索引、约束或 migration |
| 远端 Postgres 直连 | 不引入直接 Postgres 连接 |
| Supabase trigger/function/Realtime | 不使用数据库侧投影机制 |
| pull apply 改造 | r028 聚焦 push 侧远端业务投影，pull 侧仅作为回归检查对象 |
| 附件同步 | 不处理 `attachments` 二进制或元数据投影 |
| 冲突 UI | 不新增冲突处理界面 |

## 实现流程（HOW）

### 设计结论

r028 的当前技术决策是直接改造 push 路径：`sync_changes` 作为同步事实日志，业务表投影作为由 push 派生出的远端当前状态。实现入口以当前代码事实为准，`services::sync` 负责 push 编排，`sync::supabase` 负责 Supabase REST 请求，`repositories::sync` 负责本地 pending change、cursor 和 committed 状态。推荐在 Supabase client 侧新增业务投影请求构造与执行函数，在 `SyncService::push` 中按“ensure identity -> project business tables -> upsert sync_changes -> mark_push_success”的顺序调用。

### 前置契约检查

| 检查项 | 预期 |
| --- | --- |
| `vendor/zembra-schema/postgres/001_initial_schema.sql` | 已存在 `fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links` |
| 字段覆盖 | 远端表字段能覆盖当前 `sync_changes.payload` 已携带字段 |
| 主键和唯一键 | upsert/delete 能以 `vendor/zembra-schema` 已定义主键或复合主键完成 |
| 缺失契约 | 如果发现缺表或缺字段，立即暂停本仓实现，先在 `zembra-schema` 完成契约变更 |

当前仓库已固定消费 `vendor/zembra-schema`，本设计不把 Postgres DDL 内容复制到本仓文档之外作为实现产物。

### Push 编排

| 步骤 | 说明 |
| --- | --- |
| 1 | `SyncService::push` 读取同步配置并拉取本地 pending changes |
| 2 | pending 非空时调用 `ensure_default_sync_identity`，确保默认 workspace 和 backend device 存在 |
| 3 | 按本地 pending changes 顺序执行远端业务表投影 |
| 4 | 全部业务表投影成功后，批量 upsert 远端 `sync_changes` |
| 5 | `sync_changes` upsert 成功后调用 `mark_push_success` 推进本地 push cursor 并写入 `supabase_committed_at` |
| 6 | 任一步失败时调用 `record_error("push", ...)`，返回错误，不推进本地成功状态 |

该顺序让远端出现 `sync_changes` 时，对应业务表状态已经至少尝试成功写入。重复 push 时，业务表投影和 `sync_changes` upsert 都依赖幂等请求。

### 投影规则

| entity_type | operation | 远端业务表行为 |
| --- | --- | --- |
| `field` | `insert` | upsert `fields` |
| `tag` | `insert` | upsert `tags` |
| `note` | `insert`、`update`、`delete`、`restore` | upsert `notes`，保留 payload 中的 `deleted_at`、`archived_at`、`current_revision_id` |
| `note_revision` | `insert` | upsert `note_revisions` |
| `note_tag` | `attach` | upsert `note_tags` |
| `note_tag` | `detach` | delete `note_tags` by `workspace_id`、`note_id`、`tag_id` |
| `note_link` | `attach` | upsert `note_links` |
| `note_link` | `detach` | delete `note_links` by `workspace_id`、`id` |

不支持的 entity 或 operation 在本轮设计中应视为投影错误。这样可以避免远端 `sync_changes` 已提交但业务表漏投影的静默状态。

### Supabase REST 设计

| 操作 | Method | Path | 关键参数 |
| --- | --- | --- | --- |
| upsert field | `POST` | `/rest/v1/fields` | `Prefer: resolution=merge-duplicates` |
| upsert tag | `POST` | `/rest/v1/tags` | `Prefer: resolution=merge-duplicates` |
| upsert note | `POST` | `/rest/v1/notes` | `Prefer: resolution=merge-duplicates` |
| upsert note revision | `POST` | `/rest/v1/note_revisions` | `Prefer: resolution=merge-duplicates` |
| upsert note tag | `POST` | `/rest/v1/note_tags` | `Prefer: resolution=merge-duplicates` |
| delete note tag | `DELETE` | `/rest/v1/note_tags` | `workspace_id=eq.{workspace_id}`、`note_id=eq.{note_id}`、`tag_id=eq.{tag_id}` |
| upsert note link | `POST` | `/rest/v1/note_links` | `Prefer: resolution=merge-duplicates` |
| delete note link | `DELETE` | `/rest/v1/note_links` | `workspace_id=eq.{workspace_id}`、`id=eq.{link_id}` |

所有请求使用当前 Supabase client 的 auth header 构造逻辑。upsert 请求应按业务表需要补齐 `workspace_id`，该值来自 `SyncChangeRecord.workspace_id`，payload 只提供业务实体字段。

### 代码边界

| 模块 | 变更 |
| --- | --- |
| `src/sync/supabase.rs` | 新增业务表投影执行入口、请求构造函数和请求 payload record |
| `src/services/sync.rs` | 在 `push` 中插入投影步骤，并保留 push 失败时写入 sync state error 的语义 |
| `src/repositories/sync/payload.rs` | 复用或补齐 payload 解析结构，作为业务投影输入 |
| `src/repositories/sync/tests.rs` 或相关测试模块 | 增加 push 失败不推进 cursor 的服务级测试 |
| `src/sync/supabase.rs` 单元测试 | 增加 upsert/delete 请求 URL、header、body 构造测试 |

如果 `src/sync/supabase.rs` 因请求构造膨胀，应优先拆出 `src/sync/supabase/projection.rs` 或同等模块，让 Supabase client 主入口聚焦编排和共享认证。

### 错误和日志

| 场景 | 行为 |
| --- | --- |
| payload 解析失败 | 返回 Supabase/sync push 错误，记录 push error，不写远端 `sync_changes` |
| 业务表 upsert/delete 失败 | 返回远端状态错误，记录 push error，不推进 cursor |
| `sync_changes` upsert 失败 | 业务表可能已投影成功，但本地 change 保持 pending，下次 push 依赖幂等请求重试 |
| 空 pending changes | 直接返回 processed 0 |

日志记录 entity type、operation、change id、远端表名和失败摘要，禁止记录 Supabase secret key 或完整敏感响应体。

## 测试用例

### 编译检查

| 用例 | 预期 |
| --- | --- |
| `cargo fmt --check` | 通过 |
| `cargo check` | 通过 |
| `cargo clippy -- -D warnings` | 通过 |

### 自动化测试

| 用例 | 预期 |
| --- | --- |
| Supabase field upsert 请求构造 | URL、auth header、Prefer header 和 JSON body 正确 |
| Supabase tag upsert 请求构造 | `parent_tag_id`、`path`、`depth`、`workspace_id` 正确 |
| Supabase note upsert 请求构造 | `deleted_at`、`archived_at`、`current_revision_id` 保留 |
| Supabase note revision upsert 请求构造 | `note_id`、`device_id`、`base_revision_id` 正确 |
| Supabase note tag attach 请求构造 | upsert `note_tags` 关系行 |
| Supabase note tag detach 请求构造 | delete URL 包含 `workspace_id`、`note_id`、`tag_id` filters |
| Supabase note link attach 请求构造 | upsert `note_links` 关系行 |
| Supabase note link detach 请求构造 | delete URL 包含 `workspace_id` 和 link `id` filters |
| sync service 投影失败 | 不调用或不完成远端 `sync_changes` upsert，不推进 push cursor，不写 `supabase_committed_at` |
| sync service 投影成功 | 业务表投影先于 `sync_changes` upsert，成功后推进 push cursor |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| 本地已有 pending changes 后执行 `/sync/push` | Supabase `sync_changes` 和对应业务表都有可见数据 |
| 删除或归档 note 后 push | Supabase `notes` 行仍存在，`deleted_at` 或 `archived_at` 表达软状态 |
| detach tag/link 后 push | Supabase 对应关系表行被删除 |
| 人为制造业务表投影失败后 push | 本地 change 仍 pending，下一次修复后可重试 |

### 回归检查

| 用例 | 预期 |
| --- | --- |
| `cargo test sync` | pull apply 和 sync push 回归通过 |
| `cargo test notes` | 本地业务写入仍生成原有 sync changes |
| `rg -n "CREATE TABLE|ALTER TABLE|supabase/migrations" src docs vendor --glob '!vendor/zembra-schema/**'` | r028 不新增本仓 schema 或 migration |
