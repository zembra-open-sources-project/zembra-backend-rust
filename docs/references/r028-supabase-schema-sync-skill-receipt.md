# r028 Supabase Schema 初始化与真实同步 Skill Receipt

## 背景

r028 的真实同步验收最终证明，Supabase 新 base 的建表动作不应该放进后端运行时代码。后端的职责是读取已经由 `zembra-schema` 定义好的本地和远端数据契约，执行真实同步并暴露同步接口；Supabase/Postgres schema 初始化属于一次性环境准备动作，应该沉淀成可复用 skill，由 skill 按已验证 CLI 能力执行，而不是让后端在业务流程里创建、修补或推断数据库结构。

## 触发场景

- 新建 Supabase project 或新 base 后，需要把 `zembra-schema` 的 Postgres contract 初始化到远端。
- 真实同步验收前，远端还没有和本地 SQLite 对齐到同一个 schema contract version。
- 后端同步前置检查发现本地和远端 schema contract 不一致，需要先完成环境初始化再继续验收。

## 边界

- 只允许从 `zembra-schema` 已发布的 Postgres contract 产物初始化远端 schema。
- 禁止在后端仓库新增、复制、维护或演化数据库 schema。
- 禁止按“缺哪张表补哪张表”的局部修补方式处理 schema 不一致，判断对象必须是完整 schema contract。
- 禁止用单元测试、mock 数据、随机数据、接口返回成功或只看行数代替真实同步验收。
- 禁止凭印象编写 Supabase CLI 命令，必须先验证当前本机安装版本支持的命令和参数。

## 已验证 Supabase CLI 路径

r028 中本机 Supabase CLI 已验证版本为 `2.107.0`，实际可用路径是先 `link` project，再通过 `db query --linked --file` 执行 schema SQL 文件。

```bash
cd /Users/yat/code/vibeProjects/ZembraProjects/zembra-backend-rust
supabase --version
supabase db --help
supabase db query --help
supabase link --help
supabase login
supabase link --project-ref <project-ref>
supabase db query --linked --file vendor/zembra-schema/postgres/001_initial_schema.sql
```

`supabase db execute --project-ref --file` 在本次本机 CLI 上不存在，不能作为 receipt 或 skill 的命令模板。

## 真实同步验收路径

schema 初始化完成后，由后端真实同步验收脚本验证本地已有数据是否严格同步到 Supabase。验收对象是九张同步表的完整行数据：`workspaces`、`devices`、`fields`、`tags`、`notes`、`note_revisions`、`note_tags`、`note_links`、`sync_changes`。

```bash
cd /Users/yat/code/vibeProjects/ZembraProjects/zembra-backend-rust
./scripts/verify_r028_real_sync.sh
```

r028 已通过的第一验收输出显示，本地和远端 `schema_contract` 都是 `0.5.0`，九张表在同步前后完整行比较全部一致，`/sync/run` 返回 `{"pulled":0,"pushed":0}`，脚本最终输出 `r028 existing-data sync verification passed by full table row comparison`。

## 可发布 Skill 形态

推荐 skill 名称为 `supabase-schema-contract-bootstrap`。它的职责是帮助用户在已有 schema contract 的项目中初始化或校验 Supabase/Postgres 远端 schema，然后交给项目自己的真实同步验收脚本验证数据同步结果。

skill 输入包括 Supabase project ref、已发布 Postgres schema SQL 路径、项目真实同步验收脚本路径和可选的本机 Supabase CLI 版本信息。skill 输出包括远端 schema 初始化结果、CLI 命令验证记录、真实同步验收命令和验收证据摘要。

## 经验教训

这次尝试的关键价值是把 schema 初始化从后端职责里剥离出来。后端不应该为了让同步跑通而拥有建表能力，完整 schema 的 source of truth 应该留在 `zembra-schema`，一次性环境动作应该由 skill 显式执行并保留证据。真实同步验收必须围绕已有本地数据和真实 Supabase 数据展开，先确认 schema contract 一致，再比较九张同步表的完整行数据，最后再验证新数据同步。
