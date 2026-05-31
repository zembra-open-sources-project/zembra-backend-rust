# r026 v0.4.0 代码适配范围判断

日期：2026-05-31

## 需求理解

基于 `zembra-schema v0.4.0`，将后端已有代码从“最低限度兼容层级标签表结构”升级为“完整表达层级标签语义”。`r025` 已完成 schema submodule、migration 和基础写入兼容；`r026` 负责确认后续模型、API 合同、查询语义、同步基础校验和测试的适配范围。

本轮明确前提：客户端会跟随 schema 和后端 API 一起升级。后端适配 shared schema 修改时，不把旧客户端兼容性作为优先约束，除非后续需求特别强调。

## v0.4.0 变化摘要

| 变化点 | 含义 |
| --- | --- |
| `tags.name` | 当前层级内的标签名，例如 `python` |
| `tags.parent_tag_id` | 父标签 ID；为空表示根标签 |
| `tags.path` | 完整标签路径，在同一 workspace 内唯一，例如 `books/python` |
| `tags.depth` | 标签深度；根标签为 `0` |
| `note_tags.tag_id` | 继续指向标签节点；schema 允许关联任意层级标签 |

## 已确认范围

| 问题 | 结论 |
| --- | --- |
| 客户端兼容性 | 客户端会同步升级，r026 不保留旧客户端优先兼容策略 |
| `tags` 请求输入 | note create/update 继续接收 `tags: Vec<String>`，语义为完整 tag path 列表 |
| `TagRecord.name` | 改为当前节点名，对齐 `tags.name` |
| 完整路径展示 | 使用 `path` 字段表达完整标签路径 |
| `/random/tags` 候选集合 | 按所有标签节点抽样，不限制为叶子节点或已有直接 note 关联的节点 |
| 本地标签创建 | 输入完整 path 时，后端必须逐级创建缺失父节点和最终标签节点 |
| sync 乱序补偿 | 不进入 r026，记录为技术债 |

## 仓库现状关联

| 模块 | 当前状态 | r026 适配判断 |
| --- | --- | --- |
| `vendor/zembra-schema` | 已固定到 `v0.4.0` | 不需要改 |
| `src/repositories/database.rs` | 已执行 `004_add_hierarchical_tags.sql` 并记录 `0.4.0` | 不需要改 |
| `src/models/tag.rs` | `TagRecord` 仍只有 `id/name/created_at` | 需要扩展为 v0.4.0 结构 |
| `src/repositories/taxonomy.rs` | 查询使用 `path AS name`，写入已逐级创建 | 查询需要改为返回 `name/path/parent_tag_id/depth`；写入逻辑保留并补测试 |
| `src/repositories/notes/tags.rs` | note tags 查询使用 `path AS name` | 需要改为结构化 tag 返回，metadata 使用 `path` 生成路径字符串 |
| `src/repositories/notes/core.rs` | random tags 使用旧 `TagRecord` 投影 | 需要返回结构化 tag，并按所有标签节点抽样 |
| `src/dto/taxonomy.rs` | `/tags` 响应依赖旧 `TagRecord` | 需要更新 OpenAPI schema 与响应语义 |
| `src/dto/notes.rs` | note tag 响应、random tag 响应依赖旧 `TagRecord` | 需要更新 schema 文档和测试 |
| `src/repositories/sync/payload.rs` | tag payload 已解析 `parent_tag_id/path/depth` | 需要保留基础校验；不做乱序补偿 |
| `src/repositories/sync/apply.rs` | remote tag insert 写入 v0.4.0 字段 | 父节点缺失时遵守外键失败路径；乱序补偿另行处理 |
| OpenAPI | `TagRecord` schema 仍是旧形态 | 需要暴露 v0.4.0 字段 |
| 测试 | 已有基础层级创建测试 | 需要覆盖结构化响应、OpenAPI 和 random tags 语义 |

## 纳入 r026 的范围

| 范围项 | 目标 |
| --- | --- |
| Tag 模型 | `TagRecord` 表达 `id/name/parent_tag_id/path/depth/created_at` |
| `/tags` 响应 | `tags[]` 返回结构化标签对象，`names[]` 如保留则使用完整 `path` |
| note tags 响应 | `GET /notes/{note_ref}/tags` 返回结构化标签对象 |
| note metadata | metadata 中的 `tags: Vec<String>` 使用完整 `path`，保持它作为便捷字段 |
| note create/update 输入 | 明确 `tags` 数组元素是完整路径；本地按 path 逐级创建标签节点 |
| `/random/tags` | 从所有标签节点中随机抽样，返回结构化标签对象 |
| OpenAPI | `TagRecord` schema 暴露 `parent_tag_id/path/depth`，请求字段说明 tags path 语义 |
| sync 基础适配 | tag insert payload 保持 v0.4.0 字段解析和写入，校验字段存在性与基本类型 |
| 测试 | 覆盖 repository、route、OpenAPI、random tags 和层级创建行为 |

## 不纳入 r026 的范围

| 非范围项 | 理由 |
| --- | --- |
| 标签重命名 API | 重命名需要同步自身和后代 `path`，适合作为独立写入需求 |
| 标签移动 API | 移动需要更新父子关系、path、depth 和冲突处理 |
| 标签删除 API | 删除涉及子树、`note_tags` 关系和同步 tombstone |
| 多 workspace 标签管理 | 当前后端仍使用默认 workspace，不在 r026 改变 workspace 入参 |
| 前端 UI | r026 只定义后端适配范围 |
| sync 乱序补偿 | 父子标签 change 乱序应用需要队列/重试/依赖排序设计，先记录技术债 |

## 验收标准

- `TagRecord` 和 OpenAPI schema 明确包含 `name`、`parent_tag_id`、`path`、`depth`。
- `/tags`、`/notes/{note_ref}/tags`、`/random/tags` 返回结构化标签对象。
- note metadata 的 `tags` 仍提供完整 path 字符串，方便客户端直接展示或索引。
- note create/update 传入 `books/python` 时，本地创建或复用 `books` 和 `books/python` 两级节点。
- `/random/tags` 的候选集合来自所有标签节点。
- sync 乱序补偿不在 r026 实施，已写入技术债。
