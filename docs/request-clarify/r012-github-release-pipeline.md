# r012-github-release-pipeline

日期：2026-05-15

## 需求背景

当前项目已经具备 Rust 后端基础能力、SQLite migration、OpenAPI 文档、Supabase 同步配置和局域网访问支持，但仓库还没有 GitHub Actions CI、tag 发版流水线和发布使用说明。为了让后续版本可以稳定构建、验证和发布，需要先建立 GitHub 发版流水线的基础能力。

## 需求目标

本次需求只落地以下阶段：

| Stage | 目标 |
| --- | --- |
| Stage 1 | 建立 GitHub Actions CI 门禁 |
| Stage 2 | 建立 tag 驱动的 GitHub Release 流水线 |
| Stage 3 | 补充发布和安装使用文档 |

## 范围边界

### In Scope

| 项目 | 说明 |
| --- | --- |
| CI 触发 | `pull_request` 和 push 到 `master` 时运行 |
| CI 验证 | `cargo fmt --check`、`cargo check --locked`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` |
| Submodule | checkout 必须启用 `submodules: recursive`，确保 `vendor/zembra-schema` 可用 |
| Release 触发 | push `v*` tag 后触发 |
| Release 校验 | tag 版本必须和 `Cargo.toml` 中的 `package.version` 一致 |
| Release 产物 | 打包服务二进制、`config/default.toml`、`.env.example`、`LICENSE`；不包含 schema 文件 |
| 校验文件 | 发布 `SHA256SUMS` |
| 发布说明 | 文档说明如何下载、配置 `.zembra.env`、启动服务和验证 `/health` |

### Out of Scope

| 项目 | 本次不做原因 |
| --- | --- |
| Dockerfile | 容器化涉及数据目录、配置挂载和运行文档，放到后续需求 |
| GHCR 镜像发布 | 依赖 Dockerfile 和镜像 tag 策略，放到后续需求 |
| semantic-release 自动版本 | 当前历史提交中存在非白名单 commit type，先采用手动版本和 tag 发版 |
| release-please | 作为后续版本自动化备选，本次不接入 |
| Supabase 真实连通性测试 | 需要真实密钥和远端环境，不进入通用 CI |

## 验收标准

| 编号 | 标准 |
| --- | --- |
| A1 | PR 或 push 到 `master` 后，CI 能完成 fmt、check、test、clippy 验证 |
| A2 | 推送 `vX.Y.Z` tag 后，Release 流水线能校验 `Cargo.toml` 版本一致性 |
| A3 | Release 能生成 GitHub Release，并上传目标平台 tar.gz 产物 |
| A4 | Release 上传 `SHA256SUMS`，用户可校验下载产物完整性 |
| A5 | 发布文档能说明下载、解压、配置 `.zembra.env`、启动和健康检查流程 |

## 已确认决策

| 决策 | 结论 |
| --- | --- |
| 第一版发版方式 | tag 驱动 GitHub Release |
| 默认发布分支 | `master` |
| 版本来源 | `Cargo.toml package.version` |
| tag 格式 | `vX.Y.Z` |
| Docker/GHCR | 后续阶段处理 |
