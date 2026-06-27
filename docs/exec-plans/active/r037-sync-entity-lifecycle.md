# r037 同步实体生命周期执行计划

需求澄清文档：`docs/request-clarify/r037-sync-entity-lifecycle.md`

设计文档：`docs/design-docs/r037-sync-entity-lifecycle.md`

## 执行结果

已完成生命周期动作模型、双端动作执行器、同步入口改造、真实 Supabase 验证脚本口径修正和回归测试。当前实现严格切换为动作集合，不保留旧 diff 字段兼容层；`fields`、`note_tags`、`note_links` 可根据 tombstone 执行删除或解除关系，`notes` 继续用软删除 row upsert 表达生命周期，`sync_changes` 按 change id 作为只追加事实补齐，`devices` 与 `tags` 作为保留实体遇到未定义 delete 事实会进入冲突路径。真实 Supabase 验证中，用户删除两个空 field 后触发的 `synchronization conflict count 2` 已消除，第二次 `/sync/run` 返回 `{"pulled":0,"pushed":0}`；验证脚本仍按九张同步表完整比较业务数据，只跳过本地独有且没有任何同步依赖行的初始化空 workspace。

验证记录：

- `cargo fmt --check`：通过。
- `cargo test sync::diff`：通过，12 个 diff 生命周期测试全部通过。
- `cargo test repositories::sync`：通过，9 个本地同步仓储测试全部通过。
- `cargo test sync::supabase`：通过，12 个 Supabase 请求构造与边界测试全部通过。
- `cargo test --test sync_routes`：通过，2 个同步路由测试全部通过。
- `cargo clippy -- -D warnings`：通过。
- `cargo test`：通过，92 个 lib 测试、70 个集成测试和 doctest 全部通过。
- `conda run -n mlx bash scripts/verify_supabase_real_sync.sh`：通过，本地和远端 schema contract 均为 `0.5.0`，九张同步表在业务数据口径下同步前后全部一致，`/sync/run` 返回 `{"pulled":0,"pushed":0}`。

## Stage #1: 生命周期模型与差异动作

### Task #1: 定义同步实体生命周期策略和状态模型

**Status:** Finished

**Files:** Create/Modify `src/sync/diff.rs`, `src/sync/table_snapshot.rs`, tests in `src/sync/diff.rs`

功能 / 实现说明 / 预期验证结果：为九张同步表定义显式生命周期策略，覆盖 `workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links` 和 `sync_changes`。策略必须区分软删除实体、物理删除实体、关系实体、保留实体和只追加事实；`devices` 与 `tags` 当前按保留实体处理，遇到未定义 delete 事实必须生成冲突而不是静默补行。新增实体状态归纳能力，状态至少包含 `Present`、`SoftDeleted`、`Tombstoned`、`AbsentUnknown` 和 `Inconsistent`。预期验证结果是单元测试能证明九张表均有策略覆盖，缺策略或未知 delete 事实会失败，且 field 本地缺 row 加 `field/delete` change 能被归纳为 tombstone。

### Task #2: 将 diff 输出从 upsert 列表改为动作集合

**Status:** Finished

**Files:** Modify `src/sync/diff.rs`, `src/sync/table_snapshot.rs`, tests in `src/sync/diff.rs`

功能 / 实现说明 / 预期验证结果：替换或包裹现有 `SyncSnapshotDiff` 的 `write_local` / `write_remote` 模型，新增统一动作集合，动作至少包含 `UpsertLocal`、`UpsertRemote`、`DeleteLocal`、`DeleteRemote`、`SyncChangeLocal`、`SyncChangeRemote` 和 `Conflict`。动作需要携带表名、实体 key、目标侧和原因，原因用于测试与诊断，不改变后台日志 count-only 口径。`sync_changes` 不再通过 `entity_type = "sync_change"` 参与自己的 freshness 判断，只按 change id 生成事实补齐动作。预期验证结果是现有缺行测试被改写为生命周期语义测试，field tombstone 对远端 present 生成 `DeleteRemote`，远端 tombstone 对本地 present 生成 `DeleteLocal`，单边 change 缺失只生成 `SyncChangeLocal` 或 `SyncChangeRemote`。

### Task #3: 完成状态矩阵和冲突规则覆盖

**Status:** Finished

**Files:** Modify `src/sync/diff.rs`, tests in `src/sync/diff.rs`

功能 / 实现说明 / 预期验证结果：实现设计文档中的状态矩阵，覆盖 present、soft-deleted、tombstoned、absent-unknown、inconsistent 的组合。两端最新事实时间相同且 change id、operation、状态一致时不冲突；时间相同但 operation、row 或状态无法推出唯一结果时生成冲突。保留实体出现 delete 事实、row 与最新 tombstone 矛盾、物理删除实体仍有可见依赖阻塞时必须生成冲突。预期验证结果是新增单元测试覆盖 equal timestamp 同事实不冲突、equal timestamp 不同事实冲突、保留实体 delete 事实冲突、notes 软删除生成 upsert 而非 delete、note_tags 和 note_links detach 生成关系删除动作。

### Task #4: Stage #1 验证、计划回写和提交

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r037-sync-entity-lifecycle.md`

功能 / 实现说明 / 预期验证结果：运行 `cargo fmt --check`、`cargo test sync::diff` 和 `cargo check`。验证通过后把 Stage #1 已完成任务状态更新为 `Finished`，记录验证命令和结果，按项目规则 stage、commit 并尝试推送。预期结果是生命周期 diff 模型独立测试通过，工作区只包含 Stage #1 相关改动。

## Stage #2: 本地与远端动作执行器

### Task #1: 扩展本地 SQLite 动作执行能力

**Status:** Finished

**Files:** Modify `src/repositories/sync/write_snapshot.rs`, `src/repositories/sync/tests.rs`, possibly create helper module under `src/repositories/sync/`

功能 / 实现说明 / 预期验证结果：将本地写入能力从只接收 partial snapshot upsert 扩展为接收生命周期动作。`UpsertLocal` 继续复用现有 upsert 能力；`DeleteLocal` 根据实体策略执行删除或关系解除；`SyncChangeLocal` 按 change id 幂等补齐事实。field 本地删除必须先清理同 workspace 下已删除或已归档 notes 的 `field_id`，如果仍有可见 note 引用该 field，则返回冲突或执行失败并回滚，不允许改写可见 note。预期验证结果是 repository 测试覆盖本地 field delete 动作、note_tags detach、note_links detach、sync_changes 幂等补齐、本地事务失败不部分提交。

### Task #2: 扩展 Supabase 远端动作执行能力

**Status:** Finished

**Files:** Modify `src/sync/supabase.rs`, tests in `src/sync/supabase.rs`

功能 / 实现说明 / 预期验证结果：将远端写入能力从 `upsert_table_snapshot` 扩展为可执行 upsert、delete 和 sync-change 补齐动作。保留现有 `note_tags` 和 `note_links` DELETE 请求能力，并新增 `fields` DELETE 请求能力；后续若策略不支持物理删除的实体出现 delete 动作，应在动作生成层冲突，远端执行器不做兜底删除。field 远端删除前需要按现有契约处理 notes 引用；如果无法在 REST 层安全清理依赖或发现仍有可见 note 引用，必须返回明确错误并阻止同步成功。预期验证结果是 Supabase 请求构造测试覆盖 fields DELETE 过滤条件、note_tags DELETE、note_links DELETE、sync_changes upsert，以及不支持实体不生成 DELETE 请求。

### Task #3: 按依赖顺序调度动作执行

**Status:** Finished

**Files:** Modify `src/services/sync.rs`, `src/sync/diff.rs`, tests in `src/sync/diff.rs` or service-level tests

功能 / 实现说明 / 预期验证结果：在 service 层或专用执行调度层按动作类型和实体依赖排序执行动作。顺序必须覆盖上游主表 upsert、软删除状态 upsert、关系删除、物理删除依赖解除、主表物理删除、剩余主表 upsert、关系 upsert、最后补齐 `sync_changes`。该顺序不能退化回固定表 upsert 顺序。预期验证结果是排序测试证明关系删除早于关系 upsert，field 删除依赖处理早于 fields DELETE，`sync_changes` 补齐晚于业务投影动作。

### Task #4: Stage #2 验证、计划回写和提交

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r037-sync-entity-lifecycle.md`

功能 / 实现说明 / 预期验证结果：运行 `cargo fmt --check`、`cargo test repositories::sync`、`cargo test sync::supabase`、`cargo test sync::diff` 和 `cargo check`。验证通过后把 Stage #2 已完成任务状态更新为 `Finished`，记录验证命令和结果，按项目规则 stage、commit 并尝试推送。预期结果是本地和远端动作执行能力通过定向测试，工作区只包含 Stage #2 相关改动。

## Stage #3: 同步入口改造与收敛验证

### Task #1: 改造 `/sync/run` 使用生命周期动作模型

**Status:** Finished

**Files:** Modify `src/services/sync.rs`, `tests/sync_routes.rs`, related service tests if present

功能 / 实现说明 / 预期验证结果：将 `SyncService::run_once` 和内部双向同步流程改为读取两端快照、生成生命周期动作、检查冲突、按调度器执行本地和远端动作、重新读取快照并用同一模型验证收敛。返回 summary 的 pushed/pulled 数量应统计实际执行的远端目标动作和本地目标动作，不能只统计 upsert row 数。预期验证结果是 `/sync/run` 相关测试仍可调用，field tombstone 场景不会生成冲突或拉回旧 row，执行后剩余动作为空。

### Task #2: 改造 `/sync/push` 和 `/sync/pull` 共享同一模型

**Status:** Finished

**Files:** Modify `src/services/sync.rs`, `tests/sync_routes.rs`

功能 / 实现说明 / 预期验证结果：`push` 和 `pull` 不再各自基于旧 `write_remote` / `write_local` 列表执行。`push` 只执行目标为远端的业务动作和远端缺失的必要事实补齐；`pull` 只执行目标为本地的业务动作和本地缺失的必要事实补齐。两者都必须使用同一生命周期 diff、同一冲突判断和同一收敛验证逻辑。预期验证结果是定向测试覆盖 push-only field delete 传播到远端、pull-only field delete 传播到本地、push/pull 在存在反方向动作时不会误执行。

### Task #3: 保持后台日志和错误口径稳定

**Status:** Finished

**Files:** Modify `src/services/sync.rs`, `src/sync/worker.rs` if needed, tests if present

功能 / 实现说明 / 预期验证结果：生命周期模型可以携带详细动作原因，但后台同步失败日志仍保持 `synchronization conflict count N` 这类 count-only 口径，不重新输出完整冲突详情。`SyncError::Conflict` 可以继续只暴露 count；`NotConverged` 需要能表达剩余动作数和冲突数。预期验证结果是错误格式相关测试或定向断言确认日志/错误消息不退回 verbose 模式，真实冲突仍能返回正确数量。

### Task #4: Stage #3 验证、计划回写和提交

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r037-sync-entity-lifecycle.md`

功能 / 实现说明 / 预期验证结果：运行 `cargo fmt --check`、`cargo test --test sync_routes`、`cargo test sync::diff`、`cargo test sync::supabase`、`cargo test repositories::sync` 和 `cargo check`。验证通过后把 Stage #3 已完成任务状态更新为 `Finished`，记录验证命令和结果，按项目规则 stage、commit 并尝试推送。预期结果是三个同步入口共享生命周期模型并通过路由和定向测试。

## Stage #4: 端到端回归、真实 Supabase 验证和收尾

### Task #1: 补齐高风险同步回归测试

**Status:** Finished

**Files:** Modify `src/sync/diff.rs`, `src/repositories/sync/tests.rs`, `tests/sync_routes.rs`, `tests/notes_taxonomy_routes.rs` if needed

功能 / 实现说明 / 预期验证结果：补齐覆盖设计文档测试矩阵的自动化测试，重点覆盖九张表策略覆盖、field tombstone 双向传播、notes 软删除、note_tags detach、note_links detach、保留实体 delete 事实冲突、equal timestamp 同事实不冲突、equal timestamp 不同事实冲突、本地事务回滚、远端 DELETE 请求构造、run/push/pull 入口行为。预期验证结果是同步相关定向测试能独立证明不再存在未经过生命周期策略的缺行默认补行路径。

### Task #2: 执行真实 Supabase field 删除同步验证

**Status:** Finished

**Files:** Verify `scripts/verify_supabase_real_sync.sh` or create/modify a dedicated verification script only if existing scripts cannot cover r037, update docs with verification result

功能 / 实现说明 / 预期验证结果：在真实 Supabase 环境中验证本地删除空 field 后，远端 `fields` row 被删除，两端保留一致 `field/delete` tombstone，随后至少再经过一轮后台同步或手动 `/sync/run` 后无新增冲突。验证还要回归 notes 软删除不物理删除、note_tags detach 和 note_links detach 两端关系收敛。预期验证结果是真实远端和本地九张同步表通过生命周期模型无剩余动作、无冲突。

### Task #3: 完整质量检查和文档同步

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r037-sync-entity-lifecycle.md`, `docs/PROGRESS.md`, docs if behavior documentation needs sync

功能 / 实现说明 / 预期验证结果：运行完整验证：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`。执行回归搜索，确认不存在未经过生命周期策略的普通缺行补齐分支，确认 `sync_changes` 不再通过 `entity_type = "sync_change"` 判断自己的新旧，确认不存在 field-only 特殊同步分支。必要时同步更新用户可见同步说明文档，但不改 generated 文档。预期结果是完整回归通过，文档记录真实验证结果和剩余风险。

### Task #4: Stage #4 收尾记录、提交和推送

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r037-sync-entity-lifecycle.md`, `docs/PROGRESS.md`

功能 / 实现说明 / 预期验证结果：使用收尾记录技能更新 `docs/PROGRESS.md`，执行计划保持在 active，等待用户验收后再归档。按项目规则 stage、commit 并尝试推送。预期结果是 r037 完整实现、自动化测试、真实 Supabase 验证和进度记录完成，远端分支收到对应提交，执行计划未在未经用户验收前移动到 completed。
