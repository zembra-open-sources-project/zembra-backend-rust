# r007 日志模块设计文档

日期：2026-05-04

需求澄清文档：`docs/request-clarify/r007-logging-module.md`

## 设计目标

- 日志初始化集中在独立模块中，避免启动入口承担过多细节。
- 配置继续沿用现有 `Settings` 和 `~/.zembra.env` TOML 文件。
- API 列表从运行时 OpenAPI 文档生成，避免手工维护两份路由清单。

## 配置设计

| 结构 | 字段 | 类型 | 默认值 | 用途 |
| --- | --- | --- | --- | --- |
| `Settings` | `logging` | `LoggingSettings` | 默认结构 | 聚合日志配置 |
| `LoggingSettings` | `level` | `String` | `INFO` | 日志显示级别 |
| `LoggingSettings` | `path` | `String` | `logs` | 日志保存目录 |

## 模块设计

| 模块 | 职责 |
| --- | --- |
| `src/config.rs` | 解析日志配置并提供默认值 |
| `src/logging.rs` | 初始化控制台和按天滚动的文件日志，生成启动摘要 |
| `src/main.rs` | 调用日志模块并在监听成功后打印启动信息 |

## 启动日志设计

- base url 由绑定地址生成，格式为 `http://{host}:{port}`。
- 数据库路径直接使用 `database.path`，便于用户对应配置文件。
- API 列表读取 `ApiDoc::openapi()` 的 `paths`，按 path 和 method 排序后逐条打印。
- 额外补充 `/api-docs/openapi.json` 和 `/swagger-ui` 两个文档入口。

## 预期改动范围

- 新增 `tracing-appender` 依赖用于日滚动文件日志。
- 新增 `src/logging.rs`。
- 更新 `src/config.rs`、`src/main.rs`、`.env.example`。
- 增加配置默认值和 API 列表单元测试。
