# r033 workspace 隔离机制需求澄清

日期：2026-06-25

当前同步冲突不是单纯的数据库同步失败，而是 workspace 隔离机制没有真正落地造成的系统性问题。schema 的原始设计把 workspace 作为同步隔离边界，但后端实现仍大量依赖固定 `DEFAULT_WORKSPACE_ID = 00000000-0000-4000-8000-000000000300`。这会导致新初始化本地库和已有远端数据拥有同一个 workspace id 但元数据时间不同，且缺少 workspace 对应的 `sync_changes` 后无法判断新旧。

本次需求目标是补齐真实 workspace 隔离机制。`zembra-backend init` 在初始化真实用户数据库时必须生成一个新的随机 UUID workspace，不能继续使用固定默认 workspace 作为真实身份。现有固定 workspace 数据需要迁移到本地新的随机 UUID，但迁移不能为了本地新 UUID 去强迫更新、覆盖或污染远端已有 workspace 数据。同步过程必须支持拉取不同 workspace 的数据，不能继续只按固定默认 workspace 过滤业务表。

新增 `GET /workspaces` API 返回当前本地可用 workspace 的有序列表。每个 workspace 返回完整 `workspace_id`、`short_hash`、可见笔记数量和最近一条可见笔记的创建时间。`short_hash` 规则为把 UUID 去掉连字符后取前 8 位，例如 `550e8400-e29b-41d4-a716-446655440000` 返回 `550e8400`。笔记数量和最近笔记统计必须严格遵守当前 notes 可见性纪律，只统计 `deleted_at IS NULL` 且 `archived_at IS NULL` 的笔记。空 workspace 的 `latest_note_created_at` 返回 `null`。

`GET /workspaces` 列表按可见笔记数量从多到少排序。数量相同时，为了保持返回稳定，按 `latest_note_created_at DESC NULLS LAST` 再按 `workspace_id ASC` 排序。短 hash 只用于展示，真实标识使用完整 `workspace_id`。

本需求验收通过的唯一标准是 sandbox 外启动 backend 后不再出现由 workspace id 引起的同步错误。新初始化数据库生成随机 UUID workspace、真实用户路径不再产生固定默认 workspace、已有固定 workspace 可以迁移到本地新 UUID 且不主动覆盖远端 workspace、同步能读取和写入多个 workspace 的数据、`GET /workspaces` 返回完整 `workspace_id`、8 位 `short_hash`、可见笔记数量和最近可见笔记创建时间、统计口径排除已删除和已归档笔记、空 workspace 的最近笔记时间为 `null`、OpenAPI 暴露该 API 合同，这些都是支撑唯一验收标准的功能要求。
