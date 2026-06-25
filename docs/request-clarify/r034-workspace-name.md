# r034 workspace name 返回与交互初始化需求澄清

日期：2026-06-25

r033 workspace 隔离机制遗漏了 `workspace_name`。`zembra-schema` 已经在 `workspaces` 表中定义 `workspace_name`，它是 workspace 的可选显示名，但当前 `GET /workspaces` 只返回 `workspace_id`、`short_hash`、可见笔记数量和最近可见笔记创建时间，没有返回 `workspace_name`。这会让客户端只能展示 UUID 或短 hash，无法展示 schema 已支持的人类可读名称。

本次需求目标是补齐 workspace name 的 API 返回和初始化输入。`GET /workspaces` 每个 workspace 必须返回 `workspace_name` 字段，字段值保持 schema 语义，允许为 `null`。新初始化随机 workspace 时必须通过交互输入获得 workspace name，不能静默生成空 name，也不能在用户未提供有效输入时使用 fallback name。workspace name 禁止包含空格或任何 whitespace，空字符串、全空白、包含任意 whitespace 的输入都无效。

交互输入最多提示 3 次。3 次仍未获得有效 workspace name 时，初始化必须失败退出，不创建或继续使用 fallback workspace name。当前需求不补生成已有 `workspace_name IS NULL` 的 workspace 名称；已经存在空 name 的历史数据保持原样。当前需求不新增 workspace 创建 API、不新增 workspace 重命名 API、不做 workspace name 唯一性约束。

验收标准：`GET /workspaces` 响应项包含 `workspace_name`；OpenAPI schema 暴露该字段；`zembra-backend init` 在需要创建随机 workspace 时交互询问 workspace name；合法 name 能写入 `workspaces.workspace_name`；空输入、全空白和包含 whitespace 的输入会被拒绝并最多重试 3 次；3 次失败后初始化退出且不使用 fallback；已有 `workspace_name IS NULL` 的 workspace 不被自动补名。
