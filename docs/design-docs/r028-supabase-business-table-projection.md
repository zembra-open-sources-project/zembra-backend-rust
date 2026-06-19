# r028 Supabase 真实双向同步

日期：2026-06-19

需求澄清文档：`docs/request-clarify/r028-supabase-business-table-projection.md`

## 核心功能（WHAT）

实现本地 SQLite 与 Supabase 之间的真实双向同步。同步对象只包含 `workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links`、`sync_changes` 九张表；同步流程必须读取两端当前表数据，比较差异，再把缺失或较旧的数据写入另一端，最终以两端九张表一致为目标。

### 需求背景（WHY）

当前后端已经能把部分 `sync_changes` 写入 Supabase，也能从远端 `sync_changes` 拉取并应用到本地，但这条路径没有保证 Supabase 业务表存在本地已有数据对应的真实记录。用户已经明确本需求不是写同步日志，也不是只处理游标后的新增变更，而是把本地已有笔记数据真实同步到 Supabase，并在此基础上验证新数据也能同步。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 已有数据同步 | 当前本地已有九张同步表数据必须真实写入 Supabase |
| 远端数据拉取 | Supabase 有而本地没有的数据必须拉取到本地 |
| 差异比较 | 每次同步先读取本地和远端九张表，再按主键或复合主键比较差异 |
| 时间顺序 | 两端字段不一致时，以对应实体的 `sync_changes.created_at` 时间顺序判断较新状态 |
| 写入安全 | 按外键依赖顺序写入，避免关系表早于主表导致失败 |
| 真实验收 | 验收只接受真实 Supabase 数据结果，不接受单元测试、mock、接口成功响应或随机数据替代 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| 同步表 | `workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links`、`sync_changes` |
| 本地读取 | 从 SQLite 读取九张同步表完整当前状态 |
| 远端读取 | 通过 Supabase REST/PostgREST 读取九张同步表完整当前状态 |
| 差异处理 | 本地缺失、远端缺失、字段不同、关系不同都必须进入差异处理 |
| 本地写入 | Supabase 较新或本地缺失的数据写入 SQLite |
| 远端写入 | 本地较新或远端缺失的数据写入 Supabase |
| 冲突处理 | 无法用 `sync_changes.created_at` 判断方向时停止并记录冲突或返回错误 |
| 验收验证 | 先验证已有数据真实同步，再验证新数据真实同步 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| `sync_state` | 不作为同步表，只可继续作为本地同步游标或运行状态 |
| `sync_conflicts` | 不作为同步表，只可作为冲突记录 |
| schema 修改 | 不修改 `zembra-schema`，不新增本仓 schema、migration、DDL、字段、索引或约束 |
| Supabase 平台能力 | 不使用 Supabase trigger、function、Realtime 或直接远端 Postgres 连接 |
| 附件 | 不同步附件二进制或 `attachments` 表 |
| 多 workspace | 不扩展多 workspace 选择或隔离策略 |
| 冲突 UI | 不提供人工冲突处理界面 |

## 实现流程（HOW）

### 总体流程

真实同步由一个完整流程完成：读取本地九张表，读取 Supabase 九张表，按表主键或复合主键建立对应关系，计算本地缺失、远端缺失、字段不同和关系不同，按 `sync_changes.created_at` 判断字段差异方向，按固定顺序写入本地或远端，写入后重新读取两端并确认差异为空。`push`、`pull`、`run_once` 可以保留现有 API 名称，但实现语义必须服务于真实同步，而不是只处理游标后的 `sync_changes`。

### 同步表与主键

| 表 | 主键或复合键 | 差异比较字段 |
| --- | --- | --- |
| `workspaces` | `id` | `workspace_name`、`created_at`、`updated_at`、`archived_at`、`deleted_at` |
| `devices` | `id` | `workspace_id`、`name`、`platform`、`created_at`、`last_seen_at`、`sync_enabled`、`last_synced_at` |
| `fields` | `id` | `workspace_id`、`name`、`created_at` |
| `tags` | `id` | `workspace_id`、`name`、`parent_tag_id`、`path`、`depth`、`created_at` |
| `notes` | `id` | `workspace_id`、`content`、`role`、`field_id`、`created_at`、`updated_at`、`archived_at`、`deleted_at`、`current_revision_id`、`last_change_id`、`conflict_status` |
| `note_revisions` | `id` | `workspace_id`、`note_id`、`content`、`title`、`device_id`、`created_at`、`base_revision_id`、`change_id` |
| `note_tags` | `workspace_id + note_id + tag_id` | `created_at` |
| `note_links` | `id` | `workspace_id`、`source_note_id`、`target_note_id`、`anchor_text`、`position`、`created_at` |
| `sync_changes` | `id` | `workspace_id`、`device_id`、`entity_type`、`entity_id`、`operation`、`base_revision_id`、`new_revision_id`、`payload`、`created_at`、`applied_at`、`supabase_committed_at` |

### 本地读取设计

本地读取在 `SyncRepository` 中新增九张表快照读取能力。每张表使用显式字段列表和稳定排序，`sync_changes` 固定按 `created_at ASC, id ASC` 排序，关系表按复合键排序。读取结果转换为统一快照结构，后续差异计算不直接依赖 SQL 查询结果。

### 远端读取设计

远端读取由 `SupabaseClient` 通过 Supabase REST/PostgREST 完成。每张表使用 `/rest/v1/{table}` 读取，带认证 header，按 `workspace_id` 过滤需要归属工作区的数据，按主键或复合键稳定排序；`sync_changes` 按 `created_at ASC, id ASC` 排序。读取必须支持分页或明确批量范围，不能只依赖 PostgREST 默认返回窗口。

### 差异计算设计

差异计算使用纯函数接收本地快照和远端快照，输出写本地、写远端、删除本地关系、删除远端关系和冲突停止五类结果。本地有远端没有的数据写远端，远端有本地没有的数据写本地，两端字段相同不处理，两端字段不同则查找该实体在两端 `sync_changes` 中最新的 `created_at`。如果本地最新 change 晚于远端最新 change，写远端；如果远端最新 change 晚于本地最新 change，写本地；如果找不到足够 change 或时间顺序相同且字段不同，停止并记录冲突或返回错误。

### 写入顺序设计

本地和远端写入都必须遵守固定顺序：`workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links`、`sync_changes`。关系删除必须在关系写入前处理，且不能删除主表实体。这个顺序来自当前 schema 的外键依赖，目的是保证 note、tag、device 等主表先存在，再写 revision 和关系表。

### 本地写入设计

远端较新或本地缺失的数据写入 SQLite 时，必须在事务中执行。普通实体使用 upsert，关系缺失使用 insert，关系删除使用 delete，`sync_changes` 使用 `INSERT OR IGNORE` 或等价幂等写入。写入失败时事务回滚；无法判断覆盖方向时不静默覆盖。

### 远端写入设计

本地较新或远端缺失的数据写入 Supabase 时，普通实体和 `sync_changes` 使用 upsert，关系解除使用 delete。每个 Supabase 请求失败都必须向上返回错误，并阻止本次同步标记成功。远端写入后必须重新读取两端快照确认差异为空，不能只看 HTTP 成功响应。

### API 行为设计

现有 `/sync/run`、`/sync/push`、`/sync/pull` API 路径继续保留。`/sync/run` 执行完整双向同步；`/sync/push` 只执行本地到远端方向的差异写入，但仍必须读取两端数据并比较差异；`/sync/pull` 只执行远端到本地方向的差异写入，也必须读取两端数据并比较差异。接口返回的 processed 数量表示实际写入或删除的差异数量，不代表验收通过。

### 冲突处理设计

`sync_conflicts` 不作为同步表。它只用于记录无法安全判断方向的冲突，例如两端字段不同但找不到对应 change、两端最新 change 时间无法区分、关系 attach/detach 同时存在且无法按时间确定最终状态。记录冲突后本次同步返回错误或停止该实体写入，禁止自动乱覆盖。

### 真实验收设计

验收严格分两步。第一步只使用当前本地已有数据，执行真实同步后到 Supabase 九张表检查主键集合和字段值是否与本地一致，同时检查远端多出的数据是否已拉回本地。第二步只能在第一步通过后进行，通过正式业务入口创建新数据，再执行同步并确认 Supabase 业务表和 `sync_changes` 都出现对应记录。单元测试、mock、接口返回成功、只看到 `sync_changes` 有数据、随机临时数据都不能写成验收通过。

## 测试用例

### 编译检查

| 用例 | 预期 |
| --- | --- |
| `cargo fmt --check` | 格式检查通过 |
| `cargo check` | 编译通过 |

### 自动化测试

| 用例 | 预期 |
| --- | --- |
| 本地快照读取 | 能读取九张同步表，排序稳定，字段完整 |
| Supabase 请求构造 | 九张表 GET、upsert、delete 请求路径、header、filter、order 正确 |
| 差异计算 | 覆盖本地缺失、远端缺失、字段相同、字段不同、无法判断方向 |
| 写入顺序 | note 早于 note_tag 和 note_link，workspaces/devices 早于依赖表 |
| 本地写入事务 | 远端快照写本地失败时不部分提交 |
| sync route 回归 | `/sync/run`、`/sync/push`、`/sync/pull` 路由仍可调用并返回结构稳定 |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| 真实 Supabase 已有数据同步 | 当前本地已有九张表数据真实出现在 Supabase 九张表 |
| 真实 Supabase 远端拉取 | Supabase 多出的九张表数据真实拉回本地 |
| 真实 Supabase 新数据同步 | 第一验收通过后，新建数据同步到 Supabase 业务表和 `sync_changes` |

### 回归检查

| 用例 | 预期 |
| --- | --- |
| `rg -n "sync_state|sync_conflicts" docs/design-docs/r028-supabase-business-table-projection.md docs/exec-plans/active/r028-supabase-business-table-projection.md` | 命中只说明两表不是同步表或只作本地状态/冲突记录 |
| `rg -n "supabase/migrations|CREATE TABLE|ALTER TABLE" docs/design-docs/r028-supabase-business-table-projection.md docs/exec-plans/active/r028-supabase-business-table-projection.md` | 不存在把 schema 变更作为本需求实现内容的语句 |
| `git diff --stat` | 设计和计划阶段只包含 r028 文档，不包含生产代码改动 |
