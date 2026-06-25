# r031 绝对数据库路径约束需求澄清

日期：2026-06-25

当前同步报错 `cannot determine newer row from sync_changes.created_at` 的直接触发条件是后端使用了相对 `database.path = "data/zembra.db"`，导致服务从不同工作目录启动时连接到不同 SQLite 文件。当前误连到的库缺少原有业务数据和 `sync_changes`，同步比对时无法通过 `sync_changes.created_at` 判断业务行新旧。

本次需求目标是禁止默认配置和用户配置使用相对 SQLite 数据库路径。`database.path` 必须是绝对路径，`zembra-backend config init` 生成的 `~/.zembra.env` 必须写入用户目录下的绝对数据库路径，文档示例也必须改为绝对路径。同步冲突判断逻辑本轮不改，因为它在无法判断新旧时停止是正确保护。

验收标准：相对 `database.path` 在配置校验中失败；`config init` 生成的数据库路径是绝对路径；仓库默认配置和用户文档不再指导使用 `data/zembra.db` 作为运行数据库。
