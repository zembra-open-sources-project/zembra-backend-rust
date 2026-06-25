# r032 全局初始化命令执行计划

日期：2026-06-25

需求澄清文档：`docs/request-clarify/r032-global-init-command.md`

设计文档：`docs/design-docs/r032-global-init-command.md`

## Stage #1: 全局初始化入口

### Task #1: CLI 解析和初始化模块

**状态：** Finished

**文件：**
- 创建：`src/init.rs`
- 修改：`src/cli.rs`
- 修改：`src/main.rs`
- 修改：`src/lib.rs`
- 修改：`src/error.rs`
- 验证：`tests/cli_tests.rs`
- 验证：`tests/init_tests.rs`

- 功能：新增 `zembra-backend init`，按默认路径先创建数据库再生成 env，并在数据库和 env 都存在时跳过。
- 实现说明：使用 `Database::connect()` 复用现有 SQLite 创建和 migration；`GlobalInitConfig` 固定默认路径为 `~/.zembra/zembra.sqlite3`；不实现 `--force`。
- 预期验证结果：`cargo test init` 通过。
- 完成时间：2026-06-25

### Task #2: 默认配置路径和文档

**状态：** Finished

**文件：**
- 修改：`src/config_init.rs`
- 修改：`tests/config_init_tests.rs`
- 修改：`README.md`
- 修改：`docs/release.md`
- 修改：`docs/design-docs/r031-absolute-database-path.md`
- 修改：`docs/exec-plans/active/r031-absolute-database-path.md`

- 功能：让 `config init` 和用户文档使用同一个默认数据库路径。
- 实现说明：配置初始化模板写入 `~/.zembra/zembra.sqlite3`；文档把首次使用入口改为 `zembra-backend init`。
- 预期验证结果：`cargo test config`、`cargo test init` 和全量验证通过。
- 完成时间：2026-06-25
