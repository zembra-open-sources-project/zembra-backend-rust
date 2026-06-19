# r028 Supabase 业务表投影同步

日期：2026-06-19

## 需求结论

将 Supabase 从“只保存 `sync_changes` 同步日志”升级为“同步日志 + 业务表当前状态投影”的双向同步数据库。`sync_changes` 继续作为同步协议、增量游标、冲突判断和重放依据；远端 `notes`、`fields`、`tags`、`note_revisions`、`note_tags`、`note_links` 等业务表需要随 push 更新，作为 Supabase 控制台、调试工具和未来远端查询的当前状态入口。

## 背景

当前后端已经能将本地 `sync_changes` push 到 Supabase，也能从远端 `sync_changes` pull 并 apply 到本地业务表。实际验证中，远端 `sync_changes` 已有数据，但 `notes`、`fields`、`tags` 等业务表仍为空。这说明当前 Supabase 只承担同步日志存储，尚未形成可查询的远端业务状态投影。

## 范围

| 项目 | 结论 |
| --- | --- |
| 同步模型 | `sync_changes` 作为权威同步日志，业务表作为当前状态投影 |
| 投影触发位置 | 后端 push 时先写远端业务表投影，再写远端 `sync_changes`，最后推进本地 push cursor |
| 远端访问方式 | 继续使用 Supabase REST/PostgREST，不直接连接远端 Postgres |
| 第一版投影实体 | `fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links` |
| note 删除/归档语义 | 对 `note insert/update/delete/restore/archive` 均 upsert `notes`，通过 `deleted_at`、`archived_at` 表达软状态 |
| 关系解除语义 | `note_tag detach` 删除远端 `note_tags` 关系行；`note_link detach` 删除远端 `note_links` 关系行 |
| 失败处理 | 业务表投影失败时不写入对应远端 `sync_changes`，不推进本地 push cursor，不标记本地 change 为 committed |
| 幂等性 | 远端业务表 upsert/delete 和 `sync_changes` upsert 必须可重复执行 |

## 验收标准

- 本地已有 pending `sync_changes` push 后，Supabase `sync_changes` 和对应业务表都有可见数据。
- 远端 `fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links` 与本地 change payload 表达的状态一致。
- `note delete/archive/restore` 不物理删除 note，而是更新远端 `notes.deleted_at` 或 `notes.archived_at`。
- `note_tag detach` 和 `note_link detach` 会删除对应远端关系行。
- 任一业务表投影失败时，本地 push cursor 和 `supabase_committed_at` 不推进，下一次 push 可重试。
- 自动化测试覆盖 Supabase client 业务表 upsert/delete 请求构造，以及 sync service 在投影失败时不推进游标。
- 手工验证能在 Supabase 控制台看到业务表当前状态数据。

## 非范围

- 不修改 `zembra-schema`。
- 不新增远端表、字段、索引、约束或 Postgres migration。
- 不使用 Supabase trigger、function 或 Realtime。
- 不直接连接远端 Postgres。
- 不做附件二进制同步。
- 不做多 workspace。
- 不做冲突 UI。
- 不重新设计已有 pull apply 规则。
