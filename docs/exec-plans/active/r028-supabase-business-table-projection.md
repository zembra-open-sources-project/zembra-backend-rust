# r028 Supabase 业务表投影同步

日期：2026-06-19

需求澄清文档：`docs/request-clarify/r028-supabase-business-table-projection.md`
设计文档：`docs/design-docs/r028-supabase-business-table-projection.md`

## 关联设计文档

`docs/design-docs/r028-supabase-business-table-projection.md`

## Stage #1: 契约核对和投影模型

### 任务 #1: 核对 vendor schema 与 payload 覆盖

**Status:** Finished

**Files:**
- Verify: `vendor/zembra-schema/postgres/001_initial_schema.sql`
- Verify: `src/repositories/sync/payload.rs`
- Verify: `src/repositories/notes/payloads.rs`
- Verify: `src/repositories/taxonomy.rs`
- Verify: `src/repositories/notes/`

- 功能：确认 r028 所需远端业务表和字段都已存在于 `vendor/zembra-schema`，并确认本地 change payload 足够构造远端投影行。
- 实现说明：逐一核对 `fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links` 的字段、主键和关系删除条件；如果发现缺失契约，停止本仓实现并记录需要先在 `zembra-schema` 处理的字段或表。
- 预期验证结果：形成明确的实现输入清单；本仓不新增 schema、migration、DDL 或字段推断实现。
- 执行记录：2026-06-19 已核对 `vendor/zembra-schema/postgres/001_initial_schema.sql`，r028 所需 6 张业务表和投影字段均存在；本次未新增 schema、migration 或 DDL。

### 任务 #2: 定义业务投影请求模型

**Status:** Finished

**Files:**
- Modify: `src/sync/supabase.rs`
- Create: `src/sync/supabase/projection.rs`
- Modify: `src/repositories/sync/payload.rs`
- Modify: `src/repositories/sync/mod.rs`

- 功能：为业务表 upsert/delete 构造稳定的内部请求模型。
- 实现说明：基于当前 payload parser 补齐投影所需解析能力；为每个业务表定义只用于 Supabase JSON 序列化的 record 类型，所有类型成员保持文档注释。
- 预期验证结果：每类 change 都能被转换为明确的投影动作；未知 entity/operation 或 payload 缺字段会返回错误。
- 执行记录：2026-06-19 已将 sync payload parser 暴露为 crate 内部模块，并在 `projection.rs` 中定义业务投影 record 和 payload error。

## Stage #2: Supabase 业务表投影客户端

### 任务 #3: 实现业务表 upsert 请求构造

**Status:** Finished

**Files:**
- Modify: `src/sync/supabase.rs`
- Create: `src/sync/supabase/projection.rs`
- Create: `src/sync/supabase/projection/tests.rs`

- 功能：支持 `fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links` 的远端 upsert 请求构造。
- 实现说明：新增 `build_upsert_*_request` 或等价私有 helper；使用当前 Supabase client auth header；upsert body 中补齐 `workspace_id`；使用 `Prefer: resolution=merge-duplicates` 保证幂等。
- 预期验证结果：单元测试能断言各 upsert 请求的 URL、header 和 body。
- 执行记录：2026-06-19 已新增业务表 upsert 请求构造，并用投影单元测试覆盖 field、tag、note、note revision、note link upsert body。

### 任务 #4: 实现关系 detach 删除请求构造

**Status:** Finished

**Files:**
- Modify: `src/sync/supabase.rs`
- Create: `src/sync/supabase/projection.rs`
- Create: `src/sync/supabase/projection/tests.rs`

- 功能：支持 `note_tag detach` 和 `note_link detach` 的远端关系行删除。
- 实现说明：为 `note_tags` 使用 `workspace_id`、`note_id`、`tag_id` filters；为 `note_links` 使用 `workspace_id` 和 link `id` filters；DELETE 成功状态使用当前 Supabase response 校验逻辑。
- 预期验证结果：单元测试能断言 DELETE URL query filters 正确，重复删除可作为幂等重试路径。
- 执行记录：2026-06-19 已新增关系删除请求构造，并用投影单元测试覆盖 `note_tag detach` 和 `note_link detach` filters。

### 任务 #5: 实现批量投影执行入口

**Status:** Finished

**Files:**
- Modify: `src/sync/supabase.rs`
- Create: `src/sync/supabase/projection.rs`

- 功能：提供 `project_business_tables` 或同等入口，按 pending changes 顺序执行业务表投影。
- 实现说明：对每条 change 解析 payload 并派发到对应 upsert/delete；遇到任一失败立即返回错误；日志记录 change id、entity type、operation 和表名。
- 预期验证结果：业务投影成功时返回 `Ok(())`；失败时保留错误，调用方不会继续写远端 `sync_changes`。
- 执行记录：2026-06-19 已新增 `project_business_tables` 和 `build_business_projection_requests`，投影请求按 pending changes 顺序构造并执行。

## Stage #3: Push 编排接入

### 任务 #6: 在 push 中接入投影步骤

**Status:** Finished

**Files:**
- Modify: `src/services/sync.rs`

- 功能：将 push 顺序调整为 ensure identity、业务表投影、`sync_changes` upsert、mark success。
- 实现说明：pending changes 非空时先确保默认 workspace/device，再调用业务投影入口；投影成功后才调用 `upsert_sync_changes`；失败时写入 push error，成功时调用 `mark_push_success`。
- 预期验证结果：业务投影失败时 push 返回错误，本地 push cursor 和 `supabase_committed_at` 不推进。
- 执行记录：2026-06-19 已在 `SyncService::push` 中接入业务投影 preflight、identity upsert、业务表投影、`sync_changes` upsert、mark success 顺序。

### 任务 #7: 补齐 sync service 失败语义测试

**Status:** Finished

**Files:**
- Modify: `src/services/sync.rs`

- 功能：用自动化测试锁定投影失败时不提交远端 change 的语义。
- 实现说明：使用可控 Supabase 响应或测试 client 注入方式模拟业务表投影失败；断言 `sync_state(scope='push')` 未推进，pending change 的 `supabase_committed_at` 仍为空。
- 预期验证结果：测试能稳定复现失败路径，并在实现正确时通过。
- 执行记录：2026-06-19 已新增 `push_projection_payload_error_does_not_advance_cursor`，用 payload 投影失败覆盖不推进 cursor 和不写 `supabase_committed_at`。

## Stage #4: 验证、文档回写和提交

### 任务 #8: 运行自动化和手工验证

**Status:** Finished

**Files:**
- Verify: `src/sync/supabase.rs`
- Verify: `src/services/sync.rs`
- Verify: `src/repositories/sync/payload.rs`
- Verify: `tests/`
- Verify: `docs/`

- 功能：确认业务表投影功能、失败语义和既有同步能力都符合设计。
- 实现说明：运行格式、编译、定向 sync/notes 测试和 schema 边界检查；如可连接 Supabase 测试环境，手工执行 `/sync/push` 并检查业务表数据。
- 预期验证结果：`cargo fmt --check`、`cargo check`、`cargo test sync`、`cargo test notes` 通过；schema 边界检查未发现本仓新增 schema；手工 Supabase 控制台能看到业务表投影数据。
- 验证记录：2026-06-19 已通过 `cargo test supabase::projection`、`cargo test push_projection_payload_error_does_not_advance_cursor`、`cargo fmt --check`、`cargo test sync`、`cargo test notes`、`cargo check`、`cargo clippy -- -D warnings`。当前 diff 只包含 r028 代码和计划文档，未新增 schema 或 migration 文件；未执行真实 Supabase 控制台手工验证。

### 任务 #9: 更新计划状态并创建原子提交

**Status:** Finished

**Files:**
- Modify: `docs/exec-plans/active/r028-supabase-business-table-projection.md`
- Verify: repository

- 功能：按实际执行结果更新任务状态和验证记录，并完成提交。
- 实现说明：每个 Stage 完成后更新任务状态；提交前检查 `git status --porcelain` 和 `git diff --stat`；commit message 使用 Conventional Commits。
- 预期验证结果：工作区只包含 r028 范围内改动；提交信息满足项目规范；未经用户验收不移动到 `docs/exec-plans/completed/`。
- 执行记录：2026-06-19 提交前检查确认工作区改动只包含 r028 业务表投影实现、测试和 active plan 更新；未经用户验收未归档 completed。
