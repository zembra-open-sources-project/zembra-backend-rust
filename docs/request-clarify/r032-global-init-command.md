# r032 全局初始化命令需求澄清

日期：2026-06-25

当前仓库已有 `config init` 和 `init service`，但缺少面向首次使用的全局初始化命令。用户明确纠正初始化语义：全局初始化不能先依赖 env 决定数据库位置，而应该先按产品默认路径创建数据库，再生成指向该数据库的 env。

本次需求目标是新增 `zembra-backend init`。命令使用固定默认数据库路径 `~/.zembra/zembra.sqlite3`，先创建 SQLite 文件并执行现有 schema migration，再生成 `~/.zembra.env`，其中 `[database].path` 指向该默认数据库。如果默认数据库和配置文件都已经存在，命令提示用户并跳过初始化动作。本轮不需要 `zembra-backend init --force`。

验收标准：`zembra-backend init` 能创建默认数据库和配置文件；配置文件中的数据库路径为 `~/.zembra/zembra.sqlite3`；默认数据库和配置文件都存在时不覆盖任一文件并返回跳过结果；现有 `config init` 继续可用并使用相同默认数据库路径。
