# r026 v0.4.0 代码适配范围判断

日期：2026-05-31

## 需求理解

基于 `zembra-schema v0.4.0` 的层级标签结构，判断当前后端已有代码还需要做哪些适配修改。这个需求不等同于再次升级 schema 版本；`r025` 已完成 submodule、migration 和最低兼容写入，本轮要明确后续代码模型、API、同步和测试的适配边界。

## v0.4.0 变化摘要

| 变化点 | 含义 |
| --- | --- |
| `tags.parent_tag_id` | 标签从平铺模型变成邻接表层级模型 |
| `tags.path` | 保存完整路径，在同一 workspace 内唯一，例如 `books/hands-on-python` |
| `tags.depth` | 保存层级深度，根标签为 `0` |
| `note_tags.tag_id` | 继续指向标签节点，可指向任意层级；是否只允许叶子由应用层决定 |
| 旧数据迁移 | `004_add_hierarchical_tags.sql` 会拆分旧斜杠路径标签，保留叶子节点原 tag id |

## 仓库现状关联

| 模块 | 当前状态 | 适配判断 |
| --- | --- | --- |
| `vendor/zembra-schema` | 已固定到 `v0.4.0` | 已适配 |
| `src/repositories/database.rs` | 已执行 `004_add_hierarchical_tags.sql` 并记录 `0.4.0` | 已适配 |
| `src/repositories/taxonomy.rs` | 已按 `/` 拆分路径逐层创建标签，查询返回 `path AS name` | 基础兼容已适配；缺少显式层级模型 |
| `src/models/tag.rs` | `TagRecord` 仍只有 `id/name/created_at` | 需要适配，用模型表达 `parent_tag_id/path/depth` |
| `src/dto/taxonomy.rs` | `/tags` 返回 `TagRecord` 和 `names` | 需要判断是否扩展响应，至少 OpenAPI schema 应暴露层级字段 |
| `src/dto/notes.rs` | note metadata 和 note tags 仍只面向标签字符串或旧 `TagRecord` | 需要判断是否保持字符串兼容，同时增加结构化 tag 信息 |
| `src/repositories/notes/tags.rs` | note tag 查询、添加、删除、替换已按 `path` 兼容 | 需要补充层级语义测试和删除/替换边界 |
| `src/repositories/notes/core.rs` | random tags 返回 `path AS name` | 需要判断随机抽样是否包含父标签；v0.4.0 允许任意层级，但产品可能只想抽叶子 |
| `src/repositories/sync/payload.rs` | tag insert payload 已能解析 `parent_tag_id/path/depth` | 需要补强校验，避免 path/depth 与 name/parent 不一致 |
| `src/repositories/sync/apply.rs` | remote tag insert 已写入 v0.4.0 必填列 | 需要判断远端乱序同步时，子标签早于父标签是否要记录 conflict 或延迟应用 |
| OpenAPI | `TagRecord` schema 仍是旧形态 | 需要适配，否则 API 合同没有表达 v0.4.0 标签结构 |
| 测试 | 已有基础层级创建测试 | 需要覆盖 API 响应、OpenAPI、sync apply 和旧数据迁移后的查询行为 |

## 推荐纳入 r026 的范围

| 范围项 | 推荐结论 |
| --- | --- |
| Tag 模型 | 新增或扩展结构化 tag 响应模型，表达 `id/name/parent_tag_id/path/depth/created_at` |
| API 响应兼容 | 保留现有 `names: Vec<String>` 和 note metadata `tags: Vec<String>`，继续返回完整路径字符串 |
| `/tags` 响应 | `tags[]` 应返回结构化层级字段；排序继续按 `path ASC` |
| note tag 响应 | `GET /notes/{note_ref}/tags` 返回结构化层级字段；metadata 继续返回完整路径字符串 |
| random tags | 明确默认抽样口径；推荐只抽有笔记关联的标签节点，避免随机到无笔记父节点 |
| sync payload | tag insert payload 应要求 `path` 和 `depth`，旧 payload 兼容只保留在明确的向后兼容分支 |
| sync apply | 子标签父节点缺失时记录 `schema_incompatible` conflict，不静默创建不可信父节点 |
| 测试 | 增加 repository、route、OpenAPI、sync apply 和 migration 回归测试 |
| 文档 | 更新 r026 设计文档和执行计划，说明 API 兼容策略和层级字段语义 |

## 推荐不纳入 r026 的范围

| 非范围项 | 理由 |
| --- | --- |
| 新增标签重命名 API | v0.4.0 说明重命名需要同步后代 `path`，这是独立写入能力 |
| 新增标签移动 API | 移动会影响父子关系、路径级联和同步冲突，适合作为单独需求 |
| 新增标签删除 API | 删除涉及 `note_tags` 关系、子树处理和同步 tombstone，范围较大 |
| 多 workspace 标签管理 | 当前后端仍使用默认 workspace，r026 不改变 workspace 入参 |
| 前端层级树 UI | 当前仓库是后端，r026 只定义后端适配范围 |
| 强制叶子标签关联规则 | v0.4.0 允许任意层级关联；是否只允许叶子是产品决策，不默认收紧 |

## 适配优先级

1. **API 合同适配**：让 `TagRecord` / tag response / OpenAPI 正确表达 v0.4.0 结构。
2. **同步安全适配**：确保 tag payload 和 remote apply 不制造不完整层级数据。
3. **查询语义适配**：确认 `/tags`、note tags、random tags 在父节点和叶子节点混合时的返回口径。
4. **回归测试补齐**：覆盖层级字段、旧数据迁移、API schema 和 sync conflict。

## 需要决策的问题

1. **`TagRecord.name` 是否继续表示完整路径？**
   - 推荐：保留 `name = path` 兼容现有客户端，同时新增 `display_name` 或 `local_name` 表示当前层级名。
   - 备选：改为 `name = 当前层级名`，并要求客户端改用 `path` 展示完整路径。

2. **`/random/tags` 是否允许抽到父标签节点？**
   - 推荐：只从有 `note_tags` 关联的标签节点抽样，避免返回无笔记父标签组。
   - 备选：从所有 tags 抽样，父标签没有直接关联笔记时返回空组。

3. **远端 sync 收到缺父节点的子标签时怎么处理？**
   - 推荐：记录 `schema_incompatible` conflict，等待父节点 change 后再由后续同步修复。
   - 备选：根据 payload 自动补建父节点，但需要额外定义生成 ID 和可信路径规则。

## 当前成功标准

- r026 设计前，用户确认上述 3 个决策点。
- r026 实施后，API schema、repository 查询、sync apply 和测试都能明确表达并验证 v0.4.0 层级标签语义。
