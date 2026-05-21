# AGENTS

## Codex 持续开发工作流

### 开发流程

本项目的所有需求开发，除非用户在 Prompt 中指定特殊流程，否则严格遵守如下步骤完成：

1. 需求澄清：接到用户需求后，首先进行需求澄清，确认功能范围。需求澄清过程以用户多轮交互的方式完成，澄清结束后，按模块写入 docs/request-clarify/ 下对应目录，写入方式为新建文件。
2. 基于需求澄清结果，完成方案设计，必要时和用户交互完成方案决策，按模块新建文件写入 docs/design-docs/ 下对应目录，该文件中需要记录需求澄清文档位置。写入后提醒用户审阅。设计文档中禁止堆砌代码，涉及接口设计、字段设计等内容时，以表格形式清晰说明即可，必要时可使用函数签名，但不要在文档中列举代码实现。
3. 根据技术设计结果，制定执行计划，按模块新建文件写入到 docs/exec-plans/active/ 下对应目录，提醒用户审核修改。开发计划必须记录需求澄清文档和设计文档的路径，开发计划中根据需求的工作量，划分为不同 Stage，并在 Stage 根据必要性划分为不同的 Task。
4. 根据执行计划进行开发，并更新执行计划中的任务状态，完成每个 Stage 后，如果修改了任何代码，必须进行一次 git 提交。
5. 用户验证后，将开发计划移动到 docs/exec-plans/completed/，并更新`docs/PROGRESS.md`文件。在未经用户允许，禁止擅自归档
6. 所有文档使用 markdown 格式撰写，语言为简体中文，关键术语、函数接口等信息可保留英文

### 初始化规则
- 初始化命令只创建缺失目录和文件。
- 已存在目录和文件必须 SKIP，禁止覆盖、删除、重命名或改写已有内容。
- 初始化输出只报告新建和跳过的目录/文件，不输出项目业务内容。
- 初始化只建立持续开发工作流骨架，不执行具体需求开发。

### 文档系统说明
- AGENTS.md 记录全局工作流、文档阅读顺序、产物落点、禁止事项和关键入口。
- ARCHITECTURE.md 描述系统全局结构、模块边界、依赖方向和不变量。
- docs/request-clarify/ 存放需求澄清结果，可按模块建立子目录。
- docs/design-docs/ 存放设计决策、设计理念和子系统方案，可按模块建立子目录。
- docs/product-specs/ 存放功能规格、用户流程、验收标准。
- docs/exec-plans/active/ 存放进行中的执行计划，可按模块建立子目录。
- docs/exec-plans/completed/ 存放已完成且已通过用户验收的执行计划，可按模块建立子目录。
- docs/exec-plans/tech-debt-tracker.md 记录项目的技术债。
- docs/references/ 存放外部依赖、协议、框架和工具参考资料摘要。
- docs/generated/ 存放自动生成文档；默认只读，不手工维护。
- docs/PROGRESS.md 记录当前专注任务、流程记录、最近验证结果和归档索引。
- docs/DESIGN.md 记录项目的设计规则，包括 UI 设计、功能设计规范。
- docs/FRONTEND.md 记录项目的前端设计架构。
- docs/PRODUCT_SENSE.md 记录产品判断和体验原则。
- docs/QUALITY_SCORE.md 记录质量评估标准。
- docs/RELIABILITY.md 记录项目可靠性设计。
- docs/SECURITY.md 记录项目安全规范。

阅读规则
- 开始任何任务时先读 AGENTS.md。
- 涉及结构、分层、依赖、模块边界时读 ARCHITECTURE.md 和相关 design docs。
- 涉及需求范围和澄清结论时读 docs/request-clarify/。
- 涉及功能目标、用户流程、验收条件时读 docs/product-specs/。
- 涉及中大改动时先在 docs/exec-plans/active/ 创建或更新执行计划，再开始实施。
- 涉及外部依赖、框架、协议时先读 docs/references/。
- 涉及生成物时只读 docs/generated/，不手工修改 generated 下的内容。

写回规则：
- 需求澄清结果写入 docs/request-clarify/。
- 技术设计和方案决策写入 docs/design-docs/。
- 执行计划、阶段进度、决策日志写入 docs/exec-plans/active/。
- 当前专注任务、流程记录、验证记录和归档索引写入 docs/PROGRESS.md。
- 遗留问题和明确未解决事项写入 docs/exec-plans/tech-debt-tracker.md。
- 工具或脚本生成的摘要写入 docs/generated/。

模块化文档落点：
- docs/request-clarify/、docs/design-docs/、docs/exec-plans/active/ 和 docs/exec-plans/completed/ 支持按模块建立子目录。
- 模块子目录命名应与代码模块、业务域或长期维护边界一致。
- 同一需求的澄清、设计、计划文档应使用同名文件，分别落入对应文档层。

文档命名规则：
- 默认命名格式为 rNNN-request-desc.md。
- 启用模块编号时，可使用 rwNNN-request-desc.md、rbNNN-request-desc.md、rpNNN-request-desc.md 等格式。
- w、b、p 是示例子模块前缀，必须根据项目模块变化调整。
- 同一需求在 request-clarify、design-docs、exec-plans 中使用同名文件。
- 日期只允许写在文档头部，不放入文件名。

执行计划状态规则：
- docs/exec-plans/active/ 只放当前进行中或等待验收的执行计划。
- 未经用户验收的执行计划不得自动移动到 docs/exec-plans/completed/。
- 用户验收通过后，才能把对应执行计划归档到 docs/exec-plans/completed/，并更新 docs/PROGRESS.md。

### 子目录 AGENTS.md 渐进披露
- 子目录 AGENTS.md 只能补充或收紧当前目录及其子树的局部规则。
- 子目录 AGENTS.md 不能覆盖、放宽或删除根 AGENTS.md 中的全局安全规则。
- 规则冲突时，以更严格的安全规则为准。

## Git 安全规则

- commit message 必须遵守 Conventional Commits：feat、fix、docs、style、refactor、perf、test、build、ci、chore、revert。
- 除非用户提示，否则禁止使用 git commit --amend。
- git 操作必须串行执行，避免并发 git add、commit、merge、rebase、checkout、switch。
- 执行 git checkout、git switch 或任何会改变当前分支的操作前，必须先运行 git worktree list，并确认当前 worktree 归属和目标分支占用情况。

## OpenAPI 维护规则

- 新增或修改 HTTP handler 时，必须同步维护对应的 `#[utoipa::path]` 标注，声明 method、path、params、request_body、responses 和 tag。
- 新增或修改 API DTO、数据库响应模型、错误响应结构时，必须确保相关类型派生或实现 `utoipa::ToSchema`；新增 query 参数结构时必须派生或实现 `utoipa::IntoParams`。
- 新增 handler 后，必须把 handler path 和相关 schema 注册到 `src/api_doc.rs` 的 `ApiDoc`，确保 `/api-docs/openapi.json` 能动态暴露最新 API 合同。
- 修改路由 path、method、状态码或响应结构时，必须同步更新 handler 的 OpenAPI 标注和相关测试，禁止只改 Axum route 不改 OpenAPI。
- 完成 API 相关改动前，必须验证 `GET /api-docs/openapi.json` 可返回 `200 OK`，且 JSON 中包含新增或修改的 path；保留 `/swagger-ui` 作为人类调试入口。

## 后端开发的额外约束

1. 配置文件管理：除非用户额外强调，否则统一使用 `.zembra.env`（也就是当前代码仓的管理方式）
2. 笔记可见性默认口径：所有 notes 相关接口，除非用户明确要求包含归档或删除内容，否则必须只处理未删除、未归档笔记，即查询条件必须过滤 `deleted_at IS NULL` 和 `archived_at IS NULL`；该规则是严格纪律，不得作为需求澄清点反复询问。
