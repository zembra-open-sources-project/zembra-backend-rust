# r028 Supabase 真实双向同步

日期：2026-06-19

## 需求结论

本需求目标是实现本地 SQLite 与 Supabase 之间的真实双向同步。同步不是单纯推送 `sync_changes`，也不是只把本地 pending changes 投影到远端业务表；同步必须先读取本地和 Supabase 两端的当前数据，比较差异，再把差异同步到另一端。验收只以真实 Supabase 数据结果为准：先确认本地已有数据完整同步到 Supabase，再确认新数据也能同步到 Supabase。单元测试、mock 请求、只看接口返回成功或只看 `sync_changes` 都不能作为验收通过依据。

## 背景

当前后端已经具备部分 Supabase 同步能力，远端 `sync_changes` 中已有数据，但 Supabase 业务表仍为空或不完整。这说明现有实现没有完成真实数据同步，只保存了同步日志。用户明确要求同步的是所有指定表的真实数据，而不是日志投影、临时补偿或只处理新增 pending change 的路径。

## 同步表范围

本需求只同步以下 9 张表：

| 表 | 同步要求 |
| --- | --- |
| `workspaces` | 本地与远端双向对齐 |
| `devices` | 本地与远端双向对齐 |
| `fields` | 本地与远端双向对齐 |
| `tags` | 本地与远端双向对齐 |
| `notes` | 本地与远端双向对齐，软删除和归档状态也必须同步 |
| `note_revisions` | 本地与远端双向对齐 |
| `note_tags` | 本地与远端双向对齐 |
| `note_links` | 本地与远端双向对齐 |
| `sync_changes` | 本地与远端双向对齐，并作为变更时间顺序依据 |

明确不纳入同步表范围：

| 表 | 结论 |
| --- | --- |
| `sync_state` | 不参与两端数据同步，只作为本地同步游标或运行状态 |
| `sync_conflicts` | 不参与两端数据同步，只作为冲突记录 |

## 同步语义

| 场景 | 处理 |
| --- | --- |
| 本地有、Supabase 没有 | 推送到 Supabase |
| Supabase 有、本地没有 | 拉取到本地 |
| 两边都有且字段相同 | 不处理 |
| 两边都有但字段不同 | 以 `sync_changes.created_at` 时间顺序判断，较新的 change 对应状态覆盖较旧状态 |
| 无法用 `sync_changes.created_at` 判断先后 | 不允许静默覆盖，必须记录冲突或返回错误停止 |
| 关系表两端不一致 | 按同样规则同步 `note_tags`、`note_links` 的增删差异 |
| note 软删除或归档状态不一致 | 按 `sync_changes.created_at` 判断最终 `deleted_at`、`archived_at` 状态 |

同步必须以表数据真实一致为目标。`sync_changes` 是判断变更顺序的依据之一，但不能替代对本地表和 Supabase 远端表的实际读取和差异比较。

## 真实同步流程要求

1. 读取本地 9 张同步表的当前数据。
2. 读取 Supabase 远端 9 张同步表的当前数据。
3. 按表主键或复合主键建立对应关系。
4. 计算本地缺失、远端缺失、两端字段不同三类差异。
5. 根据差异写入本地或远端。
6. 字段冲突必须按 `sync_changes.created_at` 判断方向。
7. 无法判断方向时停止并记录冲突，禁止自动乱覆盖。
8. 同步完成后再次读取两端数据，确认 9 张表达到一致状态。

## 验收标准

验收只接受真实 Supabase 同步结果，必须按顺序满足以下两条：

1. 本地已有数据必须完整、真实同步到 Supabase。验收时需要在 Supabase 中看到本地已有 `workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links`、`sync_changes` 对应数据。
2. 在第一条通过后，新创建的数据也必须能够同步。验收时需要先创建一条新数据，再确认这条新数据对应的业务表记录和 `sync_changes` 都真实出现在 Supabase。

以下内容不能作为验收通过标准：

- 本地单元测试通过。
- mock Supabase 请求通过。
- 只看到 `/sync/push`、`/sync/run` 返回成功。
- 只看到 Supabase `sync_changes` 有数据。
- 使用临时随机数据替代本地已有数据。
- 无条件重放 committed changes，而不比较本地和远端真实差异。

## 非范围

- 不修改 `zembra-schema`。
- 不新增远端表、字段、索引、约束或 Postgres migration。
- 不使用 Supabase trigger、function 或 Realtime。
- 不直接连接远端 Postgres。
- 不做附件二进制同步。
- 不做多 workspace。
- 不做冲突 UI。
