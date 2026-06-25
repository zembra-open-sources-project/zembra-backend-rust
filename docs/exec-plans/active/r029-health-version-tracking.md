# r029-health-version-tracking

## 关联设计文档

- 需求澄清文档：`docs/request-clarify/r029-health-version-tracking.md`
- 设计文档：`docs/design-docs/r029-health-version-tracking.md`

## Stage #1: Health 版本追踪

### 任务 #1: 增加版本追踪元数据

**Status:** Finished

**Files:** Modify `Cargo.toml`

功能：在本仓库 TOML 中声明版本追踪策略元数据。

实现说明：保留 `package.version` 作为语义版本号唯一来源，新增 `package.metadata.zembra.version_policy` 和 `package.metadata.zembra.release_channel`。

预期验证结果：代码可以从仓库 TOML 读取版本策略和发布通道，不依赖 `.zembra.env` 或运行时外部环境变量。

### 任务 #2: 先写 health 响应版本字段测试

**Status:** Finished

**Files:** Modify `tests/health_routes.rs`

功能：用集成测试定义 `/health` 版本追踪响应合同。

实现说明：断言 `/health` 返回 `version`、`version_policy`、`release_channel`，其中 `version` 等于 Cargo package version，策略字段等于仓库 TOML 配置。

预期验证结果：测试在实现前因缺少字段失败。

### 任务 #3: 实现版本读取并扩展 `/health`

**Status:** Finished

**Files:** Create `src/version.rs`; Modify `src/lib.rs`, `src/handlers/health.rs`

功能：新增版本信息读取模块，并把版本追踪字段加入 `HealthResponse`。

实现说明：`version` 使用 Cargo 编译期包版本常量，版本策略字段从仓库 `Cargo.toml package.metadata.zembra` 读取。`HealthResponse` 保持 `ToSchema`，使 OpenAPI 动态暴露新增字段。

预期验证结果：health 响应测试通过，OpenAPI schema 自动包含新增字段。

### 任务 #4: 整体验证和状态回写

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r029-health-version-tracking.md`

功能：运行格式化、编译、测试和 clippy，并记录结果。

实现说明：执行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`。完成后更新本执行计划任务状态和验证记录。

预期验证结果：全部验证通过；完成 Stage 后提交本次改动并尝试推送。

## 验证记录

- 2026-06-25：已通过 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`。首次 `cargo check` 和 `cargo clippy` 在沙箱内因 `utoipa-swagger-ui` build script 下载 Swagger UI 资产时无法解析 `github.com` 失败，提升权限重跑后通过。
