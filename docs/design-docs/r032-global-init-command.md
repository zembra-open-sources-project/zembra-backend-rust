# r032 全局初始化命令设计文档

日期：2026-06-25

需求澄清文档：`docs/request-clarify/r032-global-init-command.md`

## 核心功能（WHAT）

新增 `zembra-backend init` 作为首次使用入口，完成默认 SQLite 数据库创建和用户配置生成。

### 需求背景（WHY）

只有 `config init` 会留下“配置已存在但数据库尚未创建”的状态，数据库创建被推迟到 server 启动时才发生。同步开启后，这类初始化不完整状态会放大成误连空库或无法判断同步新旧的问题。全局初始化需要把默认数据库和配置文件一次准备好。

### 需求目标（GOAL）

`zembra-backend init` 使用 `~/.zembra/zembra.sqlite3` 作为默认数据库路径，先创建并迁移数据库，再生成 `~/.zembra.env` 指向该数据库。数据库和配置文件都存在时提示并跳过。

### 范围边界

纳入范围：新增 CLI action、全局 init 模块、默认数据库创建、配置文件生成、跳过逻辑、README 和 release 文档更新。非范围：不新增 `init --force`，不改变 `init service` 的 service 管理语义，不清空或重建已有数据库。

## 实现流程（HOW）

新增 `src/init.rs`，定义 `GlobalInitConfig`、`GlobalInit` 和 `init_global()`。`init_global()` 先计算 `~/.zembra/zembra.sqlite3` 和 `~/.zembra.env`，两者都存在时返回 `Skipped`；否则调用 `Database::connect()` 创建并迁移默认数据库，再调用配置初始化能力生成 env。`src/cli.rs` 解析单独的 `init` 命令，`src/main.rs` 根据结果打印初始化或跳过提示。

## 测试用例

| 场景 | 预期 |
| --- | --- |
| `parse_cli_args(["zembra-backend", "init"])` | 返回 `CliAction::Init` |
| 默认数据库和 env 都不存在 | 创建 `~/.zembra/zembra.sqlite3` 和 `~/.zembra.env` |
| 默认数据库和 env 都存在 | 返回 `Skipped`，不覆盖文件 |
| `config init` | 继续生成指向 `~/.zembra/zembra.sqlite3` 的 env |
