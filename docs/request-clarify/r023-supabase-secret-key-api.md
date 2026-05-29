# r023 Supabase 新版 Secret Key 接入

日期：2026-05-28

## 需求理解

用户明确要求 Supabase 接入升级到新版 API key 体系，并且不允许继续使用旧版 `service_role` 接入。当前仓库的同步配置字段、API DTO、测试和文档仍使用 `service_role_key`，Supabase REST client 也把 key 放入 `Authorization: Bearer <key>`，这会让旧版 JWT 型 service role key 继续成为可用路径。

## 范围

| 项目 | 结论 |
| --- | --- |
| 配置字段 | 使用 `sync.secret_key` |
| API 请求字段 | 使用 `secret_key` |
| API 响应字段 | 使用 `secret_key_configured`，不返回密钥明文 |
| Supabase key 类型 | 只接受新版 `sb_secret_...` secret key |
| 旧版接入 | 不兼容 `service_role_key`，不接受 legacy service role key |
| Supabase Auth | 不引入用户登录或 Auth 流程 |
| 远端 schema | 不修改 `supabase/migrations/` |

## 验收标准

- `.zembra.env`、`.env.example` 和 README 不再指导用户配置 `service_role_key`。
- `sync.enabled=true` 时，`sync.secret_key` 为空或不是 `sb_secret_` 前缀会被拒绝。
- `PUT /sync/config` 和 `POST /sync/config/test` 只接受 `secret_key` 字段。
- `GET /sync/config` 返回 `secret_key_configured`，不返回 `secret_key` 明文。
- Supabase REST 请求使用新版 API key 语义，不再依赖 legacy JWT 型 service role key。
- 自动化测试覆盖配置保存、配置拒绝和脱敏响应。
