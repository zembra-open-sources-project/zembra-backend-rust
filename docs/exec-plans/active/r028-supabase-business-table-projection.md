# r028 Supabase 真实双向同步实现计划

> **给 Claude：** 必需工作流：使用 superpowers:executing-plans 逐任务实现此计划。

**目标：** 实现本地 SQLite 与 Supabase 九张同步表的真实双向同步，先让本地已有数据严格同步到 Supabase，再验证新数据也能同步。

**需求澄清文档：** `docs/request-clarify/r028-supabase-business-table-projection.md`

**相关设计文档：** `docs/design-docs/r028-supabase-business-table-projection.md`

**架构：** 同步流程必须先确认本地 SQLite 与远端 Supabase/Postgres 处在同一个 `zembra-schema` contract version；不一致时先按 `vendor/zembra-schema/postgres/` 既有迁移把远端整体迁到目标版本，再读取本地和 Supabase 两端的真实表数据，按主键或复合主键比较差异，再把差异写入缺失或较旧的一端。`sync_changes.created_at` 只作为字段差异和关系差异的时间顺序依据，不能替代 schema contract 一致性检查，也不能替代九张表的实际读取、比较和写入。

**技术栈：** Rust、Tokio、SQLx SQLite、Reqwest、Supabase REST/PostgREST、`vendor/zembra-schema` 已固定的数据契约。

**范围 / 非范围：** 范围包含远端 schema contract 一致性检查、基于 `vendor/zembra-schema/postgres/` 既有迁移的远端整体 contract 迁移，以及 `workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links`、`sync_changes` 九张表的真实双向同步；`sync_state` 和 `sync_conflicts` 不作为同步表，只可作为本地状态或冲突记录；本仓库不定义、不复制、不手写业务 schema，不做缺表缺字段局部修补，不把 `note_links` 当特殊 case，不使用 Supabase trigger/function/Realtime，不做附件同步、不做多 workspace、不做冲突 UI。

---

## Stage #1: 设计归零与真实同步方案

### Task #1: 创建 r028 设计文档

**Status:** Finished

**文件：**
- 创建：`docs/design-docs/r028-supabase-business-table-projection.md`
- 读取：`docs/request-clarify/r028-supabase-business-table-projection.md`
- 读取：`vendor/zembra-schema/sqlite/001_initial_schema.sql`
- 读取：`vendor/zembra-schema/postgres/001_initial_schema.sql`
- 读取：`vendor/zembra-schema/postgres/migrations/005_add_unified_schema_contract.sql`
- 读取：`src/services/sync.rs`
- 读取：`src/sync/supabase.rs`
- 读取：`src/repositories/sync/`

- 功能：把 r028 设计从旧的游标推送或日志写入口径改成真实双向同步口径。
- 实现说明：设计文档必须明确九张表的本地读取方式、远端读取方式、主键或复合主键、差异类型、写入方向、写入顺序、冲突停止条件和真实 Supabase 验收方式。设计文档禁止引入 `sync_state`、`sync_conflicts` 作为同步表，禁止写入任何 schema 变更。
- 预期验证结果：设计文档包含需求澄清文档路径；文档没有把只处理游标后日志或只重放已提交日志作为实现路径；文档明确先读取两端数据再比较差异。

### Task #2: 建立真实验收清单

**Status:** Finished

**文件：**
- 修改：`docs/design-docs/r028-supabase-business-table-projection.md`
- 修改：`docs/exec-plans/active/r028-supabase-business-table-projection.md`

- 功能：把用户验收标准落成可执行检查清单，避免再用单元测试或随机数据当通过依据。
- 实现说明：清单必须分成两步：第一步只验证当前本地已有数据同步到 Supabase；第二步在第一步通过后创建新数据并验证 Supabase 九张表。清单必须禁止制造随机测试数据替代已有数据，禁止把本地单元测试、mock 请求、接口返回成功、只看到 `sync_changes` 有数据当作验收通过。
- 预期验证结果：设计文档和本计划都能直接看出真实验收顺序；后续实现完成前不得把自动化测试结果写成验收通过。

## Stage #2A: Schema Contract 一致性与远端迁移

### Task #16: 设计并实现本地与远端 schema contract version 读取

**Status:** Finished

**文件：**
- 创建：`src/sync/schema_contract.rs`
- 修改：`src/sync/mod.rs`
- 修改：`src/repositories/sync/mod.rs`
- 验证：`src/repositories/sync/tests.rs`

- 功能：同步前读取本地 SQLite 与远端 Supabase/Postgres 的 schema contract version。
- 实现说明：本地版本读取 `schema_migrations` 当前最高或目标版本；远端版本必须读取远端 `schema_migrations`，不能通过检查九张同步表是否存在来替代。远端读取需要管理员级 Postgres/Supabase migration 连接配置，不能复用只适合 PostgREST 数据访问的 Supabase REST `secret_key` 来执行 DDL。
- 预期验证结果：自动化测试覆盖本地版本读取；远端读取封装有清晰错误类型；当远端版本缺失或低于本地目标版本时返回 schema contract mismatch。
- 完成时间：2026-06-19，已通过 `cargo fmt --check`、`cargo test repositories::sync::tests::local_schema_contract_version_reads_current_version`、`cargo test sync::supabase::tests::schema_contract_request_targets_schema_migrations`、`cargo check`。

### Task #17: 实现远端 schema contract migration

**Status:** Finished

**文件：**
- 创建：`src/sync/schema_migration.rs`
- 修改：`src/config.rs`
- 修改：`src/services/sync.rs`
- 验证：`src/sync/schema_migration.rs`

- 功能：当远端 contract version 落后时，由后端执行 `vendor/zembra-schema/postgres/` 既有迁移，把远端整体迁到目标版本。
- 实现说明：迁移来源只能读取 `vendor/zembra-schema/postgres/`，禁止在后端代码中内嵌、复制或拼接业务 DDL；迁移按 contract version 执行整体路径，不按缺失表或缺失字段做局部修补。迁移必须是显式管理员能力，普通数据同步发现版本不一致时停止并返回明确错误，具备迁移配置和执行许可时才运行 migration。
- 预期验证结果：单元测试覆盖 migration 文件选择、版本顺序和禁止局部补表；配置缺少管理员连接时，普通同步返回 schema contract mismatch，不继续读取九张表。
- 完成时间：2026-06-19，已实现显式远端 contract migration 开关和管理员 Postgres 连接配置，迁移 SQL 只来自 `vendor/zembra-schema/postgres/001_initial_schema.sql`，同步入口在 schema mismatch 且允许迁移时先执行 contract migration 并重新读取远端 contract version；已通过 `cargo fmt --check`、`cargo test config`、`cargo test sync::schema_migration`、`cargo test sync_routes`、`cargo check`、`bash -n scripts/verify_r028_real_sync.sh`。

### Task #18: 将 schema contract gate 接入同步入口

**Status:** Finished

**文件：**
- 修改：`src/services/sync.rs`
- 修改：`src/handlers/sync.rs`
- 修改：`scripts/verify_r028_real_sync.sh`
- 验证：`tests/sync_routes.rs`

- 功能：`/sync/run`、`/sync/push`、`/sync/pull` 在读取九张表前先执行 schema contract gate。
- 实现说明：schema contract 一致时进入已有快照同步流程；不一致且不能迁移时返回同步失败；迁移成功后重新读取远端 contract version，再进入同步流程。脚本验收必须先报告两端 schema contract version，再报告九张表数据一致性。
- 预期验证结果：路由测试覆盖 schema mismatch 返回失败；真实验收脚本输出本地/远端 contract version，且不再把 `note_links` 缺失表作为局部修补目标。
- 完成时间：2026-06-19，已通过 `cargo fmt --check`、`cargo test sync_routes`、`cargo check`；真实脚本显示本地 contract 为 `0.5.0`，远端缺少 `schema_migrations` contract 表，`/sync/run` 在读取九张表前返回 schema contract 失败。

## Stage #2: 本地九张表快照读取

### Task #3: 定义同步表数据模型和主键规则

**Status:** Finished

**文件：**
- 创建：`src/sync/table_snapshot.rs`
- 修改：`src/sync/mod.rs`
- 读取：`vendor/zembra-schema/json/workspace.schema.json`
- 读取：`vendor/zembra-schema/json/device.schema.json`
- 读取：`vendor/zembra-schema/json/field.schema.json`
- 读取：`vendor/zembra-schema/json/tag.schema.json`
- 读取：`vendor/zembra-schema/json/note.schema.json`
- 读取：`vendor/zembra-schema/json/note_revision.schema.json`
- 读取：`vendor/zembra-schema/json/note_tag.schema.json`
- 读取：`vendor/zembra-schema/json/note_link.schema.json`
- 读取：`vendor/zembra-schema/json/sync_change.schema.json`

- 功能：建立九张表的 Rust 数据结构、表名枚举、主键或复合主键规则和字段比较规则。
- 实现说明：结构体字段必须对应 `vendor/zembra-schema` 已有契约；`note_tags` 使用 `workspace_id + note_id + tag_id` 作为关系键；其他表按 schema 主键或现有代码事实确认；字段比较必须覆盖软删除、归档、revision、关系表字段和 `sync_changes` 字段。所有结构体成员变量按项目规范写注释。
- 预期验证结果：`cargo check` 能通过；模型中只出现需求指定九张同步表；没有新增 schema 文件或 migration。
- 完成时间：2026-06-19，已通过 `cargo fmt --check`、`cargo test repositories::sync::tests::read_local_table_snapshot_returns_all_sync_tables_in_stable_order`、`cargo check`。

### Task #4: 实现本地 SQLite 快照读取

**Status:** Finished

**文件：**
- 创建：`src/repositories/sync/snapshot.rs`
- 修改：`src/repositories/sync/mod.rs`
- 修改：`src/repositories/sync/types.rs`
- 验证：`src/repositories/sync/tests.rs`

- 功能：一次读取本地九张同步表的当前数据，供差异比较使用。
- 实现说明：在 `SyncRepository` 中新增读取本地快照的方法，SQL 查询必须显式列字段并按稳定顺序排序；读取 `notes` 时包含 `deleted_at`、`archived_at` 和 `current_revision_id`；读取关系表时包含完整复合键；读取 `sync_changes` 时按 `created_at ASC, id ASC` 排序。该任务只读本地数据，不写本地和远端。
- 预期验证结果：新增仓储测试能在已有测试数据库中插入或复用 fixtures 后读取九张表；断言本地快照包含 notes、关系表和 `sync_changes`，且排序稳定。
- 完成时间：2026-06-19，已通过 `cargo fmt --check`、`cargo test repositories::sync::tests::read_local_table_snapshot_returns_all_sync_tables_in_stable_order`、`cargo check`。

## Stage #3: Supabase 九张表读写能力

### Task #5: 扩展 Supabase REST client 的通用表读取

**Status:** Finished

**文件：**
- 修改：`src/sync/supabase.rs`
- 验证：`src/sync/supabase.rs`

- 功能：通过 Supabase REST/PostgREST 读取九张同步表的远端当前数据。
- 实现说明：为九张表分别建立 GET 请求构造和响应解析，统一使用认证 header、`workspace_id` 过滤和稳定排序；`sync_changes` 必须按 `created_at ASC, id ASC` 读取；大表读取必须支持分页或明确批量上限，不能只取默认第一页。请求构造测试只验证 URL、header、filter、order、limit/range，不作为真实验收。
- 预期验证结果：`cargo test sync::supabase` 通过；请求路径覆盖 `workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links`、`sync_changes`。
- 完成时间：2026-06-19，已通过 `cargo fmt --check`、`cargo test sync::supabase`、`cargo check`。

### Task #6: 扩展 Supabase REST client 的表写入和删除

**Status:** Finished

**文件：**
- 修改：`src/sync/supabase.rs`
- 验证：`src/sync/supabase.rs`

- 功能：通过 Supabase REST/PostgREST 对九张同步表执行 upsert 和必要 delete。
- 实现说明：普通实体和 `sync_changes` 使用 upsert；关系解除使用 delete；写入顺序必须满足外键依赖：`workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links`、`sync_changes`。任何远端写入失败都必须返回错误，不能推进本地成功状态。
- 预期验证结果：请求构造测试覆盖每张表的 upsert/delete 路径和 `Prefer: resolution=merge-duplicates`；`cargo test sync::supabase` 通过。
- 完成时间：2026-06-19，已通过 `cargo fmt --check`、`cargo test sync::supabase`、`cargo check`。

## Stage #4: 差异比较与方向判断

### Task #7: 实现九张表差异计算

**Status:** Finished

**文件：**
- 创建：`src/sync/diff.rs`
- 修改：`src/sync/mod.rs`
- 验证：`src/sync/diff.rs`

- 功能：比较本地快照和远端快照，产出需要写本地、写远端、删除关系或冲突停止的差异列表。
- 实现说明：本地有远端没有时写远端；远端有本地没有时写本地；两端字段相同时不处理；两端字段不同时查找对应实体最新 `sync_changes.created_at`，较新的状态覆盖较旧状态；无法判断时间顺序时产出冲突错误。差异计算必须是纯函数，便于自动化覆盖边界。
- 预期验证结果：单元测试覆盖九张表至少一种本地缺失、远端缺失、字段不同、字段相同场景；覆盖 `sync_changes.created_at` 较新方向判断；覆盖无法判断时返回错误。
- 完成时间：2026-06-19，已通过 `cargo fmt --check`、`cargo test sync::diff`、`cargo check`。

### Task #8: 定义写入批次和外键安全顺序

**Status:** Finished

**文件：**
- 修改：`src/sync/diff.rs`
- 验证：`src/sync/diff.rs`

- 功能：把差异结果整理成安全写入顺序，避免关系表先于主表导致远端外键失败。
- 实现说明：写入远端和写入本地都使用固定顺序：`workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links`、`sync_changes`；关系删除在对应关系 upsert 前处理；不能用随机顺序或 HashMap 迭代顺序驱动真实写入。
- 预期验证结果：单元测试断言包含 note 和 note_tag 差异时，note 写入一定排在 note_tag 前；包含 note_link 差异时，相关 notes 写入一定排在 note_links 前。
- 完成时间：2026-06-19，已通过 `cargo fmt --check`、`cargo test sync::diff`、`cargo check`。

## Stage #5: 本地写入与 sync service 编排

### Task #9: 实现远端差异写入本地

**Status:** Finished

**文件：**
- 创建：`src/repositories/sync/write_snapshot.rs`
- 修改：`src/repositories/sync/mod.rs`
- 修改：`src/repositories/sync/apply.rs`
- 验证：`src/repositories/sync/tests.rs`

- 功能：把 Supabase 有而本地缺失或较新的表数据写入本地 SQLite。
- 实现说明：本地写入应复用或抽取现有 `apply_remote_changes` 中的表写入能力，但不能只依赖远端 `sync_changes` payload；必须能直接从远端九张表快照写入本地九张表。写入应在事务中执行，失败时回滚，并在无法判断覆盖方向时记录 `sync_conflicts` 或返回错误。
- 预期验证结果：仓储测试能把远端快照中的 note、revision、tag、note_tag、note_link 和 `sync_changes` 写入空本地库；失败路径不会部分提交。
- 完成时间：2026-06-19，已通过 `cargo fmt --check`、`cargo test repositories::sync`、`cargo test sync_routes`、`cargo check`。

### Task #10: 重写 `SyncService::push`、`pull` 和 `run_once`

**Status:** Finished

**文件：**
- 修改：`src/services/sync.rs`
- 修改：`src/sync/supabase.rs`
- 修改：`src/repositories/sync/snapshot.rs`
- 修改：`src/repositories/sync/write_snapshot.rs`
- 验证：`tests/sync_routes.rs`
- 验证：`src/repositories/sync/tests.rs`

- 功能：把同步入口改为读取两端九张表、比较差异、写入差异、再复读确认一致。
- 实现说明：`push` 可以保留 API 名称，但行为必须是把本地较新或远端缺失的数据同步到 Supabase；`pull` 可以保留 API 名称，但行为必须是把远端较新或本地缺失的数据同步到本地；`run_once` 必须执行完整双向流程。每次真实同步完成后要重新读取两端数据并确认无差异，失败时不标记成功。
- 预期验证结果：路由测试仍能调用 `/sync/run`、`/sync/push`、`/sync/pull`；服务层测试可用假的 Supabase client 或请求构造验证编排，但不能把这些测试记为用户验收通过。
- 完成时间：2026-06-19，已通过 `cargo fmt --check`、`cargo test repositories::sync`、`cargo test sync_routes`、`cargo check`。

## Stage #6: 真实 Supabase 验收

### Task #11: 准备真实同步验证脚本或手工检查命令

**Status:** Finished

**文件：**
- 创建：`scripts/verify_r028_real_sync.sh`
- 修改：`docs/exec-plans/active/r028-supabase-business-table-projection.md`

- 功能：提供只读取和同步现有数据的真实验收流程，避免再制造随机数据。
- 实现说明：脚本只能使用当前 `.zembra.env` 或现有配置读取本地数据库和 Supabase 配置；第一阶段只统计本地已有九张表记录数、执行真实同步、再读取 Supabase 九张表记录数和关键 ID；不得插入随机 note。第二阶段必须在第一阶段通过后，由用户明确触发新数据路径，或使用应用正式 API 创建一条新数据后再同步验证。
- 预期验证结果：脚本输出本地与 Supabase 九张表记录数、缺失 ID 列表和最终一致性结果；输出中不能把单元测试通过写成验收通过。
- 完成时间：2026-06-19，已创建 `scripts/verify_r028_real_sync.sh` 并通过 `bash -n scripts/verify_r028_real_sync.sh`；脚本会读取当前本地 SQLite 和 Supabase 真实数据，先报告本地和远端 schema contract version，contract 不一致时跳过无意义的九表比对并执行真实 `/sync/run`，同步后重新读取 contract version，contract 一致后按九张同步表的完整行数据而非 count 做最终比较。脚本不制造随机数据；如需让后端执行远端 contract migration，可通过 `ZEMBRA_SYNC_REMOTE_DATABASE_PASSWORD` 或 `~/.zembra.env` 中的 `remote_database_password` 提供 Supabase database password，后端会根据 `supabase_url` 拼接 Postgres 连接 URL。

### Task #12: 执行第一验收：已有数据真实同步到 Supabase

**Status:** Testing

**文件：**
- 验证：`.zembra.env`
- 验证：`scripts/verify_r028_real_sync.sh`
- 修改：`docs/exec-plans/active/r028-supabase-business-table-projection.md`

- 功能：把当前本地已经存在的笔记和相关数据真实同步到 Supabase。
- 实现说明：禁止制造随机数据；禁止只看 `sync_changes`；必须确认 Supabase 中九张表都出现本地已有数据对应记录。若远端存在本地没有的数据，也必须验证被拉回本地。
- 预期验证结果：Supabase 九张表与本地九张表在主键集合和字段值上达到一致，计划中记录真实 Supabase 验证时间、命令摘要和结果。
- 验证记录：2026-06-19 真实执行 `./scripts/verify_r028_real_sync.sh`。本地 contract 为 `0.5.0`，远端 contract 为 `missing`，当前配置已提供 `remote_database_password`，后端会自动尝试 remote schema contract migration，不再需要 `migrate_remote_schema` 开关。本机真实验收仍未通过，失败点为后端 direct Postgres 连接未在当前网络环境完成连接，尚未执行到九表同步；该结果不是验收通过，下一步需要让后端获得可用的 Supabase Postgres 管理连接后重新运行本脚本。

### Task #13: 执行第二验收：新数据真实同步到 Supabase

**Status:** Designed

**文件：**
- 验证：`scripts/verify_r028_real_sync.sh`
- 修改：`docs/exec-plans/active/r028-supabase-business-table-projection.md`

- 功能：在已有数据同步通过后，验证新创建数据也能同步到 Supabase。
- 实现说明：必须先完成 Task #12；新数据应通过正式应用 API 或用户认可的正式入口创建，不能绕过业务逻辑直接写库。同步后必须在 Supabase 业务表和 `sync_changes` 中都看到对应记录。
- 预期验证结果：新数据对应的 `notes`、必要的 `note_revisions`、关系表和 `sync_changes` 在 Supabase 可见，并与本地一致。

## Stage #7: 收尾验证与提交

### Task #14: 自动化回归验证

**Status:** Designed

**文件：**
- 验证：`Cargo.toml`
- 验证：`src/`
- 验证：`tests/`

- 功能：确认真实同步实现没有破坏既有后端行为。
- 实现说明：运行 `cargo fmt --check`、`cargo check`、`cargo test sync`、`cargo test sync_routes`、`cargo test openapi_routes`；如果改动影响全局路由或 DTO，再运行 `cargo test`。这些结果只作为工程回归验证，不作为 r028 用户验收通过标准。
- 预期验证结果：相关自动化验证通过，或计划中记录明确失败原因和处理结果。

### Task #15: 文档和进度更新

**Status:** Designed

**文件：**
- 修改：`docs/exec-plans/active/r028-supabase-business-table-projection.md`
- 修改：`docs/PROGRESS.md`

- 功能：记录 r028 实现状态、真实 Supabase 验收结果和剩余风险。
- 实现说明：只在真实验收完成后记录“验收通过”；用户未确认前不得移动到 `docs/exec-plans/completed/`。如果真实同步仍有未解决的冲突或缺失表，必须记录为未通过。
- 预期验证结果：`docs/PROGRESS.md` 指向当前 r028；active plan 状态与实际实现一致；工作区在提交后干净。
