# r023 Supabase 新版 Secret Key 接入开发计划

日期：2026-05-28

需求澄清文档：`docs/request-clarify/r023-supabase-secret-key-api.md`

设计文档：`docs/design-docs/r023-supabase-secret-key-api.md`

## Stage #1: 配置模型与 Supabase Client

### Task #1: 更新同步配置字段

**状态：** Finished

**Files:** Modify `src/config.rs`, `.env.example`

**Function:** 将同步密钥字段从 `service_role_key` 改为 `secret_key`，并只接受 `sb_secret_...`。

**Implementation Notes:** `SyncSettings` 字段改名；`validate()` 在 `enabled=true` 时校验 URL 非空、secret key 非空且以 `sb_secret_` 开头；相关 config 单测同步改名并覆盖旧 key 被拒绝。

**Expected Verification Result:** `cargo test config` 中同步配置相关测试通过，旧 legacy key 字符串无法通过验证。

**完成时间：** 2026-05-28

### Task #2: 更新 Supabase REST client

**状态：** Finished

**Files:** Modify `src/sync/supabase.rs`

**Function:** 让 REST client 使用新版 secret key 命名和认证语义。

**Implementation Notes:** 字段和构造参数改为 `secret_key`；错误枚举改为 `InvalidSecretKey`；header 构造使用 `apikey` 和 `Authorization: Bearer <secret_key>`，但注释和校验不再描述为 service role JWT。

**Expected Verification Result:** Supabase client 请求构建测试通过，请求头包含 `apikey` 和同值 Bearer。

**完成时间：** 2026-05-28

## Stage #2: API、文档与回归验证

### Task #3: 更新 Sync Config API

**状态：** Finished

**Files:** Modify `src/dto/sync.rs`, `src/services/sync_config.rs`, `tests/sync_config_routes.rs`

**Function:** 配置 API 只暴露和接收新版 `secret_key` 字段。

**Implementation Notes:** 响应字段改为 `secret_key_configured`；PUT 和 test request 改为 `secret_key`；保存配置时请求不带 key 则保留已保存的 `secret_key`；旧字段不作为兼容入口。

**Expected Verification Result:** route 测试覆盖脱敏响应、保存 `secret_key`、启用但缺少 secret key 被拒绝。

**完成时间：** 2026-05-28

### Task #4: 更新使用文档并验证

**状态：** Finished

**Files:** Modify `README.md`, `docs/release.md`, `docs/references/lan-web-access.md`, `docs/exec-plans/active/r023-supabase-secret-key-api.md`

**Function:** 文档统一指导用户从 Supabase `Settings > API Keys` 使用新版 secret key。

**Implementation Notes:** 删除旧 service role 指引；执行 `rg` 确认用户面对的配置文档不再残留旧接入口径；运行格式、测试、构建和 clippy 验证。

**Expected Verification Result:** `rg "service_role_key|service role"` 只在历史需求/设计记录或明确说明旧版被拒绝的位置出现；`cargo fmt --check`、`cargo test`、`cargo check`、`cargo clippy -- -D warnings` 通过。

**完成时间：** 2026-05-28

## 执行记录

- 2026-05-28：完成 Supabase 同步配置从 `service_role_key` 到 `secret_key` 的切换，运行时只接受 `sb_secret_...`。
- 2026-05-28：更新 sync config API、Supabase REST client、配置样例和用户文档，旧版 legacy service role key 已由校验拒绝。
- 2026-05-28：已通过 `cargo fmt --check`、`cargo test config`、`cargo test`、`cargo check` 和 `cargo clippy -- -D warnings`。
