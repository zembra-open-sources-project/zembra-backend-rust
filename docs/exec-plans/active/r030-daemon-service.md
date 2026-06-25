# r030-daemon-service

## 关联设计文档

- 需求澄清文档：`docs/request-clarify/r030-daemon-service.md`
- 设计文档：`docs/design-docs/r030-daemon-service.md`

## Stage #1: CLI 入口与服务启动边界

### 任务 #1: 引入 CLI 参数解析并保留无参数启动

**Status:** Finished

**Files:** Modify `Cargo.toml`, `src/main.rs`; Create `src/cli.rs`

功能：新增 `zembra-backend init service` 命令入口，同时保持无参数执行二进制时启动现有 HTTP server。

实现说明：使用标准库参数解析定义 `CliAction` 和 `ServiceInitOptions`，避免为当前窄命令面新增依赖。无参数时走现有 server 启动路径；`init service` 只进入初始化逻辑。命令名在 Cargo binary、文档和后续 unit 中统一为 `zembra-backend`。

预期验证结果：CLI 解析单元测试覆盖无参数、`init service`、`--start`、`--force`；直接运行无参数仍能进入 server 启动路径。

### 任务 #2: 抽出可复用 server 启动函数

**Status:** Finished

**Files:** Modify `src/main.rs`; Create or Modify `src/server.rs`, `src/lib.rs`

功能：把当前 `main()` 中的 HTTP server 启动逻辑拆到可复用函数，便于 CLI 分发和 graceful shutdown。

实现说明：保持配置加载、日志初始化、数据库连接、sync worker、router 构建和 listener 绑定顺序不变。新增函数需要有清晰 docstring，返回 `Result<(), error::AppError>`，避免改变已有 API 行为。

预期验证结果：`cargo check` 通过；现有 route 和 OpenAPI 测试不需要修改即可继续通过。

## Stage #2: 用户级 service 初始化

### 任务 #1: 实现用户路径解析与配置模板生成

**Status:** Finished

**Files:** Create `src/service_init.rs`; Modify `src/lib.rs`; Verify `tests/service_init_tests.rs`

功能：为 daemon 初始化生成当前用户路径和 `~/.zembra.env` 模板。

实现说明：Linux 使用 `${XDG_DATA_HOME:-~/.local/share}/zembra`、`${XDG_STATE_HOME:-~/.local/state}/zembra/logs`、`${XDG_CONFIG_HOME:-~/.config}/systemd/user/zembra-backend.service`。配置文件继续固定为 `~/.zembra.env`。如果配置文件已存在，默认不覆盖；`--force` 允许重写。测试中使用临时 HOME 和 XDG 环境，避免读写真实用户目录。

预期验证结果：单元测试验证默认 XDG 路径、自定义 XDG 路径、已有配置不覆盖、`--force` 覆盖。

### 任务 #2: 生成 systemd user unit

**Status:** Finished

**Files:** Modify `src/service_init.rs`; Verify `tests/service_init_tests.rs`

功能：在 Ubuntu/Linux 当前用户目录生成 `zembra-backend.service`。

实现说明：unit 写入 `${XDG_CONFIG_HOME:-~/.config}/systemd/user/zembra-backend.service`，`ExecStart` 使用当前可执行文件绝对路径，`WorkingDirectory` 使用 XDG 数据目录，`Restart=on-failure`，`WantedBy=default.target`。unit 禁止包含 `User=`、`/etc/systemd/system`、`/var/lib`、`/var/log`。

预期验证结果：测试断言 unit 路径、内容、覆盖策略和禁止字段。

### 任务 #3: 实现 `--start` 平台动作

**Status:** Finished

**Files:** Modify `src/service_init.rs`, `src/cli.rs`, `src/main.rs`; Verify `tests/service_init_tests.rs`

功能：`zembra-backend init service --start` 在 Linux 上初始化后启动 user service，macOS 上不调用 Homebrew。

实现说明：Linux 执行 `systemctl --user daemon-reload`、`systemctl --user enable zembra-backend`、`systemctl --user start zembra-backend`。命令执行封装为可注入 runner，测试使用 fake runner 断言命令序列。macOS 分支只输出下一步提示，不调用 `brew services start`。其他平台返回明确不支持错误。

预期验证结果：测试验证 Linux `--start` 命令顺序，macOS 不触发外部命令，错误路径有清晰返回。

## Stage #3: Graceful shutdown

### 任务 #1: 接入 Ctrl-C 和 SIGTERM graceful shutdown

**Status:** Finished

**Files:** Modify `src/server.rs` or `src/main.rs`; Verify focused server/shutdown tests where practical

功能：让前台进程、systemd user service 和 Homebrew service 停止时都能干净退出。

实现说明：使用 `axum::serve(listener, app).with_graceful_shutdown(shutdown_signal())`。`shutdown_signal()` 同时监听 `tokio::signal::ctrl_c()` 和 Unix `SIGTERM`，收到信号后记录日志。实现需要兼容 macOS 和 Linux；如果测试直接发送信号成本过高，至少用函数边界和编译检查覆盖。

预期验证结果：`cargo check` 和 `cargo test` 通过；手工停止 service 时进程退出且日志记录 shutdown。

## Stage #4: 文档、发布说明与整体验证

### 任务 #1: 更新安装和运行文档

**Status:** Finished

**Files:** Modify `README.md`, `docs/release.md`

功能：补充 macOS Homebrew service 和 Ubuntu `systemd --user` 的初始化、启动、停止、状态、日志和健康检查说明。

实现说明：README 面向日常使用，`docs/release.md` 面向发布包安装。文档必须明确不创建系统用户、不需要 root、不启用未登录自动启动。macOS 说明 `zembra-backend init service` 不直接调用 `brew services start`，服务生命周期由 Homebrew formula 管。

预期验证结果：文档命令与 CLI 设计一致，Markdown 段落不出现段内硬换行。

### 任务 #2: 执行回归验证并更新计划状态

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r030-daemon-service.md`; Verify repository checks

功能：完成整体格式化、编译、测试、clippy 和计划状态回写。

实现说明：执行 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`。如因依赖下载或 build script 网络访问失败，按仓库权限规则提升权限重跑必要命令。完成每个 Stage 后按项目规则提交并尝试推送。

预期验证结果：全部验证通过，执行计划记录验证结果，最终提交包含本需求相关改动。

## 验证记录

- 2026-06-25：已通过 `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`。新增 `tests/cli_tests.rs` 覆盖无参数启动、`init service`、`--start`、`--force` 和未知命令；新增 `tests/service_init_tests.rs` 覆盖 XDG 路径、配置保护、`--force` 覆盖、systemd user unit 内容、Linux `--start` 命令序列和 macOS 不调用 Homebrew。已用临时 HOME 执行 `target/debug/zembra-backend init service` 验证 macOS 分支会生成用户配置和目录且不触碰真实用户目录。
