# r034 workspace name 返回与交互初始化执行计划

需求澄清文档：`docs/request-clarify/r034-workspace-name.md`

设计文档：`docs/design-docs/r034-workspace-name.md`

## Stage #1: workspace name API

### 任务 #1: 返回 workspace_name

**Status:** Done

**Files:** Modify `src/repositories/workspaces.rs`, `src/dto/workspaces.rs`, `src/handlers/workspaces.rs`, `tests/workspaces_routes.rs`, `tests/openapi_routes.rs`

功能 / 实现说明 / 预期验证结果：`GET /workspaces` 响应项新增 `workspace_name`，允许 `null`；OpenAPI `WorkspaceSummary` schema 暴露该字段；路由测试验证数据库字段被原样返回。

## Stage #2: init 交互输入

### 任务 #1: workspace name 校验

**Status:** Done

**Files:** Modify `src/repositories/workspaces.rs`, tests

功能 / 实现说明 / 预期验证结果：新增 workspace name 校验函数，拒绝空字符串、全空白和任何包含 whitespace 的 name；合法 name 使用 trim 后结果。

### 任务 #2: init 输入抽象与失败规则

**Status:** Done

**Files:** Modify `src/init.rs`, `src/main.rs`, `tests/init_tests.rs`

功能 / 实现说明 / 预期验证结果：`zembra-backend init` 在创建数据库前最多交互询问 3 次 workspace name；合法输入写入 `workspaces.workspace_name`；3 次非法输入后初始化失败退出，不使用 fallback；已有数据库和配置都存在时不询问。

## Stage #3: 验证与提交

### 任务 #1: 全量验证与进度记录

**Status:** Done

**Files:** Modify `docs/exec-plans/active/r034-workspace-name.md`, `docs/PROGRESS.md`

功能 / 实现说明 / 预期验证结果：运行 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`，更新执行计划和进度记录后提交并推送。

验证记录：2026-06-25 已通过 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`。
