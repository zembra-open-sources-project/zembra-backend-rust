# r026 v0.4.0 代码适配范围判断

日期：2026-05-31

需求澄清文档：`docs/request-clarify/r026-schema-v040-adaptation-scope.md`

## 核心功能（WHAT）

将后端 tags 相关模型、查询、API 合同和测试从 r025 的“兼容 v0.4.0 表结构”推进到“直接表达 v0.4.0 层级标签语义”。`TagRecord` 对齐 shared schema 的 `tags` 表字段，API 返回结构化标签对象，请求中的 `tags: Vec<String>` 明确表示完整标签路径。

### 需求背景（WHY）

`zembra-schema v0.4.0` 已将标签从平铺字符串升级为层级节点：`name` 是当前层级名，`path` 是完整路径，`parent_tag_id` 表达父子关系，`depth` 表达层级深度。r025 已完成 migration 和基础写入兼容，但当前后端仍通过 `path AS name` 把完整路径塞进旧 `TagRecord.name`，OpenAPI 和响应模型没有完整暴露层级标签结构。

客户端会跟随 schema 和后端 API 一起升级，因此 r026 不需要优先保留旧客户端兼容语义。

### 需求目标（GOAL）

| 目标 | 说明 |
| --- | --- |
| 模型对齐 | `TagRecord` 表达 `id/name/parent_tag_id/path/depth/created_at` |
| 查询对齐 | taxonomy、note tags、random tags 查询返回结构化 tag 字段 |
| 输入语义明确 | note create/update 的 `tags` 数组继续是完整 path 字符串 |
| metadata 便利字段 | note metadata 的 `tags` 继续返回完整 path 字符串，便于客户端直接展示或索引 |
| OpenAPI 对齐 | schema 和字段注释反映 v0.4.0 层级标签语义 |
| 测试覆盖 | repository、route、OpenAPI 和 random tags 覆盖结构化标签行为 |

### 范围边界

| 纳入范围 | 设计结论 |
| --- | --- |
| `TagRecord` | 扩展字段；`name` 改为当前节点名，新增 `parent_tag_id`、`path`、`depth` |
| `/tags` | 返回结构化 `TagRecord`；`names` 若继续保留，使用完整 `path` |
| `/notes/{note_ref}/tags` | 返回结构化 `TagRecord` |
| note response metadata | `metadata.tags` 继续是完整 path 字符串，来源为 `TagRecord.path` |
| `/random/tags` | 从所有标签节点随机抽样，返回结构化 `TagRecord` |
| note create/update | 输入 `tags` 仍为 `Vec<String>`，每个元素是完整路径 |
| sync 基础适配 | 保持 tag payload 的 v0.4.0 字段解析和写入，补基础字段校验测试 |

| 不纳入范围 | 原因 |
| --- | --- |
| 标签重命名 API | 需要级联更新后代 `path` |
| 标签移动 API | 需要更新父子关系、path、depth 和冲突处理 |
| 标签删除 API | 涉及子树、`note_tags` 关系和同步 tombstone |
| 多 workspace 标签管理 | 当前后端仍使用默认 workspace |
| 前端 UI | 本需求只处理后端适配 |
| sync 乱序补偿 | 已记录到 `docs/exec-plans/tech-debt-tracker.md` |

## 实现流程（HOW）

### 数据模型

| 字段 | Rust 类型 | 来源 | 语义 |
| --- | --- | --- | --- |
| `id` | `String` | `tags.id` | 标签节点稳定 ID |
| `name` | `String` | `tags.name` | 当前层级内标签名 |
| `parent_tag_id` | `Option<String>` | `tags.parent_tag_id` | 父标签 ID，根标签为 `None` |
| `path` | `String` | `tags.path` | 完整路径 |
| `depth` | `i64` | `tags.depth` | 根标签为 `0` |
| `created_at` | `i64` | `tags.created_at` | Unix timestamp |

`TagRecord` 继续作为 API schema 和 repository row model 使用，避免新增一组重复 DTO。所有 SQL 查询必须显式选择上述字段，禁止继续使用 `path AS name` 伪装旧模型。

### Repository 查询

| 位置 | 查询策略 |
| --- | --- |
| `TaxonomyRepository::list_tags` | `SELECT id, name, parent_tag_id, path, depth, created_at ... ORDER BY path ASC` |
| `get_or_create_tag_in_transaction` | 逐级按 `path` 查找或创建；返回最终标签节点的结构化字段 |
| `NotesRepository::list_note_tags_by_id` | join `note_tags` 后返回结构化 tag，排序按 `tags.path ASC` |
| `NotesRepository::random_tags` | 从当前 workspace 的所有 tags 节点 `ORDER BY RANDOM() LIMIT ?` |
| note metadata | 从 `TagRecord.path` 组装 `metadata.tags` |

### API 合同

| API | 变化 |
| --- | --- |
| `GET /tags` | `tags[]` 包含结构化字段；`names[]` 返回完整 path |
| `GET /notes/{note_ref}/tags` | 返回结构化 `TagRecord` |
| `GET /random/tags` | `tag` 是结构化 `TagRecord`，候选集合为所有标签节点 |
| `POST /notes` / `PATCH /notes/{note_ref}` | `tags` 请求字段说明为完整 tag path 列表 |

### 同步基础适配

`TagPayload` 保持解析 `name`、`parent_tag_id`、`path`、`depth`、`created_at`。r026 只处理字段存在性与基本类型校验，不设计父子 change 乱序补偿。远端缺父节点导致的应用失败沿用现有 conflict 记录路径，后续由技术债单独处理。

## 测试用例

| 测试类型 | 覆盖点 |
| --- | --- |
| 编译检查 | `cargo fmt --check`、`cargo check`、`cargo clippy -- -D warnings` |
| Repository 测试 | 层级 tag 创建后返回 `name=叶子名`、`path=完整路径`、`depth` 正确 |
| Route 测试 | `/tags` 和 `/notes/{note_ref}/tags` 返回结构化字段 |
| Random tags 测试 | 父节点和叶子节点都可进入随机候选集合 |
| OpenAPI 测试 | `TagRecord` schema 包含 `parent_tag_id`、`path`、`depth` |
| Sync 测试 | tag payload 缺少必需字段时记录不可应用，完整 payload 可写入结构化字段 |
| 回归检查 | `cargo test` 全量通过，既有 notes/taxonomy 行为没有无意外扩展 |
