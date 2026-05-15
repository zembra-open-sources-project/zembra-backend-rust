# r012-github-release-pipeline

日期：2026-05-15

需求澄清文档：`docs/request-clarify/r012-github-release-pipeline.md`
设计文档：`docs/design-docs/r012-github-release-pipeline.md`

## Stage #1: CI 门禁

### Task #1: 新增 CI workflow

**Status:** Finished

**Files:** Create `.github/workflows/ci.yml`

**Function:** 为 PR 和 `master` push 建立 Rust 质量门禁。

**Implementation Notes:** 使用 `actions/checkout` 并开启 `submodules: recursive`；设置 stable Rust toolchain；缓存 Cargo registry、Cargo git 和 `target`；串行执行 `cargo fmt --check`、`cargo check --locked`、`cargo test --locked`、`cargo clippy --locked -- -D warnings`。

**Expected Verification Result:** GitHub Actions 在 PR 和 `master` push 上运行并通过四项 Rust 验证。

### Task #2: 本地核对 CI 命令

**Status:** Finished

**Files:** Verify local repository

**Function:** 在提交 CI workflow 前确认本地命令组合仍可通过。

**Implementation Notes:** 运行 `cargo fmt --check`、`cargo check --locked`、`cargo test --locked`、`cargo clippy --locked -- -D warnings`。

**Expected Verification Result:** 本地验证通过，失败时先修复导致 workflow 失败的问题。

**Verification Result:** 2026-05-15 已通过 `cargo fmt --check`、`cargo check --locked`、`cargo test --locked`（54 passed）和 `cargo clippy --locked -- -D warnings`。验证过程中发现测试临时 sync config 文件可能复用旧文件，已在 `src/app.rs` 的测试 state 初始化中清理对应临时文件。

## Stage #2: tag Release 流水线

### Task #1: 新增 Release workflow

**Status:** Designed

**Files:** Create `.github/workflows/release.yml`

**Function:** 推送 `v*` tag 后自动创建 GitHub Release。

**Implementation Notes:** workflow 使用 `contents: write` 权限；checkout 启用 `submodules: recursive`；从 tag 提取版本号并比对 `Cargo.toml package.version`；版本不一致时直接失败。

**Expected Verification Result:** `vX.Y.Z` tag 与 Cargo 版本一致时进入构建，不一致时 workflow 失败。

### Task #2: 构建并打包发布产物

**Status:** Designed

**Files:** Modify `.github/workflows/release.yml`

**Function:** 生成平台二进制 tar.gz 和 `SHA256SUMS`。

**Implementation Notes:** 发布包包含 `zembra-backend-rust`、`config/default.toml`、`.env.example`、`supabase/migrations/`、`LICENSE`；不包含 `data/`、`logs/`、`.zembra.env` 或本地数据库文件。

**Expected Verification Result:** GitHub Release assets 包含 tar.gz 和 `SHA256SUMS`，tar.gz 内容符合发布范围。

### Task #3: Release 前置验证

**Status:** Designed

**Files:** Modify `.github/workflows/release.yml`

**Function:** 防止未通过基础质量门禁的 tag 被发布。

**Implementation Notes:** Release workflow 在打包前运行 `cargo fmt --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings`。

**Expected Verification Result:** 任一验证失败时，Release 不创建或不上传产物。

## Stage #3: 发布文档

### Task #1: 新增发布使用文档

**Status:** Designed

**Files:** Create or Modify docs release guide

**Function:** 说明用户如何从 GitHub Release 下载并运行后端服务。

**Implementation Notes:** 文档覆盖下载 tar.gz、校验 `SHA256SUMS`、准备 `~/.zembra.env`、启动 `zembra-backend-rust`、访问 `/health` 和 `/api-docs/openapi.json`。

**Expected Verification Result:** 用户可以只根据文档完成下载、配置、启动和基础健康检查。

### Task #2: 记录 Docker/GHCR 后续范围

**Status:** Designed

**Files:** Modify docs release guide or `docs/exec-plans/tech-debt-tracker.md`

**Function:** 明确 Dockerfile、GHCR 镜像发布和自动版本工具不属于本轮范围。

**Implementation Notes:** 记录 Docker/GHCR 的后续前置条件，包括数据目录 volume、配置文件挂载、镜像 tag 策略和运行文档。

**Expected Verification Result:** 本轮实现不会静默扩展到 Docker 或 GHCR。

### Task #3: 整体验证和计划回写

**Status:** Designed

**Files:** Modify `docs/exec-plans/active/r012-github-release-pipeline.md`

**Function:** 完成所有 Stage 后更新任务状态和验证记录。

**Implementation Notes:** 记录本地验证命令结果；如已经推送测试 tag，则记录 GitHub Actions 和 Release assets 验证结果。每个 Stage 修改代码后按项目规则进行一次原子提交，commit message 必须满足 `^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert): .{10,}$`。

**Expected Verification Result:** 执行计划准确反映实现状态和验证结果，等待用户验收，不自动归档到 completed。
