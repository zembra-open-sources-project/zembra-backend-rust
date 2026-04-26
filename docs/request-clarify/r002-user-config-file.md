# r002 用户配置文件读取

日期：2026-04-26

## 需求背景

后端服务需要支持从用户主目录读取本地配置文件，便于在不同机器上使用独立的 SQLite 数据库路径。

## 已确认需求

| 项目 | 结论 |
| --- | --- |
| 用户配置文件路径 | `~/.zembra.env` |
| 文件格式 | TOML |
| 数据库主配置字段 | `database.path` |
| 运行时连接信息 | 加载后自动转换为 SQLx SQLite URL |
| 配置来源 | `config/default.toml` 与 `~/.zembra.env` |
| 配置优先级 | `config/default.toml` < `~/.zembra.env` |
| 环境变量 | 不引入环境变量配置来源 |
| 字段对齐 | `config/default.toml` 与 `~/.zembra.env` 使用一致字段结构 |
| 用户配置文件不存在 | 输出 warning，并继续使用其他配置来源 |
| 用户配置文件存在但无效 | 按配置错误处理，启动失败 |

## 示例配置

```toml
[database]
path = "/path/to/zembra.sqlite3"
```

## 验收标准

- 默认配置文件使用 `database.path` 字段。
- `~/.zembra.env` 存在时可覆盖默认数据库路径。
- `~/.zembra.env` 不存在时服务输出 warning 并继续读取默认配置。
- 环境变量不再参与业务配置读取。
- 运行时仍能得到 SQLx 可用的 SQLite 连接 URL。
