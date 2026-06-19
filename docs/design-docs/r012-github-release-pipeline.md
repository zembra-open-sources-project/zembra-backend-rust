# r012-github-release-pipeline

日期：2026-05-15

需求澄清文档：`docs/request-clarify/r012-github-release-pipeline.md`

## 核心功能（WHAT）

为 `zembra-backend-rust` 建立 GitHub 发版基础设施，覆盖 CI 质量门禁、tag 驱动 GitHub Release 和发布使用文档。第一版不引入 Docker、GHCR、semantic-release 或 release-please。

### 需求背景（WHY）

项目当前已经形成稳定的本地验证组合：`cargo fmt --check`、`cargo check`、`cargo test`、`cargo clippy -- -D warnings`。仓库包含 `Cargo.lock` 和 `vendor/zembra-schema` submodule，但没有 `.github` workflow 和 release 产物规范。发版流水线需要把这些已有约束自动化，减少手工构建和发布风险。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| CI 可复用 | PR 和 `master` push 都执行同一组 Rust 验证 |
| 发版可追溯 | `vX.Y.Z` tag 触发 Release，且 tag 与 `Cargo.toml` 版本一致 |
| 产物可下载 | GitHub Release 上传平台二进制 tar.gz |
| 产物可校验 | Release 上传 `SHA256SUMS` |
| 用户可运行 | 文档覆盖下载、配置、启动和健康检查 |

### 范围边界

| 类型 | 内容 |
| --- | --- |
| In Scope | `.github/workflows/ci.yml`、`.github/workflows/release.yml`、发布文档 |
| Out of Scope | Dockerfile、GHCR、semantic-release、release-please、真实 Supabase 连通性测试 |

## 实现流程（HOW）

### CI Workflow

| 项目 | 设计 |
| --- | --- |
| 文件 | `.github/workflows/ci.yml` |
| 触发 | `pull_request`、push 到 `master` |
| checkout | `actions/checkout`，启用 `submodules: recursive` |
| Rust toolchain | stable |
| 依赖缓存 | Cargo registry、Cargo git、`target` |
| 验证命令 | `cargo fmt --check`、`cargo check --locked`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` |

### Release Workflow

| 项目 | 设计 |
| --- | --- |
| 文件 | `.github/workflows/release.yml` |
| 触发 | push `v*` tag |
| 权限 | `contents: write` |
| 版本校验 | 从 tag 提取 `X.Y.Z`，与 `Cargo.toml package.version` 比对 |
| 发布前验证 | 运行 fmt、test、clippy，避免未验证 tag 发版 |
| Release | 使用 GitHub Release 上传 assets |
| Release notes | 使用 GitHub 自动生成 release notes |

### 发布产物

第一版推荐先发布 Linux 和 macOS 二进制。若交叉编译配置复杂，可优先落地 runner 原生目标，后续补充更多 target。

| 产物内容 | 说明 |
| --- | --- |
| `zembra-backend-rust` | 编译后的后端服务二进制 |
| `config/default.toml` | 默认配置 |
| `.env.example` | 用户配置示例 |
| `LICENSE` | 许可证 |
| `SHA256SUMS` | 所有 tar.gz 的 SHA-256 校验 |

产物不包含 schema 文件、`data/`、`logs/`、`.zembra.env`、本地数据库文件或任何密钥。数据库契约由仓库固定的 `vendor/zembra-schema` 版本提供，不复制到 release 包。

### 发布文档

推荐新增或更新仓库发布文档，内容包括：

| 章节 | 内容 |
| --- | --- |
| 下载 | 从 GitHub Release 选择对应平台 tar.gz |
| 校验 | 使用 `SHA256SUMS` 校验 |
| 配置 | 复制并调整 `.zembra.env`，说明配置仍位于用户 home 目录 |
| 启动 | 运行 `./zembra-backend-rust` |
| 验证 | 请求 `GET /health`，必要时访问 `/api-docs/openapi.json` 和 `/swagger-ui` |

## 测试用例

### 编译检查

| 用例 | 预期结果 |
| --- | --- |
| CI 运行 `cargo fmt --check` | 格式检查通过 |
| CI 运行 `cargo check --locked` | 编译检查通过，且不更新 lockfile |
| CI 运行 `cargo test --locked` | 单元和集成测试通过 |
| CI 运行 `cargo clippy --locked -- -D warnings` | 无 warning |

### 手工检查

| 用例 | 预期结果 |
| --- | --- |
| 推送测试 tag | 创建 GitHub Release |
| 下载 tar.gz | 解压后包含二进制、默认配置、示例配置、Supabase migration 和 LICENSE |
| 校验 SHA256SUMS | 校验通过 |
| 本地运行二进制 | `/health` 返回 200 |

### 回归检查

| 用例 | 预期结果 |
| --- | --- |
| tag 与 Cargo 版本不一致 | Release workflow 失败并提示版本不一致 |
| submodule 未检出 | CI 或 Release 在构建阶段失败，workflow 配置应避免该问题 |
| 发布包误含本地数据 | tar.gz 不包含 `data/`、`logs/`、`.zembra.env` |
