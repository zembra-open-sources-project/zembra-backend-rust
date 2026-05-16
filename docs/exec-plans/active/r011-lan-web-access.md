# r011 局域网网页访问后端服务

日期：2026-05-15

需求澄清文档：`docs/request-clarify/r011-lan-web-access.md`

## Related Design Doc

`docs/design-docs/r011-lan-web-access.md`

## Stage #1: 配置模型与监听地址

### Task #1: 将 server.host 改为标准 IP 字符串

**Status:** Finished

**Files:** Modify `src/config.rs`, Modify `src/main.rs`, Modify `config/default.toml`, Modify `.env.example`

**Function:** 让用户在 `.zembra.env` 中使用 `server.host = "127.0.0.1"`、`"0.0.0.0"` 或固定局域网 IP 配置服务监听地址。

**Implementation Notes:** 将 `ServerSettings.host` 从 `[u8; 4]` 调整为 `String`。新增 `ServerSettings::host_addr()`，在启动绑定前解析为 `std::net::Ipv4Addr`，非法值返回明确配置错误。`config/default.toml` 保留 `"127.0.0.1"`，`.env.example` 给出局域网部署写法。所有新增函数补充文档字符串。

**Expected Verification Result:** 单元测试覆盖 `"127.0.0.1"`、`"0.0.0.0"` 和非法 host；默认配置可正常反序列化；启动绑定逻辑继续使用 `SocketAddr`。

### Task #2: 更新配置测试

**Status:** Finished

**Files:** Modify `src/config.rs`

**Function:** 保证配置读取测试与字符串 host 新格式一致。

**Implementation Notes:** 更新现有 `Settings` 反序列化测试中的 `[server].host` 示例。新增非法 host 测试，避免 `"localhost"`、空字符串或非 IP 文本被静默接受。

**Expected Verification Result:** `cargo test config` 或完整 `cargo test` 中配置相关测试通过。

## Stage #2: CORS 与网页前端访问

### Task #3: 增加 CORS 配置字段

**Status:** Finished

**Files:** Modify `src/config.rs`, Modify `config/default.toml`, Modify `.env.example`

**Function:** 支持通过 `server.cors_allowed_origins` 配置允许访问后端的网页前端 origin。

**Implementation Notes:** 在 `ServerSettings` 中新增 `cors_allowed_origins: Vec<String>`，默认值为空列表。新增 origin 解析或校验方法，非法 origin 在启动阶段返回配置错误。默认空列表表示不额外放开跨域浏览器访问。

**Expected Verification Result:** 默认配置中 `cors_allowed_origins = []` 可正常加载；配置合法 origin 时可解析；非法 origin 返回配置错误。

### Task #4: 在 Axum router 上挂载 CORS layer

**Status:** Finished

**Files:** Modify `Cargo.toml`, Modify `src/app.rs`, Modify `src/main.rs`

**Function:** 让浏览器前端可以从白名单 origin 访问后端 API。

**Implementation Notes:** 引入 `tower-http` 的 `CorsLayer`。调整 router 构建入口，使其接收 CORS 配置或预构建 layer。允许 `GET`、`POST`、`PUT`、`PATCH`、`DELETE`、`OPTIONS` 和 `content-type` header。本次不启用 credentials，不添加 `authorization` header。

**Expected Verification Result:** 配置白名单 origin 后，`OPTIONS` 预检请求返回允许跨域响应头；未配置或非白名单 origin 不返回放行头；既有 API 路由行为不回退。

### Task #5: 补充 CORS 路由测试

**Status:** Finished

**Files:** Modify `src/app.rs`

**Function:** 用自动化测试覆盖网页前端跨域访问路径。

**Implementation Notes:** 在 router 测试中构造带 CORS 配置的 app state 或 router 选项，发送带 `Origin`、`Access-Control-Request-Method` 的 `OPTIONS` 请求。测试允许 origin 与非允许 origin 的响应差异。

**Expected Verification Result:** CORS 预检测试通过；`/api-docs/openapi.json`、`/swagger-ui` 和 `/health` 的既有测试继续通过。

## Stage #3: 部署说明

### Task #6: 增加局域网部署说明

**Status:** Finished

**Files:** Create or Modify `docs/references/lan-web-access.md`, Modify `docs/design-docs/r011-lan-web-access.md` if design细节需要同步

**Function:** 给普通 Linux 主机部署提供清晰配置和验证路径。

**Implementation Notes:** 文档包含 `~/.zembra.env` 示例、`host = "0.0.0.0"`、`cors_allowed_origins` 示例、健康检查 URL、Swagger UI URL，以及防火墙端口提示。内容不写 NAS 平台专属步骤，不覆盖 Tailscale、HTTPS、反向代理或公网暴露。

**Expected Verification Result:** 用户可按文档配置后，在局域网其他机器访问 `http://<backend-ip>:3000/health`，前端 origin 加入白名单后浏览器请求不被 CORS 阻止。

## Stage #4: 整体验证与记录

### Task #7: 回归验证

**Status:** Finished

**Files:** Verify repository

**Function:** 确认局域网访问改动不破坏既有 API、OpenAPI 和同步配置能力。

**Implementation Notes:** 执行格式化、编译、测试和 clippy。必要时本地启动服务，使用默认配置验证 `127.0.0.1:3000/health`，再使用示例配置验证 CORS 预检请求。

**Expected Verification Result:** `cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings` 全部通过；OpenAPI JSON 和 Swagger UI 路由可访问。

### Task #8: 更新执行记录

**Status:** Finished

**Files:** Modify `docs/exec-plans/active/r011-lan-web-access.md`, Modify `docs/PROGRESS.md`

**Function:** 记录实现过程、验证结果和等待用户验收状态。

**Implementation Notes:** 每个 Stage 完成后更新任务状态和进度记录。完成每个 Stage 后，如果修改了代码，按项目规则进行一次 git 提交。未经用户验收不移动到 `docs/exec-plans/completed/`。

**Expected Verification Result:** 执行计划状态、进度记录和实际实现状态一致。

## 进度记录

- 2026-05-15：完成需求澄清，确认目标是家庭局域网内网页前端访问后端 API，不包含认证、Tailscale、公网暴露和 NAS 专属优化。
- 2026-05-15：完成设计文档，确认 `server.host` 改为标准 IP 字符串，并通过可配置 CORS origin 支持浏览器访问。
- 2026-05-15：完成开发计划，等待用户审核。
- 2026-05-15：开始 Stage #1，实现 `server.host` 标准 IP 字符串配置和对应测试。
- 2026-05-15：完成 Stage #1，`cargo fmt --check` 和 `cargo test config` 通过。
- 2026-05-15：开始 Stage #2，实现 `server.cors_allowed_origins` 配置、CORS layer 和预检测试。
- 2026-05-15：完成 Stage #2，`cargo fmt --check` 和 `cargo test` 通过，54 个测试通过。
- 2026-05-15：完成 Stage #3，新增 `docs/references/lan-web-access.md` 普通 Linux 主机局域网部署说明。
- 2026-05-15：完成 Stage #4，已通过 `cargo fmt --check`、`cargo check`、`cargo test` 和 `cargo clippy -- -D warnings`，其中 `cargo test` 为 54 passed。
- 2026-05-15：根据本地前端访问反馈修正 CORS 默认策略，空配置下默认只放行 localhost 和 loopback origin，局域网其他机器 origin 仍需显式配置。
- 2026-05-16：确认 CORS 通配符增量范围，只支持 IPv4 地址段通配，端口精确匹配，域名不支持通配符。
- 2026-05-16：完成 IPv4 通配符 CORS origin 实现，已通过 `cargo fmt --check`、`cargo check`、`cargo test` 和 `cargo clippy -- -D warnings`，其中 `cargo test` 为 67 passed。
