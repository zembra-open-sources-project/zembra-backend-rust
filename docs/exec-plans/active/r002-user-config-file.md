# r002 用户配置文件读取执行计划

日期：2026-04-26

需求澄清文档：`docs/request-clarify/r002-user-config-file.md`
设计文档：`docs/design-docs/r002-user-config-file.md`

## Stage 1 配置模型与加载行为

| Task | 状态 | 功能 | 实现要点 | 预期测试结果 |
| --- | --- | --- | --- | --- |
| T1 | Finished | 统一数据库配置字段 | 将 `database.url` 改为 `database.path`，提供 SQLite URL 转换方法 | 默认配置可反序列化，路径能转换为 SQLx URL |
| T2 | Finished | 增加用户配置文件来源 | 读取 `~/.zembra.env`，不存在时 warning，存在时覆盖默认配置 | 缺失用户配置不阻断启动，存在时覆盖默认路径 |
| T3 | Finished | 移除环境变量配置来源 | 删除 `ZEMBRA__...` 配置覆盖入口 | 配置加载不依赖环境变量 |

## 进度记录

- 2026-04-26：完成需求澄清与设计，等待实现。
- 2026-04-26：完成配置模型和加载逻辑实现，等待验证。
