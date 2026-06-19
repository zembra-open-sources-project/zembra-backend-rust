# Tech Debt Tracker

## r026: sync 层级标签乱序补偿

日期：2026-05-31

背景：`zembra-schema v0.4.0` 的 tags 使用 `parent_tag_id` 表达父子关系。远端同步拉取 tag insert change 时，理论上可能先收到子标签、后收到父标签，导致本地应用子标签时触发外键失败。

技术债：当前 r026 不实现父子标签 change 乱序补偿。后续需要设计 sync apply 的依赖排序、延迟重试或 pending 队列，避免将可恢复的乱序同步误判为永久 schema conflict。

边界：本地创建标签不受此技术债影响；本地必须按 path 逐级创建父节点和子节点。

## r027: Supabase schema 适配归属迁移

日期：2026-06-19

背景：当前仓库仍保留 `supabase/migrations/001_initial_sync_schema.sql`，这是 r009 接入 Supabase 同步时为补齐远端 Postgres 表结构而加入的临时归属。项目现在已明确 `zembra-schema` 是本地 SQLite、Supabase/Postgres 远端备份以及任何后续数据库形态的唯一数据契约来源。

技术债：本仓库存在历史 Supabase schema 适配产物，和最新的 schema ownership 边界不一致。后续需要将 Supabase/Postgres 远端备份 schema 的契约归属迁回 `zembra-schema`，本仓库只消费 `vendor/zembra-schema` 中固定版本的数据契约。

边界：在 `zembra-schema` 完成契约迁移前，本仓库禁止继续扩展或修补 Supabase schema；任何需要新增、修改或推断数据库 schema 的需求都必须先转到 `zembra-schema` 处理。
