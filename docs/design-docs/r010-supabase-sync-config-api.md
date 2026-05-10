# r010 Supabase 同步配置 API

日期：2026-05-10

需求澄清文档：`docs/request-clarify/r010-supabase-sync-config-api.md`

## 核心功能（WHAT）

新增 Supabase 同步配置 API，让前端可以读取同步配置摘要、保存 Supabase project URL、service role key、同步开关和同步频率，并在保存前测试连接。配置保存后写回 `~/.zembra.env`，同时立即影响手动同步 API。

### 需求背景（WHY）

当前 Supabase 同步功能已经实现，后端通过启动时读取 `.zembra.env` 获得 `[sync]` 配置，并已暴露 `/sync/status`、`/sync/run`、`/sync/push`、`/sync/pull`。前端无法直接配置 Supabase 连接信息，用户必须手工编辑本地配置文件并重启服务，配置体验不完整。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 前端可配置 | 前端通过 API 完成 Supabase URL、key、启用开关和同步间隔配置 |
| 持久化配置 | 保存结果写回 `~/.zembra.env` |
| 密钥安全 | 响应、日志、错误信息均不返回 `service_role_key` 明文 |
| 即时生效 | 保存后立即影响手动同步 API |
| 连接验证 | 保存前可通过测试 API 验证 Supabase REST 连通性 |
| 合同同步 | 新 API 注册 OpenAPI，并补齐 DTO schema |

### 范围边界

In Scope：

| 范围 | 内容 |
| --- | --- |
| 配置读取 API | `GET /sync/config` 返回配置摘要 |
| 配置保存 API | `PUT /sync/config` 校验并写回 `~/.zembra.env` |
| 连接测试 API | `POST /sync/config/test` 使用请求配置或已保存配置测试 Supabase |
| 运行时热更新 | 保存后更新内存配置，手动同步 API 立即使用新配置 |
| 密钥覆盖语义 | 请求包含新 key 时覆盖；请求不包含 key 时保留旧 key |
| OpenAPI | 新增 handler path 和 DTO schema 注册 |
| 自动化测试 | 覆盖读取、保存、保留 key、覆盖 key、OpenAPI path 和关键校验 |

Out of Scope：

| 范围 | 说明 |
| --- | --- |
| Supabase Auth | 不引入用户登录和 Supabase Auth |
| 前端确认弹窗 | 后端只提供覆盖语义，前端负责提示是否覆盖 |
| 多 workspace 配置 | 继续使用默认 workspace |
| 密钥加密存储 | 本轮继续沿用本地 `~/.zembra.env` 明文配置模式 |
| 冲突 UI | 不处理同步冲突展示 |
| 远端 schema 管理 | 不修改 Supabase migration |

## 实现流程（HOW）

### 总体架构

| 模块 | 职责 |
| --- | --- |
| `config` | 暴露用户配置文件路径、读取完整 settings、提供 sync 配置校验 |
| `services::sync_config` | 读取/写入 `.zembra.env`，合并请求和现有 key，构造安全响应 |
| `services::sync` | 持有可更新的 sync 配置快照，手动同步执行前读取最新配置 |
| `sync::supabase` | 增加轻量测试连接能力，并继续保证错误不包含 key |
| `dto::sync` 或 `dto::sync_config` | 定义配置 API request/response DTO |
| `handlers::sync` | 增加配置读取、保存、测试 handler |
| `routes::sync` | 注册 `/sync/config` 和 `/sync/config/test` |
| `api_doc` | 注册新增 path 和 schema |

### API 设计

| Method | Path | Request | Response |
| --- | --- | --- | --- |
| `GET` | `/sync/config` | 无 | `SyncConfigResponse` |
| `PUT` | `/sync/config` | `UpdateSyncConfigRequest` | `SyncConfigResponse` |
| `POST` | `/sync/config/test` | `TestSyncConfigRequest` | `SyncConfigTestResponse` |

### DTO 设计

`SyncConfigResponse`：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `enabled` | bool | 当前是否启用同步 |
| `interval_seconds` | u64 | 当前同步间隔 |
| `supabase_url` | string | 当前 Supabase project URL |
| `service_role_key_configured` | bool | 是否已保存 service role key |

`UpdateSyncConfigRequest`：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `enabled` | bool | 是否启用同步 |
| `interval_seconds` | u64 | 同步间隔 |
| `supabase_url` | string | Supabase project URL |
| `service_role_key` | Option<string> | 新 key；缺省时保留旧 key |

`TestSyncConfigRequest`：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `supabase_url` | Option<string> | 测试用 URL；缺省时使用已保存 URL |
| `service_role_key` | Option<string> | 测试用 key；缺省时使用已保存 key |

`SyncConfigTestResponse`：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `ok` | bool | 是否连通 |
| `message` | string | 脱敏后的测试结果说明 |

### 配置写回策略

推荐使用 TOML 文档解析和序列化维护配置文件，不用字符串拼接。写回流程：

| 步骤 | 说明 |
| --- | --- |
| 1 | 定位 `~/.zembra.env`，不存在时创建 |
| 2 | 读取现有 TOML，保留非 `[sync]` 配置字段 |
| 3 | 将请求字段合并到 `[sync]` |
| 4 | 请求不包含 `service_role_key` 时读取并保留旧值 |
| 5 | 使用 `SyncSettings::validate()` 校验合并结果 |
| 6 | 原子写回文件后更新运行时配置 |

说明：如果现有 `.zembra.env` 包含注释，TOML 重新序列化可能不保留注释。设计阶段接受这个取舍，优先保证结构化写回安全和可测试。

### 运行时热更新策略

推荐把 `SyncService` 调整为持有 `Arc<RwLock<SyncSettings>>`，每次执行 `status`、`push`、`pull`、`run_once` 前读取最新配置，并用最新配置构造或刷新 Supabase client。

| 场景 | 行为 |
| --- | --- |
| 保存配置成功 | 更新 `Arc<RwLock<SyncSettings>>` |
| 手动同步 API | 读取最新配置，立即生效 |
| 后台 worker | 每轮执行时复用同一个 `SyncService`，自然读取最新配置 |
| `enabled=false` | 手动同步继续返回 `sync_disabled` |
| `interval_seconds` 变更 | 后台 worker 的下一次 sleep 周期读取新间隔 |

为了避免在异步锁中执行网络请求，服务方法只在开始时 clone 当前 `SyncSettings`，随后释放锁并基于快照执行本轮同步。

### 连接测试策略

`POST /sync/config/test` 不保存配置。它先把请求中的 URL/key 与已保存配置合并，校验 URL 和 key 非空，再执行一次轻量 Supabase REST 请求。

推荐测试请求：

| 项目 | 决策 |
| --- | --- |
| Endpoint | `GET {supabase_url}/rest/v1/sync_changes?limit=1` |
| Header | `apikey` 和 `Authorization` 使用候选 key |
| 成功条件 | HTTP 2xx |
| 失败响应 | 返回脱敏 message，不包含 URL query 中的敏感信息和 key |

### 错误处理

| 场景 | 建议状态 | 说明 |
| --- | --- | --- |
| 请求 JSON 无效 | `400` | Axum 默认 JSON 提取错误 |
| `interval_seconds < 5` | `400` | 配置校验失败 |
| `enabled=true` 且缺少 URL/key | `400` | 配置校验失败 |
| `.zembra.env` 读写失败 | `500` | 本地配置文件错误 |
| Supabase 测试失败 | `200` + `ok=false` | 前端可展示失败原因，配置不保存 |
| 手动同步禁用 | `503` | 沿用现有 `sync_disabled` |

如果现有 `ApiError` 缺少配置错误类型，新增 `InvalidConfig` 或复用明确的 Bad Request 类型，并保证错误体不包含密钥。

### OpenAPI 维护

新增三个 handler 必须维护 `#[utoipa::path]`，声明 method、path、request_body、responses 和 tag。新增 DTO 必须派生 `utoipa::ToSchema`，并注册到 `src/api_doc.rs`。

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
| GET config 默认值 | 返回默认 sync 配置，key configured 为 false |
| GET config 脱敏 | 响应不包含 `service_role_key` 明文 |
| PUT config 创建文件 | 缺少 `.zembra.env` 时能创建并写入 `[sync]` |
| PUT config 保留 key | 请求缺省 key 时保留原 key |
| PUT config 覆盖 key | 请求包含 key 时覆盖原 key |
| PUT config 校验 | enabled=true 且缺少 URL/key 返回 400 |
| PUT config 热更新 | 保存后 `/sync/run` 使用最新 enabled 状态 |
| POST test 不持久化 | 测试请求不改变已保存配置 |
| POST test 脱敏 | 失败 message 不包含 key |
| OpenAPI | `/api-docs/openapi.json` 包含三个新增 path |

### 手工检查

| 用例 | 预期 |
| --- | --- |
| Swagger UI | `/swagger-ui` 可查看 sync config API |
| 前端保存配置 | 保存后本机 `~/.zembra.env` 中 `[sync]` 字段更新 |
| 前端测试连接 | URL/key 正确时返回 `ok=true`，错误时返回脱敏失败信息 |
