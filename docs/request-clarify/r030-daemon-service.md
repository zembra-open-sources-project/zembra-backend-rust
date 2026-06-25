# r030-daemon-service

日期：2026-06-25

## 需求背景

当前 `zembra-backend-rust` 已经具备前台常驻 HTTP server 能力，发布文档覆盖二进制下载、配置 `~/.zembra.env`、手工启动和 `/health` 验证。为了让个人机器上的后端服务长期稳定运行，需要把启动方式从手工前台运行升级为当前用户的系统服务管理器托管，同时保持配置、数据和日志仍归当前用户所有。

## 需求目标

本次需求目标是增加用户级 daemon/service 初始化能力，让 macOS 用户可以通过用户级 Homebrew service 管理服务，让 Ubuntu 用户可以通过 `systemd --user` 管理服务。服务运行身份始终是当前用户，不创建系统用户，不要求 root，不接管系统级 service 生命周期。

## 已确认决策

| 决策项 | 结论 |
| --- | --- |
| macOS 服务形态 | 用户级 Homebrew service |
| Ubuntu 服务形态 | `systemd --user` user service |
| 未登录自动启动 | 不做，不自动执行 `loginctl enable-linger` |
| 运行用户 | 当前用户 |
| 配置文件 | 继续使用 `~/.zembra.env` |
| Linux 数据目录 | 使用 XDG 用户目录：`~/.local/share/zembra` |
| Linux 日志目录 | 使用 XDG 用户目录：`~/.local/state/zembra/logs` |
| Linux unit 位置 | `~/.config/systemd/user/zembra-backend.service` |
| 安装入口 | 做成 CLI 子命令 `zembra-backend init service`，不做独立安装脚本 |
| root 权限 | 不需要、不使用 |
| 创建系统用户 | 不做 |
| 已有配置处理 | 默认不覆盖已存在文件，只有显式 `--force` 才允许重写可生成文件 |
| 启动服务 | 默认只初始化；显式 `--start` 才执行 enable/start |
| macOS CLI 行为 | `zembra-backend init service` 只初始化 `~/.zembra.env` 和用户目录，不直接调用 `brew services start`，Homebrew service 生命周期由 Homebrew formula 管理 |

## 范围边界

### In Scope

| 项目 | 说明 |
| --- | --- |
| CLI 命令 | 新增 `zembra-backend init service` |
| 默认启动行为 | 保留无参数运行二进制即启动 HTTP server 的现有行为 |
| Ubuntu user service 初始化 | 生成或更新用户级 systemd unit，创建 XDG 数据和日志目录 |
| macOS service 初始化 | 创建当前用户所需配置和目录，服务启动交给 Homebrew service |
| 配置初始化 | 如果 `~/.zembra.env` 不存在，可从模板生成 daemon 友好的默认配置 |
| 启动选项 | 支持 `--start` 在 Ubuntu 上执行 `systemctl --user daemon-reload`、`enable`、`start` 或等价流程 |
| 覆盖选项 | 支持 `--force` 覆盖 CLI 可生成的 service 文件或配置模板 |
| Graceful shutdown | 服务进程应响应 `SIGTERM` 和 `Ctrl-C`，让服务管理器停止进程时能干净退出 |
| 文档 | 更新发布或安装文档，说明 macOS Homebrew service 与 Ubuntu user service 的初始化、启停、状态查看和健康检查 |
| 验收 | 服务启动后 `/health` 返回 `200 OK`，服务管理器状态为 running |

### Out of Scope

| 项目 | 说明 |
| --- | --- |
| system-wide systemd service | 不写入 `/etc/systemd/system` |
| 创建 `zembra` 系统用户 | 不创建任何新系统用户 |
| root 安装流程 | 不作为本轮默认能力 |
| `/etc/zembra`、`/var/lib/zembra`、`/var/log/zembra` | 不作为本轮 daemon 默认路径 |
| 未登录自动启动 | 不启用 linger，不保证用户未登录时自动运行 |
| Docker/GHCR | 不纳入本轮 |
| 数据库 schema 变化 | 不纳入本轮 |
| Homebrew formula 发布自动化 | 本轮只定义服务管理需求，不强制完成 tap 或 formula 发布流水线 |

## 验收标准

| 编号 | 标准 |
| --- | --- |
| A1 | 无参数执行 `zembra-backend` 仍按现有行为启动 HTTP server |
| A2 | 执行 `zembra-backend init service` 后，Ubuntu 当前用户下生成 `~/.config/systemd/user/zembra-backend.service`，并创建 XDG 数据和日志目录 |
| A3 | `~/.zembra.env` 不存在时，初始化命令能生成使用 XDG 路径的配置；文件已存在时默认不覆盖 |
| A4 | 执行带 `--start` 的初始化命令后，Ubuntu 可通过 `systemctl --user status zembra-backend` 看到服务运行 |
| A5 | macOS 初始化命令不会直接调用 `brew services start`，文档引导用户通过 Homebrew 管理服务 |
| A6 | 服务由 `systemctl --user stop zembra-backend` 或 Homebrew service 停止时，进程能响应终止信号并退出 |
| A7 | 服务启动后请求 `GET /health` 返回 `200 OK` |

