# r033 workspace 隔离机制设计文档

日期：2026-06-25

需求澄清文档：`docs/request-clarify/r033-workspace-isolation.md`

## 核心功能（WHAT）

补齐真实 workspace 隔离机制，让本地初始化、历史固定 workspace 迁移、多 workspace 同步和 workspace 列表 API 都以 `workspaces.id` 作为长期隔离边界。

### 需求背景（WHY）

当前后端使用固定 `DEFAULT_WORKSPACE_ID` 兼容旧单机数据，这和 schema proposal 中“真实用户数据库由应用层生成默认 workspace UUID”的设计不一致。固定 workspace 在单库单用户路径中能工作，但在本地重新初始化且远端已有数据时会制造同 id 不同元数据的系统性冲突。同步差异算法在缺少 workspace 对应 `sync_changes` 时停止是正确保护，真正需要修的是 workspace 身份生成、迁移和多 workspace 同步边界。

### 需求目标（GOAL）

新初始化真实用户数据库时生成随机 UUID workspace；已有固定 workspace 迁移为本地新的随机 UUID；同步过程可以拉取和写入多个 workspace 的数据；新增 `GET /workspaces` 返回 workspace 摘要列表，供客户端展示和后续按完整 workspace id 进行操作。

### 范围边界

纳入范围：初始化生成随机 workspace、固定 workspace 本地迁移、多 workspace 同步读取过滤调整、workspace 摘要查询、`GET /workspaces` handler、DTO、OpenAPI 和测试。非范围：不新增 workspace 创建、重命名、删除 API；不新增 workspace 成员权限、Supabase Auth 或 RLS；不做前端 workspace 切换 UI；不做跨 workspace 移动笔记。

## 实现流程（HOW）

新增 workspace 领域能力，集中提供 workspace UUID 生成、固定 workspace 识别、短 hash 派生和 workspace 摘要查询。真实用户初始化路径不再依赖固定 `DEFAULT_WORKSPACE_ID` 创建 workspace，而是在创建并迁移数据库后确保存在一个随机 UUID workspace。测试或旧兼容常量可以保留为“legacy fixed workspace id”，但不能作为真实初始化身份继续扩散到业务路径。

历史固定 workspace 迁移只处理本地数据库。迁移检测到 `workspaces.id = 00000000-0000-4000-8000-000000000300` 时，为本地生成一个新的随机 UUID，并在本地事务内更新 `workspaces` 以及所有引用 `workspace_id` 的业务表、同步表和本地状态表。该迁移不主动写远端，不把本地新 UUID 推成远端覆盖，也不把远端已有 workspace 改名或改 id。同步后续按多 workspace 差异规则把远端真实 workspace 拉回本地。

同步读取从“业务表按固定默认 workspace 过滤”改为“按需要读取所有可同步 workspace 的数据”。`workspaces` 作为根表全量读取；带 `workspace_id` 的业务表和 `sync_changes` 不再绑定固定默认 workspace，而是按远端返回的 workspace 集合或无固定 workspace 过滤读取。写入仍保持外键安全顺序：`workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links`、`sync_changes`。差异比较继续在无法判断字段新旧时返回冲突，不用静默覆盖。

新增 `GET /workspaces`。Repository 使用 `workspaces` 左连接可见 notes，按 workspace 聚合 `visible_note_count` 和 `latest_note_created_at`。可见 notes 条件固定为 `notes.deleted_at IS NULL AND notes.archived_at IS NULL`。排序为 `visible_note_count DESC`，再按 `latest_note_created_at DESC NULLS LAST`，最后按 `workspace_id ASC`。`short_hash` 在服务或 DTO 层由 `workspace_id.replace("-", "")[..8]` 派生，输入不足 8 位时返回完整去连字符字符串并由测试覆盖异常兼容。

API 响应字段如下：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `workspace_id` | string | 完整 workspace UUID，真实标识 |
| `short_hash` | string | UUID 去掉连字符后的前 8 位，只用于展示 |
| `visible_note_count` | integer | 未删除且未归档笔记数量 |
| `latest_note_created_at` | integer 或 null | 最近一条可见笔记创建时间，空 workspace 返回 `null` |

## 测试用例

最终验收必须在 sandbox 外启动 backend，并确认后台同步不再出现由 workspace id 引起的同步错误。这是本需求通过验收的唯一标准；下列表格中的自动化和接口检查只作为支撑验证，不能替代 sandbox 外真实启动验收。

| 场景 | 预期 |
| --- | --- |
| 新库执行初始化 | `workspaces.id` 为随机 UUID，不等于固定 legacy id |
| 固定 legacy workspace 本地迁移 | 本地所有 `workspace_id` 引用更新为同一个新 UUID，远端不被写入 |
| 多 workspace 远端快照读取 | Supabase 业务表读取不再只过滤固定 workspace |
| `/workspaces` 有多 workspace | 按可见笔记数量降序返回，数量相同按最近可见笔记时间和 workspace id 稳定排序 |
| `/workspaces` 包含已归档或已删除笔记 | `visible_note_count` 和 `latest_note_created_at` 均排除这些笔记 |
| 空 workspace | `visible_note_count = 0` 且 `latest_note_created_at = null` |
| OpenAPI JSON | 包含 `GET /workspaces` path 和响应 schema |
