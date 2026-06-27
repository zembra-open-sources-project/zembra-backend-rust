# r037 同步实体生命周期需求澄清

日期：2026-06-27

本次需求目标是补全本地 SQLite 与 Supabase 真实双向同步中的实体生命周期语义，让同步系统能够稳定处理九张同步表的存在、更新、软删除、物理删除、关系解除和同步事实补齐。当前同步仍以“业务 row 缺失就补齐、业务 row 字段不同就按 `sync_changes.created_at` 判断方向”为核心模型；该模型无法表达主表物理删除后的 tombstone 状态，导致删除两个空 field 后，本地 `fields` 行消失但远端仍存在，同步随后把远端旧行视为需要拉回本地或在时间无法区分时报告 `synchronization conflict count 2`。本需求不是 field 删除接口的局部补丁，而是同步系统对实体生命周期的完整建模。

本次需求必须覆盖当前同步对象中的九张表：`workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links` 和 `sync_changes`。禁止只针对 `fields` 或任何少数实体做最小化实现，禁止保留“未覆盖实体继续按缺行默认补行”的隐含规则。每张同步表必须在设计中拥有明确的生命周期策略，并进入同一套状态归纳、动作生成、执行和收敛验证流程。

同步层需要把业务表当前投影和 `sync_changes` 历史事实分开处理。业务表 row 表达实体当前可查询投影；`sync_changes` 表达实体历史事实、操作顺序和物理删除后的 tombstone。`sync_changes` 不能继续像普通业务实体一样参与自己的新旧判断；它应按 change id 补齐两端事实集合，并为业务实体状态判断提供依据。对于业务实体，最新 change 的 `entity_type`、`entity_id`、`operation`、`created_at` 和 change id 都属于判断实体生命周期状态的输入。

同步实体至少需要归入四类生命周期：软删除实体、物理删除实体、关系实体和只追加事实。软删除实体的删除状态由 row 内字段表达，row 仍应保留并通过 upsert 收敛；物理删除实体的删除状态由业务 row 不存在加最新 delete tombstone 表达；关系实体的存在由关系 row 表达，解除关系由 detach 或 delete 事实表达；`sync_changes` 是只追加事实，缺失时只补齐事实本身，不用自己的 `entity_type='sync_change'` 记录判断新旧。每类实体的状态、动作和执行方式必须显式定义。

每个业务实体在本地和远端都需要先归纳为生命周期状态，再进入统一状态矩阵。状态至少包括 `Present`、`SoftDeleted`、`Tombstoned`、`AbsentUnknown` 和 `Inconsistent`。`Present` 表示业务 row 存在且最新事实不表示物理删除；`SoftDeleted` 表示业务 row 存在且 row 内删除态或归档态生效；`Tombstoned` 表示业务 row 不存在且最新事实是 delete 或 detach；`AbsentUnknown` 表示业务 row 不存在且没有可证明删除的事实；`Inconsistent` 表示业务 row 与最新事实互相矛盾，例如 row 仍存在但最新事实是物理 delete。

差异计算输出必须从当前的两组 upsert 列表升级为动作集合。动作至少包括 `UpsertLocal`、`UpsertRemote`、`DeleteLocal`、`DeleteRemote`、`SyncChangeLocal`、`SyncChangeRemote` 和 `Conflict`。`UpsertLocal` 和 `UpsertRemote` 表示用一侧当前投影修正另一侧；`DeleteLocal` 和 `DeleteRemote` 表示根据另一侧 tombstone 或 detach 删除当前投影；`SyncChangeLocal` 和 `SyncChangeRemote` 表示只补齐缺失的同步事实；`Conflict` 表示两端事实不足以推出唯一当前状态，必须停止该实体同步并向上报告。

实体策略必须覆盖九张同步表。`notes` 和 `workspaces` 按 row 内删除态处理，不应通过物理删除表达业务删除；`fields` 按物理删除实体处理，delete tombstone 是删除后的唯一事实来源；`note_tags` 和 `note_links` 按关系实体处理，detach 或 delete 事实表示关系消失；`note_revisions` 按只追加或随 note 生命周期保留的实体处理，缺失时补齐，不应通过普通物理删除同步；`sync_changes` 按只追加事实处理。`devices` 和 `tags` 的删除语义必须在设计中明确为支持删除、禁止删除或软删除之一，禁止在实现中继续沿用未定义时默认补行的行为。

同步执行顺序必须同时考虑 upsert、delete、关系解除、物理删除依赖和 `sync_changes` 补齐。物理删除主表前必须先处理会阻塞删除的依赖关系，例如 field 删除前需要处理 notes 的 `field_id` 引用，tag 删除前需要处理层级关系和 `note_tags` 关系。执行器不能只依赖固定表 upsert 顺序，必须按动作类型和实体依赖顺序执行，并在写入后重新读取两端快照，用同一套状态归纳和动作生成逻辑验证收敛。

本需求不新增、维护、复制或演化数据库 schema，不在本仓库补 migration、DDL、字段、索引、约束或触发器。任何必须改变数据库契约才能完整表达的实体删除语义，都应先停止本仓实现并转到 `zembra-schema` 完成契约设计。本需求允许调整后端同步代码对现有 `vendor/zembra-schema` 契约的消费方式，允许新增同步层状态模型、动作模型、实体策略和本地/远端执行能力。

本需求必须保留现有 `/sync/run`、`/sync/push` 和 `/sync/pull` 的外部入口语义，但三者必须共享同一套实体生命周期 diff 和动作执行模型。`/sync/run` 执行完整双向收敛；`/sync/push` 只执行目标为远端的动作和必要的事实补齐；`/sync/pull` 只执行目标为本地的动作和必要的事实补齐。三个入口都不能再各自维护一套局部判断逻辑。

验收标准：九张同步表都有明确生命周期策略；同步 diff 不再把所有缺行默认解释为需要补齐；物理删除、软删除、关系解除和事实补齐都通过统一动作模型表达；field delete 后本地 tombstone 与远端 present 能收敛为远端删除而不是冲突或拉回旧 row；`sync_changes` 只作为事实集合补齐和业务实体状态判断输入，不再作为普通业务实体判断自己的 freshness；同步完成后必须通过二次快照读取和同一状态机验证无剩余动作、无冲突。
