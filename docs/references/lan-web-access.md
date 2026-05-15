# 局域网网页访问部署说明

日期：2026-05-15

## 适用范围

本文说明如何把 Zembra 后端部署在家庭局域网内的一台普通 Linux 主机上，并让局域网内其他机器上的网页前端访问后端 API。

本文不覆盖 Tailscale、公网端口转发、HTTPS、反向代理、账号认证或 NAS 厂商专属配置。

## 后端配置

后端继续读取用户主目录下的 `.zembra.env`。局域网部署时，建议写成：

```toml
[server]
host = "0.0.0.0"
port = 3000
cors_allowed_origins = ["http://192.168.1.20:5173"]

[database]
path = "/home/zembra/data/zembra.db"

[logging]
level = "INFO"
path = "/home/zembra/logs"

[sync]
enabled = false
interval_seconds = 60
supabase_url = ""
service_role_key = ""
```

字段说明：

| 字段 | 说明 |
| --- | --- |
| `server.host` | `"0.0.0.0"` 表示监听所有 IPv4 网卡，局域网其他机器可以访问 |
| `server.port` | 后端 API 端口 |
| `server.cors_allowed_origins` | 允许访问后端的网页前端 origin，必须包含协议、IP 或域名、端口 |
| `database.path` | SQLite 数据库文件路径，建议使用主机上的持久化目录 |
| `logging.path` | 日志目录，建议使用主机上的持久化目录 |

如果前端页面运行在 `http://192.168.1.20:5173`，就把这个完整 origin 加入 `cors_allowed_origins`。如果前端换了机器或端口，需要同步更新这个字段。

## 启动前检查

确认后端主机能解析配置并访问数据库路径：

```bash
cargo check
```

确认系统防火墙允许局域网访问后端端口。以端口 `3000` 为例，具体命令取决于 Linux 发行版和防火墙工具。

## 访问验证

假设后端主机的局域网 IP 是 `192.168.1.10`，启动后在其他机器访问：

```text
http://192.168.1.10:3000/health
```

预期返回 `200 OK`，响应体中包含：

```json
{
  "status": "ok",
  "service": "zembra-server"
}
```

API 文档入口：

```text
http://192.168.1.10:3000/swagger-ui
```

机器可读 OpenAPI：

```text
http://192.168.1.10:3000/api-docs/openapi.json
```

## 前端访问检查

网页前端的 API base URL 应指向后端主机局域网地址，例如：

```text
http://192.168.1.10:3000
```

浏览器请求被 CORS 拦截时，优先检查：

| 检查项 | 期望 |
| --- | --- |
| 前端 origin | 与浏览器地址栏中的协议、主机、端口完全一致 |
| `cors_allowed_origins` | 包含前端 origin |
| 后端配置 | 修改 `.zembra.env` 后已重启后端 |
| 网络连通 | 其他机器可以打开 `/health` |

## 安全边界

本配置会让局域网内能连到后端端口的设备访问 API。本轮需求不包含认证；完整认证会在后续需求中单独设计。
