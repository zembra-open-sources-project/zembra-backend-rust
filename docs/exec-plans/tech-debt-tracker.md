# Tech Debt Tracker

## r026: sync 层级标签乱序补偿

日期：2026-05-31

背景：`zembra-schema v0.4.0` 的 tags 使用 `parent_tag_id` 表达父子关系。远端同步拉取 tag insert change 时，理论上可能先收到子标签、后收到父标签，导致本地应用子标签时触发外键失败。

技术债：当前 r026 不实现父子标签 change 乱序补偿。后续需要设计 sync apply 的依赖排序、延迟重试或 pending 队列，避免将可恢复的乱序同步误判为永久 schema conflict。

边界：本地创建标签不受此技术债影响；本地必须按 path 逐级创建父节点和子节点。
