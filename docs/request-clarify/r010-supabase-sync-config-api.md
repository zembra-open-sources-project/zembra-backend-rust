# r010 Supabase 同步配置 API

日期：2026-05-10

## 需求背景

当前 Supabase 同步能力已经实现，后端可从 `~/.zembra.env` 读取 `[sync]` 配置，并已提供同步状态与手动触发 API。前端还缺少直接配置 Supabase project URL、service role key、同步开关和同步频率的后端接口。

## 需求理解

新增一组 Supabase 同步配置 API，让前端可以读取同步配置摘要、保存同步配置，并在保存前测试 Supabase 连接。配置变更需要写回 `~/.zembra.env`，并立即影响手动同步 API。

## 已确认范围

| 项目 | 结论 |
| --- | --- |
| 配置持久化 | 写回 `~/.zembra.env` |
| 配置格式 | 继续使用 TOML `[sync]` 配置段 |
| 密钥回显 | 不返回 `service_role_key` 明文 |
| 密钥状态 | 返回是否已配置 service role key |
| 密钥覆盖 | 提交新 key 时按覆盖处理，前端负责提示用户确认覆盖 |
| 手动同步生效 | 保存后立即影响 `/sync/run`、`/sync/push`、`/sync/pull` |
| 后台 worker 生效 | 本轮不强制要求确认后台 worker 热更新行为，设计阶段明确实现策略 |
| 测试连接 | 新增 `POST /sync/config/test` |
| OpenAPI | 新增或修改 API 必须同步维护 `#[utoipa::path]` 和 `ApiDoc` |

## API 范围

| Method | Path | 用途 |
| --- | --- | --- |
| `GET` | `/sync/config` | 读取同步配置摘要，不返回 key 明文 |
| `PUT` | `/sync/config` | 保存 Supabase 同步配置并写回 `~/.zembra.env` |
| `POST` | `/sync/config/test` | 使用请求配置或已保存配置测试 Supabase REST 连接 |

## 配置字段

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `enabled` | bool | 是否启用同步 |
| `interval_seconds` | u64 | 同步间隔，沿用最小 5 秒规则 |
| `supabase_url` | string | Supabase project URL |
| `service_role_key` | string | Supabase service role key，仅请求写入，不在响应中返回 |
| `service_role_key_configured` | bool | 响应字段，表示后端是否已有 key |

## 安全边界

| 规则 | 说明 |
| --- | --- |
| 不返回密钥 | API 响应不包含 `service_role_key` 明文 |
| 不记录密钥 | 日志和错误响应不输出 `service_role_key` |
| 启用校验 | `enabled=true` 时，保存后的 `supabase_url` 和 key 必须存在 |
| 覆盖语义 | 请求包含新 key 时覆盖旧 key；请求不包含 key 时保留旧 key |
| 测试连接 | 测试 API 不保存配置，只返回连通性结果 |

## 验收标准

- `GET /sync/config` 返回 `enabled`、`interval_seconds`、`supabase_url` 和 `service_role_key_configured`，不返回 key 明文。
- `PUT /sync/config` 能写回 `~/.zembra.env`。
- `PUT /sync/config` 在请求不包含新 key 时保留已保存 key。
- `PUT /sync/config` 在请求包含新 key 时覆盖已保存 key。
- 保存后的配置立即影响手动同步 API。
- `POST /sync/config/test` 能测试 Supabase REST 连接，并且不持久化请求配置。
- API 错误响应和日志不泄露 key。
- OpenAPI JSON 包含新增配置 API path。
- Rust 格式化、编译和测试验证通过。

## 待设计决策

| 决策项 | 设计阶段处理 |
| --- | --- |
| `.zembra.env` 写回方式 | 确定保留既有配置段和注释的策略，避免误删其他配置 |
| 运行时配置热更新 | 确定 `SyncService` 如何替换当前 `SupabaseClient` 和 enabled 状态 |
| 后台 worker 热更新 | 明确保存配置后后台 worker 是否立即读取新配置，或仅手动同步立即生效 |
| 测试连接实现 | 确定使用轻量 REST 请求还是复用同步表查询能力 |
| DTO 设计 | 明确 request/response 字段、可选 key 的覆盖语义和错误码 |
