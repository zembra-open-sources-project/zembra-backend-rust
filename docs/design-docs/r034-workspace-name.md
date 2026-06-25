# r034 workspace name 返回与交互初始化设计文档

日期：2026-06-25

需求澄清文档：`docs/request-clarify/r034-workspace-name.md`

## 核心功能（WHAT）

补齐 workspace name 在 API 和初始化流程中的表达。`GET /workspaces` 返回 `workspace_name`；`zembra-backend init` 创建随机 workspace 时通过交互输入获取合法 workspace name，并把它写入 `workspaces.workspace_name`。

### 需求背景（WHY）

`zembra-schema` 已有 `workspaces.workspace_name`，r033 实现 workspace 隔离时遗漏了这个字段，导致 API 只能返回完整 UUID 和短 hash。workspace 隔离解决的是身份边界，workspace name 解决的是用户可识别性。初始化阶段是首次创建真实本地 workspace 的入口，因此需要在这里要求用户提供 name，避免真实用户库继续产生没有显示名的新 workspace。

### 需求目标（GOAL）

`GET /workspaces` 的响应项新增 `workspace_name`，值来自 `workspaces.workspace_name`，允许为 `null`。`zembra-backend init` 在创建随机 workspace 前交互提示输入 workspace name，验证 name 非空且不包含任何 whitespace；最多提示 3 次，3 次失败后返回初始化错误并退出。

### 范围边界

纳入范围：workspace summary repository 查询 `workspace_name`，DTO 和 OpenAPI 暴露 `workspace_name`，初始化路径增加交互输入和校验，初始化测试覆盖合法输入、非法输入重试和 3 次失败。非范围：不新增 `POST /workspaces`，不新增 workspace rename API，不回填已有 `workspace_name IS NULL` 的历史数据，不做 name 唯一性约束，不改变同步协议字段。

## 实现流程（HOW）

`WorkspaceSummaryRow` 增加 `workspace_name: Option<String>`，`list_summaries()` 的 SELECT 增加 `workspaces.workspace_name AS workspace_name`，`WorkspaceSummary` DTO 增加同名字段。`GET /workspaces` handler 映射 repository row 时直接透传该字段。OpenAPI 组件注册继续使用 `WorkspaceSummary`，schema 中应出现 `workspace_name`。

初始化流程新增一个小型输入抽象，避免测试依赖真实 stdin。推荐在 `src/init.rs` 中增加 `WorkspaceNameInput` trait 或等价结构，生产路径读取 stdin，测试路径提供固定输入序列。`init_global()` 在需要创建默认数据库和配置文件时触发 workspace name 交互；如果数据库和配置文件都已存在并返回 `Skipped`，不询问 name，不补生成已有 name。

workspace name 校验函数放在 workspace 领域模块中复用，规则为 `trim()` 后不能为空，且原始输入中不能包含任何 `char::is_whitespace()` 字符。合法 name 原样写入或写入 trim 后结果需要固定一个口径；推荐写入 trim 后结果，因为前后空白本来无效且不应持久化。3 次输入都无效时返回明确错误，例如 `workspace name is required and must not contain whitespace`，并停止初始化。

创建或迁移随机 workspace name 的写入点应只作用于本轮新创建的真实用户 workspace。已有本地库中 `workspace_name IS NULL` 的 workspace 不做自动补名。r033 的 legacy fixed workspace 本地迁移如果发生在 `zembra-backend init` 创建路径中，应使用本次交互获得的 name 写入迁移后的 workspace；普通 server 启动迁移历史库时不得交互阻塞，也不得补生成 name。

## 测试用例

| 场景 | 预期 |
| --- | --- |
| `GET /workspaces` | 响应项包含 `workspace_name`，值等于数据库字段，允许 `null` |
| OpenAPI JSON | `WorkspaceSummary` schema 包含 `workspace_name` |
| `zembra-backend init` 输入合法 name | 初始化成功，`workspaces.workspace_name` 写入该 name |
| 输入空字符串、全空白、包含 whitespace | 被拒绝并继续提示，最多 3 次 |
| 连续 3 次非法输入 | 初始化失败退出，不使用 fallback name |
| 数据库和配置文件都已存在 | 返回 `Skipped`，不询问 name，不补生成空 name |
| 已有 `workspace_name IS NULL` 的库启动 server | 不自动补生成 name |
