---
current_upstream_version: 'v0.1.3-dev'
current_upstream_commit: 'ad978652682270a593e7029269beff5411477a70'
custom_version: 'v0.1.3-custom.0'
last_merge_date: '2026-10-12'
last_release_version: ''
last_release_date: ''
vendor_branch: 'vendor/main'
upstream_remote: 'https://github.com/Sidenai/sidex.git'
---

# SideX 自定义改动登记

> **本文件是改动登记账本（纯数据）**。机制与规则（冲突策略、标记格式、类型/状态字典、frontmatter 字段职责）见 [`CUSTOMIZATIONS/README.md`](./README.md)。
>
> **两层结构（怎么用）**：
>
> - **改动总览（按文件）**——查"某个文件现在改了什么、上游合并时怎么处理"：**只读这一节**，每个文件一条，多轮演进已合并为当前状态，无需读历史轮次。
> - **变更日志（按次）**——查"某次改动何时发生、为什么、怎么验证"：按时间倒序的 append-only 流水，只增不改（历史是事实）。
>
> **AI Agent 注意**：
>
> 1. 开发前读**改动总览**定位相关文件与冲突策略（不必通读变更日志）
> 2. 完成改动后调用 `sidex-record-change` skill：**总览里已有该文件就更新那一节**（合并描述、追加演进链 id），没有就新增一节；变更日志追加一轮记录
> 3. 合并上游时按总览的冲突策略列处理
> 4. 不要删除历史（总览条目整体废弃时标 deprecated，变更日志永不删改）
> 5. 一致性自检：`sh CUSTOMIZATIONS/scripts/check-registry.sh`（代码标记 ↔ 总览表双向比对）

---

## 仓库信息

- **上游仓库**：https://github.com/Sidenai/sidex
- **当前基于上游版本**：{{current_upstream_version}}
- **当前基于上游 Commit**：{{current_upstream_commit}}
- **当前 Vendor 分支**：{{vendor_branch}}
- **最近一次上游合并**：{{last_merge_date}}
- **最近一次发布版本**：{{last_release_version}} ({{last_release_date}})

---

## 改动总览（按文件）

> **每个文件一条**：该文件当前生效的全部自定义改动（多轮演进已合并）；「标记」列为代码中 `[CUSTOM-BEGIN]` 的 change-id（即该文件自定义区的当前真相），演进链标注历轮 id。代码位置速查见 [`architecture.md`](./architecture.md) §1。
>
> **状态字典**：`active`（生效中）/ `deprecated`（已废弃）/ `merged-upstream`（上游已原生支持）。**冲突策略**字典见 README.md。

| 文件 | 标记（当前） | 演进链 | 当前效果（合并后） | 冲突策略 | 状态 |
| ---- | ------------ | ------ | ------------------ | -------- | ---- |

_（暂无——首次自定义改动登记后在此新增一行）_

---

## 变更日志（按次，时间倒序 append-only）

### CUSTOM-20261012-001 —— 初始化 custom-sidex 自定义开发仓库

- **日期**：2026-10-12
- **类型**：chore（仓库初始化）
- **改动**：
  - 基于上游 `Sidenai/sidex` `main@ad978652`（v0.1.3 开发中）建立 fork 结构：`upstream` → Sidenai/sidex，`vendor/main`（跟踪 upstream/main），`custom/main`（工作分支）
  - 引入自定义开发机制目录 `CUSTOMIZATIONS/`（README/architecture/registry/docs/scripts/src/patches/release-notes）
  - 引入跨工具共享 skills `.agents/skills/sidex-{record-change,merge-upstream,release,publish}/`
  - 新增 `AGENTS.md`（会话级硬约束 + 上游架构说明）与 `CLAUDE.md`（一行指路 `@AGENTS.md`）
- **验证**：`git branch -vv` 显示 custom/main@ad978652、vendor/main 跟踪 upstream/main；`sh CUSTOMIZATIONS/scripts/check-registry.sh` 全绿
- **备注**：本次为初始化，尚无对上游源码的改动，故「改动总览」为空
