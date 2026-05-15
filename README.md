# Zembra Backend Rust 使用手册

Zembra Backend Rust 是 Zembra 的本地后端服务。它在你的机器上运行，负责管理本地 SQLite 笔记数据库，并通过 HTTP API 提供笔记读写、标签管理、OpenAPI 文档和 Supabase 同步能力。

这个服务适合本机使用，也可以配置成局域网内其他设备可访问。

## 你可以用它做什么

| 功能 | 说明 |
| --- | --- |
| 创建笔记 | 写入单条或批量笔记 |
| 查看笔记 | 按更新时间列出笔记，或读取指定笔记 |
| 最近笔记 | 获取未删除、未归档的最近笔记列表 |
| 修改笔记 | 更新笔记内容，并保留 revision |
| 归档和删除 | 支持归档笔记和软删除笔记 |
| 管理标签 | 查看 fields、tags，给笔记添加或移除 tag |
| 本地存储 | 默认使用 `data/zembra.db` 保存数据 |
| 局域网访问 | 可配置监听 `0.0.0.0`，供同一局域网设备访问 |
| Supabase 同步 | 可配置远端 Supabase，同步本地变更 |
| API 文档 | 内置 Swagger UI 和 OpenAPI JSON |

## 安装

从 GitHub Release 页面下载适合你机器的压缩包，例如：

```text
zembra-backend-rust-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

解压：

```bash
tar -xzf zembra-backend-rust-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
cd zembra-backend-rust-v0.1.0-x86_64-unknown-linux-gnu
```

解压后主要文件如下：

| 路径 | 用途 |
| --- | --- |
| `zembra-backend-rust` | 后端服务程序 |
| `config/default.toml` | 默认配置 |
| `.env.example` | 用户配置示例 |
| `supabase/migrations/` | Supabase 远端数据库 migration |
| `LICENSE` | 许可证 |

## 配置

服务会读取两份配置：

| 配置文件 | 说明 |
| --- | --- |
| `config/default.toml` | 程序自带默认配置 |
| `~/.zembra.env` | 你的用户配置，存在时覆盖默认配置 |

第一次使用可以复制示例配置：

```bash
cp .env.example ~/.zembra.env
```

常用配置如下：

```toml
[server]
host = "127.0.0.1"
port = 3000
cors_allowed_origins = []

[database]
path = "data/zembra.db"

[logging]
level = "INFO"
path = "logs"

[sync]
enabled = false
interval_seconds = 60
supabase_url = ""
service_role_key = ""
```

### 本机使用

默认配置只允许本机访问：

```toml
[server]
host = "127.0.0.1"
port = 3000
```

服务地址是：

```text
http://127.0.0.1:3000
```

### 局域网使用

如果希望手机、平板或另一台电脑访问这台机器上的后端，把 `host` 改成：

```toml
[server]
host = "0.0.0.0"
port = 3000
cors_allowed_origins = ["http://192.168.1.20:5173"]
```

其中 `cors_allowed_origins` 填前端页面的地址。然后在其他设备上访问：

```text
http://<后端机器 IP>:3000/health
```

## 启动服务

进入解压目录后运行：

```bash
./zembra-backend-rust
```

启动时服务会自动：

| 步骤 | 说明 |
| --- | --- |
| 读取配置 | 加载 `config/default.toml` 和 `~/.zembra.env` |
| 初始化数据库 | 创建或打开 SQLite 数据库 |
| 执行迁移 | 自动补齐需要的表结构 |
| 启动 HTTP 服务 | 监听配置中的 host 和 port |
| 写入日志 | 默认写入 `logs/` 目录 |

健康检查：

```bash
curl http://127.0.0.1:3000/health
```

正常响应类似：

```json
{
  "status": "ok",
  "service": "zembra-server",
  "database_initialized": true
}
```

## API 文档

服务启动后可以打开 Swagger UI：

```text
http://127.0.0.1:3000/swagger-ui/
```

机器可读 OpenAPI JSON：

```text
http://127.0.0.1:3000/api-docs/openapi.json
```

如果你只是想调试接口，优先使用 Swagger UI。

## 笔记使用示例

### 创建一条笔记

```bash
curl -X POST http://127.0.0.1:3000/notes \
  -H 'content-type: application/json' \
  -d '{
    "content": "今天读完一篇关于本地优先软件的文章",
    "field": "reading",
    "tags": ["local-first", "notes"],
    "role": "Human",
    "device_id": null
  }'
```

字段说明：

| 字段 | 说明 |
| --- | --- |
| `content` | 笔记正文，不能为空 |
| `field` | 笔记所属 field，可为空 |
| `tags` | 标签列表，可为空 |
| `role` | 创建者角色，默认是 `Human` |
| `device_id` | 设备 ID，可为空 |

### 批量创建笔记

```bash
curl -X POST http://127.0.0.1:3000/notes/batch \
  -H 'content-type: application/json' \
  -d '{
    "items": [
      {
        "content": "第一条笔记",
        "field": "inbox",
        "tags": ["idea"],
        "role": "Human",
        "device_id": null
      },
      {
        "content": "第二条笔记",
        "field": "inbox",
        "tags": ["todo"],
        "role": "Human",
        "device_id": null
      }
    ]
  }'
```

### 查看笔记列表

```bash
curl 'http://127.0.0.1:3000/notes?limit=20'
```

### 查看最近笔记

```bash
curl -X POST http://127.0.0.1:3000/notes/recent \
  -H 'content-type: application/json' \
  -d '{
    "limit": 20
  }'
```

`/notes/recent` 只返回未删除、未归档的笔记。`limit` 取值范围是 1 到 100；不传时默认返回最近 50 条。

分页时可以传入完整 32 位 hex `note_uuid`：

```bash
curl -X POST http://127.0.0.1:3000/notes/recent \
  -H 'content-type: application/json' \
  -d '{
    "limit": 20,
    "note_uuid": "完整的32位note id"
  }'
```

### 查看指定笔记

`note_ref` 可以是完整 32 位 note ID，也可以是至少 4 位的唯一 hex 前缀。

```bash
curl http://127.0.0.1:3000/notes/<note_ref>
```

### 更新笔记

```bash
curl -X PATCH http://127.0.0.1:3000/notes/<note_ref> \
  -H 'content-type: application/json' \
  -d '{
    "content": "更新后的内容",
    "device_id": null
  }'
```

### 归档笔记

```bash
curl -X POST http://127.0.0.1:3000/notes/<note_ref>/archive
```

### 删除笔记

删除是软删除。

```bash
curl -X DELETE http://127.0.0.1:3000/notes/<note_ref>
```

### 查看笔记 revisions

```bash
curl http://127.0.0.1:3000/notes/<note_ref>/revisions
```

## Field 和 Tag

查看 fields：

```bash
curl 'http://127.0.0.1:3000/fields?limit=20'
```

查看 tags：

```bash
curl 'http://127.0.0.1:3000/tags?limit=20'
```

查看某条笔记的 tags：

```bash
curl http://127.0.0.1:3000/notes/<note_ref>/tags
```

给笔记添加 tag：

```bash
curl -X PUT http://127.0.0.1:3000/notes/<note_ref>/tags/<tag_name>
```

移除笔记 tag：

```bash
curl -X DELETE http://127.0.0.1:3000/notes/<note_ref>/tags/<tag_name>
```

## Supabase 同步

同步默认关闭：

```toml
[sync]
enabled = false
interval_seconds = 60
supabase_url = ""
service_role_key = ""
```

启用同步前，需要先在 Supabase 项目中执行 `supabase/migrations/` 下的 migration。

然后配置：

```toml
[sync]
enabled = true
interval_seconds = 60
supabase_url = "https://example.supabase.co"
service_role_key = "your-service-role-key"
```

`service_role_key` 只用于本地后端访问 Supabase REST API。接口响应不会返回 key 明文。

### 查看同步状态

```bash
curl http://127.0.0.1:3000/sync/status
```

### 读取同步配置

```bash
curl http://127.0.0.1:3000/sync/config
```

响应会包含：

| 字段 | 说明 |
| --- | --- |
| `enabled` | 同步是否启用 |
| `interval_seconds` | 后台同步间隔 |
| `supabase_url` | Supabase 项目地址 |
| `service_role_key_configured` | 是否已配置 service role key |

### 保存同步配置

```bash
curl -X PUT http://127.0.0.1:3000/sync/config \
  -H 'content-type: application/json' \
  -d '{
    "enabled": true,
    "interval_seconds": 60,
    "supabase_url": "https://example.supabase.co",
    "service_role_key": "your-service-role-key"
  }'
```

保存后配置会写回 `~/.zembra.env`。

### 测试 Supabase 连接

这个接口只测试连接，不保存请求中的配置。

```bash
curl -X POST http://127.0.0.1:3000/sync/config/test \
  -H 'content-type: application/json' \
  -d '{
    "supabase_url": "https://example.supabase.co",
    "service_role_key": "your-service-role-key"
  }'
```

### 手动同步

执行 push + pull：

```bash
curl -X POST http://127.0.0.1:3000/sync/run
```

只执行 push：

```bash
curl -X POST http://127.0.0.1:3000/sync/push
```

只执行 pull：

```bash
curl -X POST http://127.0.0.1:3000/sync/pull
```

## 日志和数据

| 内容 | 默认位置 |
| --- | --- |
| SQLite 数据库 | `data/zembra.db` |
| 日志目录 | `logs/` |
| 用户配置 | `~/.zembra.env` |

如果修改了 `database.path` 或 `logging.path`，数据和日志会写入你配置的位置。

## 常见问题

### 启动后无法访问服务

先检查健康检查地址：

```bash
curl http://127.0.0.1:3000/health
```

如果你配置了其他端口，把 `3000` 改成实际端口。

### 局域网设备访问失败

确认 `~/.zembra.env` 中的监听地址：

```toml
[server]
host = "0.0.0.0"
```

然后确认访问的是后端机器的局域网 IP：

```text
http://<后端机器 IP>:3000/health
```

如果浏览器前端请求失败，检查 `cors_allowed_origins` 是否包含前端页面地址。

### 启用同步时报配置错误

当 `sync.enabled = true` 时，必须同时配置：

```toml
supabase_url = "https://example.supabase.co"
service_role_key = "your-service-role-key"
```

### Swagger UI 无法打开

确认服务已经启动，并访问带尾部斜杠的地址：

```text
http://127.0.0.1:3000/swagger-ui/
```

### 数据库文件在哪里

默认在服务运行目录下的：

```text
data/zembra.db
```

可以通过 `~/.zembra.env` 修改：

```toml
[database]
path = "/path/to/zembra.db"
```
