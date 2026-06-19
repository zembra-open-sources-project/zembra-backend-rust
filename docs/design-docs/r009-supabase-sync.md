# r009 Supabase 后台同步接入

日期：2026-05-04

> r027 更新：Supabase/Postgres 远端 schema 归属已迁回 `zembra-schema v0.5.0`。本仓运行时只接入 `vendor/zembra-schema/migrations/005_register_unified_postgres_contract.sql` 的 SQLite 版本登记迁移，不再维护本仓 `supabase/migrations/001_initial_sync_schema.sql`；以下正文保留 r009 当时的历史设计上下文。

需求澄清文档：`docs/request-clarify/r009-supabase-sync.md`

## 核心功能（WHAT）

为 Zembra Rust 后端接入 Supabase 双向同步能力。本地 SQLite 继续作为主要读写库，后端在业务写入时生成 `sync_changes`，后台常驻任务按配置频率把本地 change 推送到 Supabase，并拉取远端 change 应用到本地。

### 需求背景（WHY）

`v0.3.0` schema 已经引入默认 workspace、同步变更日志、同步游标和冲突表。当前后端只完成本地 schema 兼容，还没有把业务写入转成同步事件，也没有远端 Supabase 表结构、后台同步循环和调试 API。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 本地优先 | notes 和 taxonomy API 继续读写 SQLite |
| 后台同步 | 服务启动后按配置频率执行 push/pull |
| 可配置 | `.zembra.env` 配置 Supabase URL、service role key、启用开关和同步频率 |
| 默认 workspace | 第一版仅同步默认 workspace |
| 远端迁移 | 本仓库维护 Supabase Postgres migration |
| 可观测 | 新增同步 API 查询状态并手动触发同步 |
| 冲突收敛 | note revision 全保留，按确定性规则选择 winner，并记录冲突 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| 配置扩展 | 新增 `[sync]` 配置段 |
| Supabase migration | 新增 Postgres DDL，覆盖第一版同步表 |
| 本地 change 生成 | 覆盖 `notes`、`note_revisions`、`fields`、`tags`、`note_tags` |
| 后台 worker | 随服务启动，按频率运行，服务关闭时随 Tokio runtime 停止 |
| push/pull | 基于 `sync_changes` 和 `sync_state` 增量交换 |
| 冲突处理 | note 内容冲突自动保留 revision 并记录 `sync_conflicts` |
| HTTP API | 新增同步状态与手动触发 API，并注册 OpenAPI |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 多 workspace API | 第一版固定默认 workspace |
| Supabase Auth 用户体系 | 第一版由本地后端持有 service role key |
| Realtime 订阅 | 第一版使用周期性后台同步 |
| 附件同步 | 不同步 `attachments` 二进制和元数据 |
| note link 同步 | 第一版不覆盖 `note_links` |
| 冲突 UI | 后端只记录冲突状态 |

## 实现流程（HOW）

### 总体架构

| 模块 | 职责 |
| --- | --- |
| `config` | 解析 sync 配置，隐藏密钥日志输出 |
| `repositories::sync` | 读写 `sync_changes`、`sync_state`、`sync_conflicts`，应用远端 change |
| `services::sync` | 编排 push、pull、run_once、status |
| `sync::supabase` | 通过 Supabase REST/PostgREST 调用远端表 |
| `sync::worker` | 后台常驻任务，按配置频率调用 sync service |
| `handlers::sync` | 暴露 HTTP API |
| `supabase/migrations` | 维护 Postgres 远端 schema |

### Supabase 访问方式

推荐使用 Supabase REST/PostgREST，不直接连接远端 Postgres。Supabase REST API 会基于数据库 schema 自动暴露表接口，请求使用 project URL 下的 `/rest/v1/`。本地后端持有 service role key，用于服务端同步任务。

| 项目 | 决策 |
| --- | --- |
| 访问协议 | HTTPS REST/PostgREST |
| Rust HTTP client | `reqwest` |
| 认证 header | `apikey` 和 `Authorization: Bearer <service_role_key>` |
| 密钥来源 | `.zembra.env` |
| 日志策略 | 不打印 key，不把 key 放入错误响应 |

说明：Supabase 当前文档建议服务端优先使用新式 secret key，用户本轮明确要求 service role key，因此第一版配置字段按 `service_role_key` 命名。

### 配置设计

在 `.zembra.env` 和 `.env.example` 中新增：

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `sync.enabled` | bool | `false` | 是否启动后台同步 |
| `sync.interval_seconds` | u64 | `60` | 后台同步间隔 |
| `sync.supabase_url` | string | 空 | Supabase project URL |
| `sync.service_role_key` | string | 空 | Supabase service role key |

约束：

| 规则 | 说明 |
| --- | --- |
| `enabled=false` | 允许缺少 URL 和 key |
| `enabled=true` | URL 和 key 必须非空 |
| `interval_seconds` | 最小值建议为 `5` 秒 |

### Supabase Postgres Migration

新增目录 `supabase/migrations/`，第一版创建与本地同步相关的远端表：

| 表 | 说明 |
| --- | --- |
| `workspaces` | 默认 workspace 和后续 workspace 元数据 |
| `fields` | field 元数据 |
| `tags` | tag 元数据 |
| `notes` | note 主记录 |
| `note_revisions` | note revision 历史 |
| `note_tags` | note/tag 关联 |
| `devices` | 设备元数据 |
| `sync_changes` | 同步变更事实表 |

Postgres 类型对齐 `vendor/zembra-schema/proposals/003-bidirectional-supabase-sync-schema.md`，其中 workspace 使用 `uuid`，业务 ID 和 change ID 使用 `text`，payload 使用 `jsonb`，时间戳使用 `bigint`。

RLS 策略：

| 阶段 | 策略 |
| --- | --- |
| 第一版 | 不面向客户端暴露同步表，使用 service role key 访问 |
| 后续 | 接入 Supabase Auth 后再按 workspace/user 建立 RLS |

### Sync Change Payload

第一版 payload 使用稳定 JSON 快照，按实体类型区分字段。

| entity_type | operation | payload 内容 |
| --- | --- | --- |
| `field` | `insert` | field 全量快照 |
| `tag` | `insert` | tag 全量快照 |
| `note` | `insert/update/delete/restore` | note 主记录快照 |
| `note_revision` | `insert` | revision 全量快照 |
| `note_tag` | `attach/detach` | `note_id`、`tag_id`、`created_at` |

本地写入必须在同一个 SQLite transaction 中完成业务表更新和 `sync_changes` 写入。change ID 推荐使用 UUID v4，后续如需要全局有序 ID 再切换 ULID。

### Push 流程

| 步骤 | 说明 |
| --- | --- |
| 1 | 从本地 `sync_state(scope='push')` 读取游标 |
| 2 | 按 `(created_at, id)` 查询未推送的本地 `sync_changes` |
| 3 | 批量 upsert 到 Supabase `sync_changes` |
| 4 | 推送成功后更新本地 push 游标和 `supabase_committed_at` |
| 5 | 失败时记录 `last_error_at` 和 `last_error_message` |

### Pull 流程

| 步骤 | 说明 |
| --- | --- |
| 1 | 从本地 `sync_state(scope='pull')` 读取游标 |
| 2 | 从 Supabase 拉取默认 workspace 下更晚的 `sync_changes`，排除本设备已产生的 change |
| 3 | 对每条远端 change 做幂等检查 |
| 4 | 在 SQLite transaction 中应用业务表变更，写入本地 `sync_changes` 或标记已应用 |
| 5 | 更新 pull 游标 |

### 冲突处理

| 冲突 | 处理 |
| --- | --- |
| 并发 note edit | 插入所有 revision，按最大 `(created_at, device_id, revision_id)` 选择 `current_revision_id` |
| delete vs update | 保留 revision，note 标记 `conflict_status='needs_review'`，记录 `sync_conflicts` |
| tag attach vs detach | 按 change 顺序应用，无法判断时记录 `relation_attach_vs_detach` |
| schema incompatible | 不应用业务表，记录 `schema_incompatible` |

### 后台 Worker

worker 随服务启动创建，只在 `sync.enabled=true` 时运行。每轮执行 `run_once`，内部顺序为 push 后 pull。失败不终止服务，写日志并更新 sync state。下一轮按配置间隔继续运行。

### HTTP API

| Method | Path | 用途 |
| --- | --- | --- |
| `GET` | `/sync/status` | 查看后台同步配置摘要、最近成功/失败时间、游标 |
| `POST` | `/sync/run` | 手动触发一次 push + pull |
| `POST` | `/sync/push` | 手动触发 push |
| `POST` | `/sync/pull` | 手动触发 pull |

响应不返回 Supabase key。API 需要注册 `#[utoipa::path]` 和 `src/api_doc.rs`。

## 测试用例

### 编译检查

| 用例 | 预期 |
| --- | --- |
| `cargo fmt --check` | 通过 |
| `cargo check` | 通过 |
| `cargo test` | 通过 |
| `cargo clippy -- -D warnings` | 通过 |

### 自动化测试

| 用例 | 预期 |
| --- | --- |
| sync 配置默认值 | 未配置时 `enabled=false`，服务可启动 |
| sync 配置校验 | `enabled=true` 且缺少 URL/key 时启动失败 |
| 本地创建 note | 生成 `note` 和 `note_revision` change |
| 本地 tag attach/detach | 生成 `note_tag` change |
| push 游标更新 | mock Supabase 成功后推进 push cursor |
| pull 幂等应用 | 重复远端 change 不重复写业务表 |
| note revision 冲突 | 保留全部 revision，并选择确定性 winner |
| sync API OpenAPI | `/api-docs/openapi.json` 包含 `/sync/status`、`/sync/run`、`/sync/push`、`/sync/pull` |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| 本地服务启动 | `sync.enabled=false` 时无后台同步错误 |
| Supabase 配置启动 | `sync.enabled=true` 时后台 worker 按间隔输出同步摘要 |
| OpenAPI | `/swagger-ui` 可查看 sync API |
