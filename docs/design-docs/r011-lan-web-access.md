# r011 局域网网页访问后端服务

日期：2026-05-15

需求澄清文档：`docs/request-clarify/r011-lan-web-access.md`

## 核心功能（WHAT）

让部署在家庭局域网 Linux 主机上的后端服务，可以被局域网内其他机器上的网页前端直接访问。后端需要支持标准 IP 字符串形式的监听地址配置，并为浏览器跨域请求提供可配置 CORS 能力。

### 需求背景（WHY）

当前后端默认监听本机地址，配置写法是 `server.host = [127, 0, 0, 1]`。这种数组形式不符合常见部署习惯，也不方便表达 `0.0.0.0`、固定局域网 IP 等运行场景。

前端网页访问后端 API 已经是当前实现的一部分。当前端网页运行在另一台机器或另一个端口时，浏览器会执行 CORS 校验；后端如果不显式允许对应 origin，即使网络可达，前端请求也会被浏览器拦截。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 标准 host 配置 | `server.host` 改为标准 IP 字符串，例如 `"127.0.0.1"` 和 `"0.0.0.0"` |
| 局域网监听 | 用户可通过 `.zembra.env` 配置服务监听所有网卡 |
| 前端跨域访问 | 后端支持网页前端从局域网其他机器发起 API 请求 |
| 默认开发安全 | 默认配置继续监听 `127.0.0.1`，避免无意暴露到局域网 |
| 部署说明 | 提供普通 Linux 主机上的配置、启动和访问验证说明 |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| 配置模型 | 将 `ServerSettings.host` 从 `[u8; 4]` 调整为可解析的标准 IP 字符串 |
| 默认配置 | `config/default.toml` 和 `.env.example` 使用字符串 host |
| CORS | 增加可配置 CORS origin，支持局域网前端网页访问 |
| Router | 在 Axum router 上挂载 CORS layer |
| 文档 | 增加局域网部署配置和访问验证说明 |
| 测试 | 覆盖 host 解析、非法 host 报错、CORS 预检和 OpenAPI 路由不回退 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| 认证授权 | 本次不实现 Bearer token、账号系统或设备授权 |
| 外网访问 | 不覆盖 Tailscale、公网端口转发、HTTPS、反向代理 |
| NAS 专属优化 | 不做 DSM/QNAP 等平台专属配置 |
| 前端改造 | 不修改前端工程，只保证后端具备浏览器访问条件 |
| IPv6 | 本次优先支持 IPv4 字符串；是否扩展 IPv6 后续单独确认 |

## 实现流程（HOW）

### 配置设计

推荐把 `server.host` 作为字符串读取，在启动阶段解析成 `std::net::Ipv4Addr`。配置文件保持直观写法：

| 场景 | 配置 |
| --- | --- |
| 本机开发 | `host = "127.0.0.1"` |
| 局域网部署 | `host = "0.0.0.0"` |
| 固定网卡监听 | `host = "192.168.1.10"` |

`ServerSettings` 字段设计：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `host` | `String` | 用户配置的 IPv4 字符串 |
| `port` | `u16` | HTTP 服务监听端口 |
| `cors_allowed_origins` | `Vec<String>` | 允许访问后端的浏览器 origin |

新增方法：

| 方法 | 说明 |
| --- | --- |
| `ServerSettings::host_addr()` | 将 `host` 解析成 `Ipv4Addr`，非法值返回配置错误 |
| `ServerSettings::cors_origins()` | 将配置字符串解析成 HTTP `HeaderValue` 或 `HeaderName` 所需类型，非法 origin 启动失败 |
| `ServerSettings::cors_origin_rules()` | 将精确 origin 和 IPv4 通配 origin 解析成运行时匹配规则，非法通配符启动失败 |

说明：先保持 IPv4，避免把监听地址、部署文档和测试范围扩成 IPv6。后续如果要支持 IPv6，可以把返回类型扩展为 `IpAddr`。

### 默认配置

`config/default.toml` 保持本机开发默认值：

```toml
[server]
host = "127.0.0.1"
port = 3000
cors_allowed_origins = []
```

`.env.example` 提供局域网部署示例：

```toml
[server]
host = "0.0.0.0"
port = 3000
cors_allowed_origins = ["http://192.168.1.20:5173"]
```

`cors_allowed_origins = []` 表示不额外允许跨域浏览器访问。这样默认本机开发行为最小，不会无意放开浏览器跨域访问。

### CORS 策略

推荐使用 `tower-http` 的 `CorsLayer`，只允许配置中的 origin。预期策略：

| 项目 | 策略 |
| --- | --- |
| Origin | 默认允许 localhost 和 loopback origin；`server.cors_allowed_origins` 用于局域网其他机器或自定义域名白名单 |
| Methods | 允许现有 API 需要的 `GET`、`POST`、`PUT`、`PATCH`、`DELETE`、`OPTIONS` |
| Headers | 允许 `content-type`；认证后续实现时再加入 `authorization` |
| Credentials | 本次不启用 credentials |

暂不采用 `Any` 全放开策略。默认只放行本机开发 origin，局域网其他机器和自定义域名通过配置化白名单补充。

### CORS IPv4 通配符

局域网前端设备 IP 变化时，允许在 IPv4 段中使用 `*` 通配符：

```toml
cors_allowed_origins = ["http://192.168.1.*:5173"]
```

通配符规则：

| 项目 | 规则 |
| --- | --- |
| 支持位置 | 仅支持 IPv4 host 的完整 octet，例如 `192.168.1.*` |
| 端口 | 必须精确匹配，例如 `:5173` |
| 协议 | 必须精确匹配 `http` 或 `https` |
| 域名 | 不支持域名通配符 |
| 禁止示例 | `http://*:5173`、`http://192.168.*.*:*`、`http://*.example.local:5173`、`*` |

### 启动流程

| 步骤 | 行为 |
| --- | --- |
| 1 | `Settings::load()` 读取 `config/default.toml` 和 `~/.zembra.env` |
| 2 | 校验 `server.host` 是否为合法 IPv4 字符串 |
| 3 | 校验 `server.cors_allowed_origins` 是否为合法 HTTP origin |
| 4 | 构建 Axum router，并根据配置挂载 CORS layer |
| 5 | 使用解析后的 `Ipv4Addr` 和 `port` 绑定 socket |
| 6 | 启动日志继续输出监听地址和数据库路径 |

### 文档策略

新增或更新部署说明，面向普通 Linux 主机，不做 NAS 专属路径假设。文档需要包含：

| 内容 | 示例 |
| --- | --- |
| `.zembra.env` 路径 | `~/.zembra.env` |
| 局域网监听 | `host = "0.0.0.0"` |
| 前端 origin | `cors_allowed_origins = ["http://<frontend-ip>:<frontend-port>"]` |
| 健康检查 | `http://<backend-ip>:3000/health` |
| API 文档 | `http://<backend-ip>:3000/swagger-ui` |

### 兼容性

本次不保留 `[127, 0, 0, 1]` 数组配置兼容。原因是当前需求明确要求改成标准 IP 字符串，且配置文件属于用户可控部署文件。已有 `.zembra.env` 如果仍使用数组，启动应失败并给出明确配置错误，用户按文档改为字符串即可。

## 测试用例

### 编译检查

| 用例 | 预期 |
| --- | --- |
| `cargo fmt --check` | 通过 |
| `cargo check` | 通过 |
| `cargo test` | 通过 |
| `cargo clippy -- -D warnings` | 通过 |

### 自动化测试

| 用例 | 预期 |
| --- | --- |
| 解析默认 host | `"127.0.0.1"` 可解析为 `Ipv4Addr` |
| 解析局域网 host | `"0.0.0.0"` 可解析为 `Ipv4Addr` |
| 拒绝非法 host | `"localhost"` 或 `"abc"` 返回配置错误 |
| 默认 CORS | `cors_allowed_origins = []` 时不添加跨域放行 |
| 允许配置 origin | 配置前端 origin 后，预检请求返回允许跨域的响应头 |
| OpenAPI 仍可访问 | `/api-docs/openapi.json` 和 `/swagger-ui` 不因 CORS layer 变更失效 |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| 本机开发启动 | 默认配置下 `http://127.0.0.1:3000/health` 返回 `200 OK` |
| 局域网后端访问 | Linux 主机配置 `host = "0.0.0.0"` 后，其他机器访问 `/health` 返回 `200 OK` |
| 局域网网页访问 | 前端 origin 加入白名单后，浏览器页面可调用后端 API |
| 非白名单网页访问 | 未加入白名单的 origin 被浏览器 CORS 拦截 |
