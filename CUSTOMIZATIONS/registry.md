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
6. 总览表「文件」列**不要加反引号或任何装饰**，直接写仓库根相对路径（脚本不剥离反引号，会把 `` `path` `` 当成不存在的文件报 `FILE-NOT-FOUND`）；多文件用「、」聚合在一行

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
| src-tauri/Cargo.toml | CUSTOM-20261012-002 | 002 | `[package] version` 改为 custom 版本号 `0.1.3-custom.1`（每次发布由 `sidex-release` 递增）；workspace 根的 `[workspace.package] version` 不动 | merge-manual | active |
| package.json、src-tauri/tauri.conf.json | 无标记（JSON 不支持注释） | 002 → 004 | 版本号与 Cargo.toml 同步（三处必须一致）；`tauri.conf.json` 另加 `bundle.windows.wix.version = "0.1.3.1"`——MSI/WiX 要求纯数字版本，`-custom.N` 的 N 落在第 4 位 build 上。**004 追加**：`scripts` 里新增 `dev:custom` / `build:custom` 两个快捷命令（`cross-env AWS_LC_SYS_PREBUILT_NASM=1 npm run tauri dev|build`） | merge-manual | active |
| CUSTOMIZATIONS/scripts/（dev.bat、build.bat） | 无标记（纯自定义路径） | 004 | 快捷启动 / 打包的 bat 包装：自动切到仓库根、校验 node_modules、CRLF + `chcp 65001`；`build.bat` 会先打印四处版本号再打包并列出产物 | keep-ours | active |

---

## 变更日志（按次，时间倒序 append-only）

### CUSTOM-20261012-005 —— 新增「常用命令速查」文档并接入各入口

- **日期**：2026-10-12
- **类型**：docs
- **改动**：
  - 新增 `CUSTOMIZATIONS/docs/commands.md`：把散落在 AGENTS.md / README / skill / registry 里的命令**集中到一处**，供"忘记时直接翻"。含 8 节：最短路径（4 条）· 自定义快捷命令 · 开发/构建 · 打包/发布 · 版本号四处 · 排查/日志 · 自定义机制脚本 · AI Skills · Git 分支速记
  - 目的是消除上一轮 004 留下的问题：命令虽已写入 5 个文件，但**没有单一查阅点**，"下次忘记时查看"仍要挨个翻
  - 接入入口：`AGENTS.md`（必读文件新增第 3 条 + 项目结构速查 + 文档更新职责表新增一行）、`CUSTOMIZATIONS/README.md`（目录结构树 + 核心约定）
  - 同时在 `AGENTS.md` 文档更新职责表加了治理钩子：**新增/修改常用命令 → 同步 commands.md**，避免速查表随命令演进而腐化
- **验证**：文件内所有命令/路径均取自本会话实测结论（`dev:custom`/`build:custom` 已跑通、日志路径已实测存在、产物路径与四处版本号已实测）；`sh CUSTOMIZATIONS/scripts/check-registry.sh` 全绿
- **基于上游版本**：main@ad978652（v0.1.3-dev）
- **备注**：纯自定义路径（keep-ours），总览表不单列条目（与 `docs/pitfalls.md` 的处理一致）

### CUSTOM-20261012-004 —— 新增快捷启动 / 打包命令（npm 脚本 + bat）

- **日期**：2026-10-12
- **类型**：chore（开发体验）
- **改动**：
  - `package.json`：新增 `dev:custom`、`build:custom` 两个脚本，内容为 `cross-env AWS_LC_SYS_PREBUILT_NASM=1 npm run tauri dev|build`
    - 用 `cross-env`（已是依赖）而非 `VAR=1 cmd` 前置语法——后者只在 Git Bash 生效，在 cmd/PowerShell 里无效
    - 带上 `AWS_LC_SYS_PREBUILT_NASM=1`：本机**未装 NASM**，不带则 `aws-lc-sys` 编译 panic（pitfalls #1）
  - `CUSTOMIZATIONS/scripts/dev.bat`、`build.bat`：bat 包装，自动 `cd` 到仓库根并调用上面的 npm 脚本（单一事实来源，bat 不重复写构建逻辑）
    - `build.bat` 打包前先打印**四处版本号**（package.json / tauri.conf.json version / wix.version / Cargo.toml）供核对，打包后列出 `target/release/bundle/` 产物——针对 002 那次"漏改 wix.version + 产物不在 src-tauri/target"的复发预防
    - 两个 bat 均为 **CRLF + 无 BOM UTF-8 + `chcp 65001`**（cmd.exe 对 LF-only 的多行 `if (...)` 块不可靠；中文需切码页才不乱码）
- **验证**：
  - `npm run dev:custom -- --version` → 链路正常执行（`--version` 被内层 npm 消费，未报错）
  - `cmd /c CUSTOMIZATIONS\scripts\build.bat` → 中文正常渲染、四处版本号打印正确（`0.1.3-custom.1` / `0.1.3-custom.1` / `0.1.3.1` / Cargo `0.1.3-custom.1`），并推进到 `tauri build` → vite 构建
  - `cmd /c CUSTOMIZATIONS\scripts\dev.bat` → 横幅与日志路径正确，推进到 vite dev server（`http://localhost:1420/`）+ `tauri dev`
  - 两次测试均在超时后掐断，已确认无残留 node/cargo 进程、1420 端口已释放
- **基于上游版本**：main@ad978652（v0.1.3-dev）
- **备注**：`package.json` 是上游文件但 JSON 无法加注释，故总览标「无标记」；bat 是纯自定义路径（keep-ours）

### CUSTOM-20261012-003 —— 排查 Claude Code 扩展「Windows 构建找不到」并沉淀坑点

- **日期**：2026-10-12
- **类型**：docs（排查记录，无代码改动）
- **改动**：`CUSTOMIZATIONS/docs/pitfalls.md` 新增 #5：扩展目录内同 id 同版本两个包并存时，`scan_extensions()` 的 `is_version_greater` 相等即不替换 → **先扫到的赢**（NTFS 按字母序），装错平台的 Linux-arm64 包会顶掉正确的 win32-x64 包，导致 Claude Code 面板报 "the Windows (win32-x64) build could not be found"
- **验证**：错包移出 `%APPDATA%\SideX\extensions` 后，正确包自带 `claude.exe --version` → `2.1.296 (Claude Code)`；启动 SideX 后扩展宿主子进程正常起来，无加载报错
- **基于上游版本**：main@ad978652（v0.1.3-dev）
- **备注**：本仓库后端无 VSIX 安装逻辑，此类包属外部放入；上游文件未改动，故总览表无变更

### CUSTOM-20261012-002 —— 首次本地打包（v0.1.3-custom.1）

- **日期**：2026-10-12
- **类型**：config（版本号 + 打包配置）
- **改动**：
  - 应用版本号 `0.1.3` → `0.1.3-custom.1`，三处同步：`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`（最后者带 `[CUSTOM-BEGIN]` 标记）
  - `src-tauri/tauri.conf.json` 的 `bundle.windows` 增加 `wix.version = "0.1.3.1"`：`npx tauri build` 首次在 MSI 阶段失败——WiX 的 ProductVersion 只接受纯数字，`-custom.1` 里的 `custom` 不是数字。用该字段单独覆盖 MSI 内部版本（`custom.N` 的 N 放第 4 位 build），应用显示版本与产物文件名仍保留 `-custom.1`
  - 版本号未带 `v` 前缀写进三个文件（格式 `0.1.3-custom.1`）；tag 才带 `v`
- **验证**：
  - `AWS_LC_SYS_PREBUILT_NASM=1 npx tauri build` → `Finished 2 bundles`，产出 msi 18MB / nsis 13MB
  - `SideX.exe` 资源版本 `FileVersion/ProductVersion = 0.1.3-custom.1`（`Get-Item .VersionInfo`）
  - 冒烟：运行 `target/release/SideX.exe`，进程存活且枚举到可见窗口 `SideX`（标题非空），随后终止
- **基于上游版本**：main@ad978652（v0.1.3-dev）
- **备注**：registry frontmatter 的 `custom_version` 按职责表留待 `sidex-publish` 发布时更新；**MSI 的 `wix.version` 每次发新 custom 序号时必须同步递增**（N → `0.1.3.N`），否则 MSI 升级安装会认为版本未变

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
