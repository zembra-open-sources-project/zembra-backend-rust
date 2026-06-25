# r031 绝对数据库路径约束执行计划

日期：2026-06-25

需求澄清文档：`docs/request-clarify/r031-absolute-database-path.md`

设计文档：`docs/design-docs/r031-absolute-database-path.md`

## Stage #1: 配置路径安全修复

### Task #1: 配置校验与初始化模板

**状态：** Finished

**文件：**
- 修改：`src/config.rs`
- 修改：`src/config_init.rs`
- 修改：`tests/config_init_tests.rs`

- 功能：拒绝相对 `database.path`，并让 `config init` 生成绝对 SQLite 数据库路径。
- 实现说明：先添加回归测试，再在 `Settings::load()` 后执行 `DatabaseSettings::validate()`；初始化模板使用 `UserConfigInit.home_dir` 拼出 `~/.local/share/zembra/zembra.db`。
- 预期验证结果：`cargo test config` 通过，并覆盖相对路径拒绝和初始化模板绝对路径。
- 完成时间：2026-06-25

### Task #2: 默认配置和文档示例

**状态：** Finished

**文件：**
- 修改：`config/default.toml`
- 修改：`.env.example`
- 修改：`README.md`
- 修改：`docs/release.md`
- 修改：`docs/design-docs/r002-user-config-file.md`

- 功能：移除当前运行文档和配置示例中的相对数据库路径。
- 实现说明：默认配置和示例全部使用绝对路径，README 明确 `database.path` 必须是绝对路径。
- 预期验证结果：`rg -n "data/zembra.db|data/custom-zembra" README.md docs config src tests .env.example` 不再在运行配置或用户文档中出现相对路径指导。
- 完成时间：2026-06-25
