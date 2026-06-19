# r009 Supabase 后台同步接入

日期：2026-05-04

> r027 更新：Supabase/Postgres 远端 schema 归属已迁回 `zembra-schema v0.5.0`。本仓运行时只接入 `vendor/zembra-schema/migrations/005_register_unified_postgres_contract.sql` 的 SQLite 版本登记迁移，不再维护本仓 `supabase/migrations/001_initial_sync_schema.sql`；以下正文保留 r009 当时的历史设计上下文。

## 名称说明

用户提到的 `Superbase` 按 `Supabase` 处理，指 Supabase 平台中的 Postgres、服务端密钥和远端同步能力。本需求不引入名为 `Superbase` 的项目内部组件。

## 需求理解

在本地 schema 已升级到 `v0.3.0` 后，启动 Supabase 双向同步开发。本地 SQLite 继续作为主要读写库，Supabase 作为跨设备同步协调层。后端需要在本地业务写入时记录可推送的变更，并通过后台常驻任务按配置频率和 Supabase 交换同步数据。

## 已确认范围

| 项目 | 结论 |
| --- | --- |
| 云端平台 | Supabase |
| 同步模式 | 后台常驻同步 |
| 同步频率 | 通过 `.zembra.env` 配置 |
| Supabase 密钥 | 通过 `.zembra.env` 配置 Supabase service role key |
| workspace 策略 | 第一版继续使用默认 workspace |
| 远端 schema | 本仓库新增 Supabase Postgres migration |
| 第一版同步对象 | `notes`、`note_revisions`、`fields`、`tags`、`note_tags` |
| 暂不覆盖对象 | `attachments`、`note_links` |
| 冲突处理 | 保留所有 note revision，按 `v0.3.0` 规则自动选择 winner，并写入 `sync_conflicts` |
| HTTP API | 新增后端同步 API |

## 配置需求

| 配置项 | 说明 |
| --- | --- |
| Supabase project URL | 后端访问 Supabase REST 或 PostgREST 的基础地址 |
| Supabase service role key | 后端访问 Supabase 的服务端密钥 |
| 同步开关 | 控制后台同步是否启用 |
| 同步频率 | 控制后台常驻任务执行间隔 |

配置继续沿用 `.zembra.env` TOML 文件。密钥只用于本地后端服务，不通过 OpenAPI 响应或普通日志输出。

## API 需求

需要新增后端同步 API，用于手动触发、查看状态或辅助调试后台同步。具体 path、请求体、响应体和错误码在设计文档阶段确定，并同步维护 OpenAPI 标注。

## 验收标准

- 本仓库包含 Supabase Postgres migration 文件。
- `.zembra.env` 支持配置 Supabase project URL、service role key、同步开关和同步频率。
- 后台同步任务能按配置频率常驻运行。
- 第一版同步对象的本地写入能生成 `sync_changes`。
- 后端能把本地 `sync_changes` 推送到 Supabase，并能拉取远端 change 应用到本地。
- note 内容冲突保留全部 revision，按确定性规则选择当前 revision，并记录冲突状态。
- 新增同步 API 已注册 OpenAPI。
- Rust 格式化、编译、测试和 clippy 验证通过。

## 待设计决策

| 决策项 | 设计阶段处理 |
| --- | --- |
| Supabase 访问方式 | 比较 REST/PostgREST 与 SQL 客户端后确定 |
| 后台任务生命周期 | 确定随服务启动、关闭信号和错误重试策略 |
| sync API path | 设计阶段确定具体路由和 DTO |
| 远端 RLS 策略 | 结合 service role key 使用方式确定最小可用策略 |
| sync change payload 格式 | 以 `v0.3.0` schema 为基础定义稳定 JSON 结构 |
