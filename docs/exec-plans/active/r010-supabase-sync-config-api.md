# r010 Supabase 同步配置 API

日期：2026-05-10

需求澄清文档：`docs/request-clarify/r010-supabase-sync-config-api.md`

## Related Design Doc

`docs/design-docs/r010-supabase-sync-config-api.md`

## Stage #1: 配置持久化基础能力

### Task #1: 暴露用户配置文件路径与配置校验能力

**Status:** Finished

**Files:** Modify `src/config.rs`

**Function:** 让配置写回服务可以复用当前 `~/.zembra.env` 路径和 `SyncSettings` 校验逻辑。

**Implementation Notes:** 将内部 `user_config_path` 调整为可被服务层调用的函数，保持现有 `Settings::load()` 行为不变。复用 `SyncSettings::validate()` 校验 `interval_seconds`、URL 和 key。所有新增函数补充文档字符串。

**Expected Verification Result:** 配置单元测试继续通过；新增路径函数测试覆盖 HOME 存在和缺失场景。

### Task #2: 实现 sync 配置读写服务

**Status:** Finished

**Files:** Create `src/services/sync_config.rs`, Modify `src/services/mod.rs`, Modify `Cargo.toml` if needed

**Function:** 封装 `.zembra.env` 中 `[sync]` 配置的读取、合并、写回和响应脱敏。

**Implementation Notes:** 使用结构化 TOML 解析更新 `[sync]` 配置，保留非 sync 配置段。请求缺少 `service_role_key` 时保留旧 key，请求包含新 key 时覆盖旧 key。写回成功后返回不含 key 明文的配置摘要。

**Expected Verification Result:** 单元测试覆盖创建配置文件、保留旧 key、覆盖新 key、enabled=true 缺 key 报错、响应不包含 key。

## Stage #2: 运行时热更新

### Task #3: 调整 SyncService 使用可更新配置快照

**Status:** Finished

**Files:** Modify `src/services/sync.rs`, Modify `src/app.rs`, Modify `src/main.rs`

**Function:** 保存配置后，手动同步 API 立即使用新的 Supabase URL、key 和 enabled 状态。

**Implementation Notes:** 将 `SyncService` 调整为持有 `Arc<RwLock<SyncSettings>>` 或等价共享状态。`push`、`pull`、`run_once` 和 `status` 执行开始时 clone 当前配置快照，释放锁后执行数据库和网络操作。新增更新配置的方法供配置保存 API 调用。

**Expected Verification Result:** 测试覆盖默认 disabled 时 `/sync/run` 返回 disabled，运行时更新 enabled 后手动同步路径读取新配置。

### Task #4: 让后台 worker 读取最新配置

**Status:** Finished

**Files:** Modify `src/sync/worker.rs`

**Function:** 后台 worker 每轮执行和 sleep 间隔使用最新配置。

**Implementation Notes:** worker 继续复用同一个 `SyncService`。每轮读取当前 `enabled` 和 `interval_seconds`；disabled 时不执行网络同步，仅按当前间隔等待；enabled 时执行 `run_once`。避免启动时 disabled 导致 worker 永久不运行。

**Expected Verification Result:** 单元测试或轻量行为测试覆盖 worker 启动时 disabled 后续可启用的配置读取逻辑；至少保证编译和现有启动行为不回退。

## Stage #3: 配置 API 与 OpenAPI

### Task #5: 新增 sync config DTO

**Status:** Finished

**Files:** Modify `src/dto/sync.rs` or Create `src/dto/sync_config.rs`, Modify `src/dto/mod.rs` if needed

**Function:** 定义 `SyncConfigResponse`、`UpdateSyncConfigRequest`、`TestSyncConfigRequest` 和 `SyncConfigTestResponse`。

**Implementation Notes:** 所有公开 DTO 派生 `serde` 和 `utoipa::ToSchema`。响应 DTO 不包含 `service_role_key` 字段；请求 DTO 中 key 为可选字段。

**Expected Verification Result:** DTO schema 能注册到 OpenAPI；编译通过。

### Task #6: 新增 Supabase 连接测试能力

**Status:** Finished

**Files:** Modify `src/sync/supabase.rs`, Modify `src/services/sync_config.rs`

**Function:** 为 `POST /sync/config/test` 提供不持久化的 Supabase REST 连通性检查。

**Implementation Notes:** 使用候选 URL/key 构造临时 Supabase client，访问 `GET /rest/v1/sync_changes?limit=1` 或等价轻量请求。测试失败返回脱敏 message，不输出 key。

**Expected Verification Result:** 使用 mock HTTP 测试成功、失败和错误脱敏；测试请求不改变配置文件。

### Task #7: 新增配置 handler、route 和 OpenAPI 注册

**Status:** Finished

**Files:** Modify `src/handlers/sync.rs`, Modify `src/routes/sync.rs`, Modify `src/api_doc.rs`, Modify `src/app.rs`

**Function:** 暴露 `GET /sync/config`、`PUT /sync/config`、`POST /sync/config/test`。

**Implementation Notes:** handler 调用 `SyncConfigService` 完成读取、写入和测试连接。`PUT` 写回成功后调用 `SyncService` 更新运行时配置。新增 `#[utoipa::path]` 标注并注册 path 和 schema。

**Expected Verification Result:** route 测试覆盖 GET、PUT、POST test；OpenAPI JSON 包含三个新增 path。

### Task #8: 补齐 API 错误映射

**Status:** Finished

**Files:** Modify `src/error.rs`, Modify `src/dto/error.rs` if needed

**Function:** 为配置校验失败和配置文件读写失败提供稳定错误响应。

**Implementation Notes:** 配置校验失败返回 400，文件读写失败返回 500。错误 code 语义明确，例如 `invalid_config`、`config_io_failed`。错误 message 不包含 key。

**Expected Verification Result:** API 测试断言无效配置返回 400，响应体不包含提交的 key。

## Stage #4: 整体验证与记录

### Task #9: 回归验证

**Status:** Finished

**Files:** Verify repository

**Function:** 确认 Supabase 配置 API 不破坏既有后端行为。

**Implementation Notes:** 执行格式化、编译、测试、clippy，并检查 OpenAPI JSON 包含新增 path。必要时本地启动服务验证 `/health`、`/sync/config` 和 `/swagger-ui`。

**Expected Verification Result:** `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings` 全部通过；OpenAPI JSON 包含 `/sync/config` 和 `/sync/config/test`。

### Task #10: 更新执行记录

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r010-supabase-sync-config-api.md`, Modify `docs/PROGRESS.md`

**Function:** 记录实现过程、验证结果和等待验收状态。

**Implementation Notes:** 每个 Stage 完成后更新任务状态和进度记录。完成每个 Stage 后，如果修改了代码，按项目规则进行一次 git 提交。未经用户验收不移动到 `docs/exec-plans/completed/`。

**Expected Verification Result:** 执行计划状态、进度记录和实际实现状态一致。

## 进度记录

- 2026-05-10：完成需求澄清，确认配置写回 `.zembra.env`、key 不明文返回、保存后立即影响手动同步 API，并新增 `/sync/config/test`。
- 2026-05-10：完成设计文档和开发计划，等待用户审核。
- 2026-05-10：完成 Supabase 同步配置 API 开发，新增配置读写服务、运行时 sync 配置热更新、`GET /sync/config`、`PUT /sync/config`、`POST /sync/config/test` 和 OpenAPI 注册。
- 2026-05-10：完成整体验证，已通过 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`。
