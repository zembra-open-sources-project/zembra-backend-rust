# r023 Supabase 新版 Secret Key 接入

日期：2026-05-28

需求澄清文档：`docs/request-clarify/r023-supabase-secret-key-api.md`

## 核心设计

把 Supabase 同步配置从 legacy `service_role_key` 改为新版 `secret_key`。本地后端继续通过 Supabase REST/PostgREST 访问同步表，但认证材料只允许 `sb_secret_...`，避免应用继续依赖旧版 JWT 型 service role key。

## 改动范围

| 模块 | 改动 |
| --- | --- |
| `src/config.rs` | `SyncSettings.service_role_key` 改为 `secret_key`，校验 `sb_secret_` 前缀 |
| `src/dto/sync.rs` | 配置 API DTO 字段改为 `secret_key` 和 `secret_key_configured` |
| `src/services/sync_config.rs` | 保存、读取、测试连接逻辑改用 `secret_key` |
| `src/sync/supabase.rs` | client 字段和错误文案改用 secret key；请求头按新版 API key 语义构造 |
| `.env.example` / README / release docs | 删除旧 service role 配置说明，改为 secret key |
| 测试 | 更新字段名，并新增旧 key/旧字段拒绝覆盖 |

## Supabase REST Header 策略

新版 secret key 是 opaque key，不是 JWT。REST 请求保留 `apikey: <secret_key>` 作为主认证头，并把 `Authorization` 设置为同一个 key 的 Bearer 形式，让 Supabase API gateway 按新版 key 流程替换为内部角色凭证。代码侧不再把它描述或校验为 JWT。

## 兼容策略

本轮按用户要求不做 legacy 兼容：

| 旧输入 | 行为 |
| --- | --- |
| `sync.service_role_key` | 反序列化时被忽略；启用同步时因缺少 `secret_key` 失败 |
| API 请求 `service_role_key` | DTO 不接收该字段；启用同步时因缺少 `secret_key` 失败 |
| legacy JWT service role key | 因不满足 `sb_secret_` 前缀被拒绝 |

## 风险与验证

主要风险是字段改名导致已有配置不可用。该风险符合用户“不允许旧版接入”的要求。验证以配置单元测试、sync config 路由测试、Supabase client 请求头测试和全量 Rust 测试为主。
