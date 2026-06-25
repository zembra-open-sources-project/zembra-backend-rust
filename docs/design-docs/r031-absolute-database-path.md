# r031 绝对数据库路径约束设计文档

日期：2026-06-25

需求澄清文档：`docs/request-clarify/r031-absolute-database-path.md`

本次设计在配置层阻止相对 SQLite 路径进入运行时。`Settings::load()` 反序列化配置后统一调用配置校验，`DatabaseSettings::validate()` 使用 `Path::is_absolute()` 拒绝非绝对路径并返回明确错误。`DatabaseSettings::sqlite_url()` 继续只负责把已校验路径转换为 SQLx URL，不承担路径推断或工作目录解析。

`zembra-backend config init` 继续写入 `~/.zembra.env`，但数据库路径从固定字符串 `data/zembra.db` 改为基于当前用户 home 的绝对路径。r032 后默认路径统一为 `~/.zembra/zembra.sqlite3`。`config/default.toml` 保持可反序列化和可运行，但不再使用相对路径；文档示例统一展示绝对路径。

| 位置 | 决策 |
| --- | --- |
| 配置校验 | `database.path` 必须是绝对路径 |
| 初始化模板 | 基于 `UserConfigInit.home_dir` 生成绝对 SQLite 路径 |
| SQLx URL 转换 | 保持直接转换，不做相对路径补全 |
| 同步冲突逻辑 | 不改，继续在无法通过 `sync_changes.created_at` 判断时停止 |
