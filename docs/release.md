# GitHub Release 发布与安装

日期：2026-05-15

## 发布方式

本项目通过 GitHub tag 触发发版。tag 必须使用 `vX.Y.Z` 格式，并且 `X.Y.Z` 必须和 `Cargo.toml` 中的 `package.version` 完全一致。

发布流水线会在创建 Release 前执行：

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

验证通过后，流水线会构建并上传平台产物和 `SHA256SUMS`。

## 产物内容

每个 tar.gz 产物包含：

| 路径 | 用途 |
| --- | --- |
| `zembra-backend-rust` | 后端服务二进制 |
| `config/default.toml` | 默认配置 |
| `.env.example` | 用户配置示例 |
| `supabase/migrations/` | Supabase 远端 schema migration |
| `LICENSE` | 许可证 |

发布包不包含 `data/`、`logs/`、`.zembra.env`、SQLite 数据库文件或任何密钥。

## 下载与校验

从 GitHub Release 页面下载当前机器对应的 tar.gz 和 `SHA256SUMS`。

在下载目录执行：

```bash
shasum -a 256 -c SHA256SUMS
```

如果只下载了单个平台产物，也可以执行：

```bash
shasum -a 256 zembra-backend-rust-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

然后和 `SHA256SUMS` 中对应行比对。

## 安装与配置

解压发布包：

```bash
tar -xzf zembra-backend-rust-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
cd zembra-backend-rust-v0.1.0-x86_64-unknown-linux-gnu
```

根据 `.env.example` 准备用户配置文件：

```bash
cp .env.example ~/.zembra.env
```

按实际运行环境调整 `~/.zembra.env`。常用配置如下：

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

如果需要让局域网设备访问服务，可以把 `server.host` 设置为 `"0.0.0.0"`，并按前端地址配置 `server.cors_allowed_origins`。

## 启动与验证

启动服务：

```bash
./zembra-backend-rust
```

健康检查：

```bash
curl http://127.0.0.1:3000/health
```

OpenAPI JSON：

```bash
curl http://127.0.0.1:3000/api-docs/openapi.json
```

Swagger UI：

```text
http://127.0.0.1:3000/swagger-ui/
```

## 创建新版本

发版前先更新 `Cargo.toml` 中的版本号，并确保本地验证通过：

```bash
cargo fmt --check
cargo check --locked
cargo test --locked
cargo clippy --locked -- -D warnings
```

创建并推送 tag：

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions 会自动创建对应 Release。

## 后续范围

以下能力不属于本轮 GitHub Release 流水线：

| 能力 | 后续前置条件 |
| --- | --- |
| Dockerfile | 明确运行用户、SQLite 数据目录、`~/.zembra.env` 挂载方式和端口暴露策略 |
| GHCR 镜像发布 | 完成 Dockerfile 后再定义镜像 tag、`latest` 策略和权限 |
| 自动版本工具 | 历史提交规范稳定后再接入 release-please 或 semantic-release |
