# r008 数据库 schema 升级到 v0.3.0

日期：2026-05-04

## 需求理解

将后端使用的共享数据库 schema 从 `v0.2.0` 升级到 `v0.3.0`，继续以 `vendor/zembra-schema` submodule 作为唯一 schema 契约来源。

## 已确认范围

| 项目 | 结论 |
| --- | --- |
| schema 来源 | `vendor/zembra-schema` |
| 目标版本 | `v0.3.0` |
| 主要变化 | 新增 workspace 维度、同步相关表、业务表同步元数据 |
| 后端兼容策略 | 现有 API 继续使用 shared schema 提供的默认 workspace |

## 验收标准

- `vendor/zembra-schema` 固定到 tag `v0.3.0`。
- 后端启动 migration 可把新库或旧库升级到 `0.3.0`。
- 现有 notes 和 taxonomy API 在 `v0.3.0` schema 下保持可用。
- Rust 格式化、编译和测试通过。
