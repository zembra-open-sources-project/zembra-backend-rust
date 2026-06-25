# r030-daemon-service

日期：2026-06-25

需求澄清文档：`docs/request-clarify/r030-daemon-service.md`

## 核心功能（WHAT）

为 `zembra-backend-rust` 增加用户级 daemon/service 初始化能力。安装后的用户命令命名为 `zembra-backend`，无参数时继续启动现有 HTTP server，新增 `zembra-backend init service` 用于初始化当前用户的服务运行环境。macOS 由 Homebrew formula 提供用户级 service 管理，Ubuntu 由 `systemd --user` 管理。

### 需求背景（WHY）

当前服务只能通过前台运行二进制启动，发布文档覆盖下载、配置和健康检查，但没有用户级服务托管路径。用户希望个人机器上的后端可以长期运行，同时不创建系统用户、不要求 root、不写系统级 unit、不接管未登录自动启动。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 用户级托管 | 服务始终以当前用户身份运行 |
| CLI 初始化 | 使用 `zembra-backend init service` 初始化服务环境 |
| macOS 兼容 Homebrew | macOS 初始化只准备配置和目录，服务生命周期由 `brew services` 管理 |
| Ubuntu 兼容 systemd user | Ubuntu 生成 `~/.config/systemd/user/zembra-backend.service` |
| 配置延续 | 继续使用 `~/.zembra.env` |
| 平台标准目录 | Ubuntu daemon 默认使用 XDG 用户目录 |
| 可停止 | 服务响应 `SIGTERM` 和 `Ctrl-C` 并干净退出 |
| 可验证 | 初始化、启动、停止和 `/health` 都有明确验证方式 |

### 范围边界

| 类型 | 内容 |
| --- | --- |
| In Scope | CLI 子命令、用户级 systemd unit 生成、XDG 目录创建、配置模板初始化、`--start` 和 `--force`、graceful shutdown、发布文档更新、定向测试 |
| Out of Scope | 创建系统用户、root 安装、system-wide systemd service、`/etc/zembra`、`/var/lib/zembra`、`/var/log/zembra`、未登录自动启动、Docker/GHCR、数据库 schema、Homebrew tap 发布自动化 |

## 实现流程（HOW）

### CLI 入口

当前 `src/main.rs` 无参数直接启动 HTTP server。新增轻量 CLI 解析层，保持无参数行为不变，并支持 `init service` 子命令。当前命令面很小，使用标准库参数解析即可满足需求，避免为一个窄命令面新增依赖；同时把启动 server 的逻辑拆成可测试函数。

| 命令 | 行为 |
| --- | --- |
| `zembra-backend` | 启动 HTTP server，保持现有行为 |
| `zembra-backend init service` | 初始化当前用户的 daemon/service 配置，不启动服务 |
| `zembra-backend init service --start` | 初始化后在 Ubuntu 上 reload、enable、start user service；macOS 不直接启动 Homebrew service |
| `zembra-backend init service --force` | 允许覆盖 CLI 生成的 service unit 和配置模板 |

安装命令名以 `zembra-backend` 为准。实现阶段需要在 Cargo binary 配置、发布文档和 service unit 中统一这个命令名，同时评估是否保留现有 `zembra-backend-rust` 二进制名作为 release 产物兼容入口。

### 平台行为

| 平台 | 初始化行为 | 启动行为 |
| --- | --- | --- |
| macOS | 创建当前用户配置和目录；不写 launchd plist；不调用 `brew services start` | 文档提示用户通过 `brew services start zembra-backend` 启动 |
| Ubuntu/Linux systemd | 创建 XDG 目录，写入 `~/.config/systemd/user/zembra-backend.service`，必要时初始化 `~/.zembra.env` | 只有 `--start` 时执行 `systemctl --user daemon-reload`、`enable`、`start` |
| 其他平台 | 返回明确错误，提示当前只支持 macOS 和 Linux user service 初始化 | 不启动 |

### 用户目录与配置

| 项目 | Linux 默认路径 |
| --- | --- |
| 配置文件 | `~/.zembra.env` |
| 数据目录 | `${XDG_DATA_HOME:-~/.local/share}/zembra` |
| 数据库文件 | `${XDG_DATA_HOME:-~/.local/share}/zembra/zembra.db` |
| 日志目录 | `${XDG_STATE_HOME:-~/.local/state}/zembra/logs` |
| systemd user unit | `${XDG_CONFIG_HOME:-~/.config}/systemd/user/zembra-backend.service` |

如果 `~/.zembra.env` 不存在，初始化命令生成 daemon 友好的 TOML 配置，`database.path` 指向 XDG 数据库文件，`logging.path` 指向 XDG 日志目录。如果文件已存在，默认不覆盖；`--force` 才允许重写。macOS 路径在设计上跟随 Homebrew formula 的 `var` 和日志约定，CLI 初始化只确保当前用户配置存在，不反向依赖 Homebrew 命令。

### systemd user unit

Ubuntu user unit 使用当前安装二进制的绝对路径作为 `ExecStart`，避免依赖非交互 shell 的 `PATH`。unit 只表达用户级运行，不声明 `User=`，不写系统目录，不创建或引用系统用户。

| 字段 | 设计 |
| --- | --- |
| `Description` | `Zembra backend service` |
| `ExecStart` | 当前 `zembra-backend` 可执行文件绝对路径 |
| `Restart` | `on-failure` |
| `WorkingDirectory` | XDG 数据目录 |
| `Environment` | 只在必要时设置 `RUST_LOG` 等非敏感运行参数；不把密钥写入 unit |
| `WantedBy` | `default.target` |

### Graceful shutdown

服务启动逻辑改为使用 `axum::serve(...).with_graceful_shutdown(...)`。shutdown signal 同时监听 `tokio::signal::ctrl_c()` 和 Unix `SIGTERM`。收到信号后记录日志并让 Axum 停止接收新连接，已有连接按 Axum/Tokio 默认行为收尾。

### 文档与发布

更新 `docs/release.md` 和 README 中的服务运行说明。macOS 文档说明 Homebrew 安装后通过 `brew services start/stop/restart zembra-backend` 管理；Ubuntu 文档说明 `zembra-backend init service`、`zembra-backend init service --start`、`systemctl --user status zembra-backend`、`journalctl --user -u zembra-backend` 和 `/health` 验证。

## 测试用例

### 编译检查

| 用例 | 预期结果 |
| --- | --- |
| `cargo fmt --check` | 格式检查通过 |
| `cargo check` | 编译检查通过 |
| `cargo test` | 单元和集成测试通过 |
| `cargo clippy -- -D warnings` | 无 warning |

### 自动化测试

| 用例 | 预期结果 |
| --- | --- |
| CLI 无参数解析 | 无参数仍进入 server 启动路径 |
| CLI `init service` 解析 | 正确识别 init service 子命令和默认选项 |
| CLI `--start`、`--force` 解析 | 正确设置初始化选项 |
| Linux 路径解析 | 未设置 XDG 环境变量时生成 `~/.local/share`、`~/.local/state`、`~/.config` 路径 |
| 已有配置保护 | `~/.zembra.env` 已存在时默认不覆盖 |
| `--force` 覆盖 | 可覆盖 CLI 生成的 unit 或配置模板 |
| systemd unit 内容 | unit 不包含 `User=`、`/etc/systemd/system`、`/var/lib`、`/var/log`，`ExecStart` 为绝对路径 |
| shutdown signal 构造 | server 启动函数使用 graceful shutdown 分支 |

### 手工检查

| 用例 | 预期结果 |
| --- | --- |
| Ubuntu 执行 `zembra-backend init service` | 生成 user unit、XDG 数据目录、XDG 日志目录和配置文件 |
| Ubuntu 执行 `zembra-backend init service --start` | `systemctl --user status zembra-backend` 显示 running |
| Ubuntu 查看日志 | `journalctl --user -u zembra-backend` 能看到启动日志 |
| macOS 初始化 | 不调用 `brew services start`，只完成配置和目录初始化 |
| Homebrew 启停 | `brew services start zembra-backend` 后 `/health` 返回 200 |

### 回归检查

| 用例 | 预期结果 |
| --- | --- |
| 前台启动 | 直接运行二进制仍可启动 HTTP server |
| 配置读取 | 现有 `~/.zembra.env` 读取语义不变 |
| OpenAPI | `/api-docs/openapi.json` 仍返回 200 |
| 健康检查 | `/health` 仍返回服务状态和版本信息 |
