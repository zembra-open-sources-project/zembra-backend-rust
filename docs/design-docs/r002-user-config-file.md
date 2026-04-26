# r002 用户配置文件读取设计

日期：2026-04-26

需求澄清文档：`docs/request-clarify/r002-user-config-file.md`

## 设计目标

支持从 `~/.zembra.env` 读取 TOML 用户配置，并将数据库配置字段统一为 `database.path`。配置加载后由代码负责把文件路径转换为 SQLx SQLite URL，避免配置文件暴露底层连接字符串格式。

## 配置模型

| 配置段 | 字段 | 类型 | 说明 |
| --- | --- | --- | --- |
| `server` | `host` | `[u8; 4]` | HTTP 服务绑定 IPv4 地址 |
| `server` | `port` | `u16` | HTTP 服务绑定端口 |
| `database` | `path` | `String` | SQLite 数据库文件路径 |

## 加载顺序

| 顺序 | 来源 | 必需 | 行为 |
| --- | --- | --- | --- |
| 1 | `config/default.toml` | 是 | 提供完整默认配置 |
| 2 | `~/.zembra.env` | 否 | 存在时覆盖默认配置；不存在时记录 warning |

环境变量不作为配置来源。

## 转换策略

| 输入 | 输出 |
| --- | --- |
| `database.path = "data/zembra.db"` | `sqlite://data/zembra.db` |
| `database.path = "/path/to/zembra.sqlite3"` | `sqlite:///path/to/zembra.sqlite3` |

转换逻辑作为 `DatabaseSettings` 的方法提供，调用方不直接拼接连接字符串。

## 预期改动范围

| 文件 | 改动 |
| --- | --- |
| `src/config.rs` | 调整配置字段、加载来源、用户配置缺失 warning、SQLite URL 转换方法 |
| `src/main.rs` | 改为通过配置方法获取数据库 URL |
| `config/default.toml` | 将 `database.url` 改为 `database.path` |
| `docs/request-clarify/r002-user-config-file.md` | 记录需求澄清结果 |
| `docs/exec-plans/active/r002-user-config-file.md` | 记录执行计划与进度 |

## 验证方式

- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `cargo clippy`
