# 代码链路图谱 · custom-sidex

> **这是什么**：面向 AI Agent 的**加速索引**——一页纸回答"改某个功能该动哪些文件"。图谱过期时以代码为准，并顺手订正本文件。
> **配套**：改动登记见 [`registry.md`](./registry.md)；机制规则见 [`README.md`](./README.md)；坑点见 [`docs/pitfalls.md`](./docs/pitfalls.md)。

---

## 0. 一句话架构

SideX 是 **VSCode 的 Tauri 移植版**：前端是直接移植的 VSCode TypeScript 工作台（Monaco 编辑器 + workbench + 扩展宿主 API），后端用 Rust（Tauri 2）替换掉了 Electron 主进程与 Node 原生模块（fs/pty/git/搜索/SQLite），并额外带一个 **Go 写的本地 AI agent server**（`127.0.0.1` 回环，随应用启动/退出）。

```
┌──────────────────────────────────────────────────────────────┐
│ 前端（Webview 内）  src/vs/  —— VSCode 工作台移植（TypeScript）│
│   base / platform / editor(Monaco) / workbench               │
│   ↑ Tauri invoke()/event  ⇅  HTTP/WS (loopback)              │
├──────────────────────────────────────────────────────────────┤
│ Rust 后端  src-tauri/src/commands/*  +  crates/sidex-*       │
│   fs · terminal(portable-pty) · git · search · sqlite · LSP  │
│   DAP · 扩展宿主(sidecar) · providers(模型凭据)              │
├──────────────────────────────────────────────────────────────┤
│ Go agent server  sidexai/sidex-server/                       │
│   chat · tools · MCP · memory · context/index                │
└──────────────────────────────────────────────────────────────┘
```

**四个语言层**——改代码前先判断属于哪层，别串层找：

| 层         | 目录                              | 语言       | 何时动它                                   |
| ---------- | --------------------------------- | ---------- | ------------------------------------------ |
| 前端工作台 | `src/vs/`                         | TypeScript | UI、编辑器行为、命令、视图、扩展 API       |
| 原生后端   | `src-tauri/src/` + `crates/`      | Rust       | 文件/终端/Git/搜索/存储/系统集成/模型凭据  |
| AI agent   | `sidexai/sidex-server/`           | Go         | 聊天、工具调用、MCP、记忆、上下文索引      |
| WASM 扩展  | `src-wasm/`、`extensions-rust/`   | Rust→WASM  | 高性能扩展与扩展 SDK                       |

---

## 0.5 任务作用域路由（先定范围，避免污染上下文）

> **原则**：先按任务类型锁定**最小文件集**，只读这些；不要把整个仓库喂进上下文。

| 任务类型                  | 只读这些                                                                 | 不要读                             |
| ------------------------- | ------------------------------------------------------------------------ | ---------------------------------- |
| 改编辑器行为 / UI / 命令  | `src/vs/workbench/contrib/<feature>/`、`src/vs/workbench/services/`      | Rust 后端、Go server               |
| 改平台服务（DI/配置/存储）| `src/vs/platform/<service>/`、`src/vs/workbench/services/`               | Monaco 内部、crates                |
| 改 Monaco 编辑器内核      | `src/vs/editor/`                                                         | workbench、后端                   |
| 改原生能力（fs/pty/git）  | `src-tauri/src/commands/<mod>.rs` + 对应 `crates/sidex-*`                | 前端（除非要接通 IPC）             |
| 改 AI agent 行为          | `sidexai/sidex-server/internal/{ai,tools,agent}/`                         | 前端工作台、多数 Rust              |
| 改模型 provider / 凭据    | `src-tauri/src/commands/providers.rs`、`sidex-server/internal/ai/`        | UI 之外的层                        |
| 改扩展宿主                | `src-tauri/src/commands/{ext_host,extension_platform}.rs`、`src/vs/workbench/api/` | Monaco、Go server          |
| 打包 / 发布               | `package.json`、`vite.config.ts`、`src-tauri/tauri.conf.json`、`scripts/` | 业务代码                           |

---

## 1. 文件职责速查

### 前端（`src/`，移植自 VSCode）

| 路径                      | 职责                                                        |
| ------------------------- | ----------------------------------------------------------- |
| `src/main.ts`             | 应用入口；注入 `__SIDEX_SERVICES__`（editor/syntax/git/search/... 桥接） |
| `src/vs/base/`            | 基础工具（纯 TS，无 DOM/无平台依赖）                        |
| `src/vs/platform/`        | 平台服务 + DI（`@inject` 装饰器）、configuration、storage    |
| `src/vs/platform/sidex/`  | **SideX 自有平台服务**（`sidexServices.ts` 桥接后端）← 自定义常落脚处 |
| `src/vs/editor/`          | Monaco 编辑器内核                                           |
| `src/vs/workbench/`       | IDE 外壳：`browser/`(parts/layout)、`contrib/`(功能)、`services/`、`api/`(扩展宿主 API) |
| `src/vs/sidex-bridge.ts`  | 前端 ↔ Rust 后端桥接                                        |

### Rust 后端（`src-tauri/` + `crates/`）

| 路径                              | 职责                                                       |
| --------------------------------- | ---------------------------------------------------------- |
| `src-tauri/src/main.rs`           | 入口                                                       |
| `src-tauri/src/lib.rs`            | **App 装配 + 命令注册**（新增 Rust 命令必在此注册）← 关键  |
| `src-tauri/src/server.rs`         | 监管 Go agent server 子进程（spawn/重启/退出清理）         |
| `src-tauri/src/commands/`         | 52 个命令模块（见下表）                                    |
| `crates/sidex-*/`                 | 21 个支撑 crate（agent/context/git/lsp/dap/terminal/...）  |

**`src-tauri/src/commands/` 高频模块**：

| 模块                                                             | 职责                                       |
| ---------------------------------------------------------------- | ------------------------------------------ |
| `fs.rs` `path.rs` `watch.rs`                                     | 文件读写、路径、文件监视                    |
| `terminal.rs` `sidex_terminal.rs` `process.rs` `tasks.rs`        | 终端 PTY、进程、任务                       |
| `git.rs`                                                         | Git 全套（status/diff/commit/branch/...）  |
| `search.rs` `index.rs` `text.rs` `syntax.rs` `textmate.rs`       | 搜索、索引、语法高亮                       |
| `db_state.rs` `storage.rs` `settings.rs` `profiles.rs`           | SQLite 存储、设置、配置档案                |
| `providers.rs` `auth.rs` `secrets.rs` `crypto.rs`                | 模型 provider 与凭据解析、密钥             |
| `agent.rs` `mcp.rs` `hooks.rs` `next_gen_tools.rs` `orchestrate.rs` | 本地 agent 工具执行、MCP、钩子          |
| `ext_host.rs` `extension_platform.rs` `extension_wasm.rs` `extensions.rs` `ext_api.rs` | 扩展宿主与扩展平台 |
| `lsp.rs` `debug.rs` `remote.rs` `browser.rs` `updater.rs` `menu.rs` `window.rs` `theme.rs` | LSP/DAP/远程/浏览器/更新/菜单/窗口/主题 |

> **`crates/sidex-extensions/src/registry.rs`** 定义扩展搜索路径（`default_search_paths()`）；**`src-tauri/src/commands/extension_platform.rs` 的 `extension_search_paths()` 才是运行时实际生效的那份**——注意二者不一致，改扩展发现逻辑认后者（见 `docs/pitfalls.md`）。

### AI agent server（`sidexai/`）

| 路径                                        | 职责                                             |
| ------------------------------------------- | ------------------------------------------------ |
| `sidexai/sidex-server/cmd/server/`          | Go 服务入口                                      |
| `sidexai/sidex-server/internal/ai/`         | 模型调用（`anthropic.go` 走 `/v1/messages`，其余走 OpenAI 兼容 `/chat/completions`） |
| `sidexai/sidex-server/internal/tools/`      | 工具实现                                         |
| `sidexai/sidex-server/internal/agent/`      | agent 循环                                       |
| `internal/{context,index,memory,memdir,skills,session,plan,prompt,mcp}/` | 上下文/索引/记忆/技能/会话/计划/提示/MCP |
| `sidexai/sidex-agent/` `sidexai/sidex-cli/` | agent 相关子项目与 CLI                           |

### 其他

| 路径                          | 职责                                    |
| ----------------------------- | --------------------------------------- |
| `CUSTOMIZATIONS/src/`         | **自定义前端模块**（独立文件 + 从入口挂载，冲突策略恒 `keep-ours`）；现有模块与挂载点见 `src/README.md` |
| `src-wasm/`                   | WASM 模块（hash/scroll/tfidf）          |
| `extensions-rust/`            | Rust 扩展                               |
| `sidex-extension-sdk/`        | 扩展 SDK                                |
| `scripts/`                    | `generate-extension-meta.js`（生成扩展元数据）、`setup-extensions.sh`、`postbuild.js`、`vite-plugin-nls.ts` |
| `infrastructure/marketplace-proxy/` | 市场代理                          |
| `vite.config.ts` `index.html` | 前端构建配置与入口（`index.html` 声明 `_VSCODE_PACKAGE_JSON` 版本 1.110.0） |
| `src-tauri/tauri.conf.json`   | Tauri 应用配置（版本、窗口、打包）      |

---

## 2. 任务 → 代码位置反查

### 2.1 编辑器 / Monaco

| 想改什么            | 去哪                                                      |
| ------------------- | --------------------------------------------------------- |
| 语法高亮 / 主题     | `src/vs/editor/`、`src/vs/platform/theme/`、`commands/theme.rs`、`crates/sidex-theme` |
| 编辑器命令 / 快捷键 | `src/vs/platform/commands/`、`src/vs/platform/keybinding/`、`crates/sidex-keymap` |
| 文本模型 / 编码     | `crates/sidex-text`、`commands/encoding.rs`               |

### 2.2 文件与工作区

| 想改什么          | 去哪                                                        |
| ----------------- | ----------------------------------------------------------- |
| 文件读写 / 资源管理器 | `commands/fs.rs`、`src/vs/platform/files/`                 |
| 文件监视          | `commands/watch.rs`（Rust `notify`）                        |
| 路径处理          | `commands/path.rs`、`crates/sidex-workspace`                |
| 搜索（文件/全文） | `commands/search.rs`、`commands/index.rs`（并行 Rust 实现） |

### 2.3 终端

| 想改什么        | 去哪                                                          |
| --------------- | ------------------------------------------------------------- |
| 终端 PTY / 外壳 | `commands/terminal.rs`、`crates/sidex-terminal`（`portable-pty`） |
| 终端 UI         | xterm.js（前端），`src/vs/workbench/contrib/terminal/`         |

### 2.4 Git

| 想改什么    | 去哪                                              |
| ----------- | ------------------------------------------------- |
| Git 全部操作 | `commands/git.rs`、`crates/sidex-git`            |
| Git UI      | `src/vs/workbench/contrib/scm/`（VSCode 原路径）  |

### 2.5 存储 / 设置 / 配置

| 想改什么      | 去哪                                                            |
| ------------- | --------------------------------------------------------------- |
| SQLite 存储   | `commands/db_state.rs`、`commands/storage.rs`、`crates/sidex-db` |
| 设置与配置    | `commands/settings.rs`、`crates/sidex-settings`、`src/vs/platform/configuration/` |
| 配置档案      | `commands/profiles.rs`、`crates/sidex-profiles`                  |

### 2.6 AI / 模型 provider

| 想改什么                | 去哪                                                            |
| ----------------------- | --------------------------------------------------------------- |
| 模型 provider 与凭据解析 | `commands/providers.rs`（解析顺序：设置 → 环境变量 → CLI 登录 → 本地服务） |
| 凭据加密 / 密钥         | `commands/secrets.rs`、`commands/crypto.rs`                      |
| 聊天后端（Go）          | `sidexai/sidex-server/internal/ai/`、`internal/agent/`           |
| 工具执行（本地）        | `commands/agent.rs`、`crates/sidex-agent`                        |
| MCP 客户端              | `commands/mcp.rs`、`sidex-server/internal/mcp/`                  |
| 上下文 / 索引 / 记忆    | `crates/sidex-context`、`sidex-server/internal/{context,index,memory}/` |
| agent server 生命周期   | `src-tauri/src/server.rs`、`commands/server`（`server_endpoint`/`server_restart`） |

### 2.7 扩展

| 想改什么        | 去哪                                                              |
| --------------- | ----------------------------------------------------------------- |
| 扩展发现 / 扫描 | `commands/extension_platform.rs`（`extension_search_paths()`、`scan_extensions()`） |
| 扩展宿主进程    | `commands/ext_host.rs`（spawn Node sidecar）                       |
| 扩展 API（前端）| `src/vs/workbench/api/`（`extHost*.ts`、`mainThread*.ts`）         |
| WASM 扩展       | `commands/extension_wasm.rs`、`src-wasm/`、`extensions-rust/`      |
| 扩展注册表 crate| `crates/sidex-extensions`                                          |
| **从 VSIX 安装（自定义）** | `CUSTOMIZATIONS/src/vsix-install/vsixInstall.contribution.ts`（挂载于 `src/main.ts`）；本地 `.code-workspace` 拖放也从此 Tauri 监听入口进入，路由至 `src/main.ts` 的 workspace URL/provider。Rust 侧复用 `commands/extensions.rs` 的 `install_extension`。链路见 §3 #1/#2 |

### 2.8 LSP / 调试 / 远程

| 想改什么    | 去哪                                                    |
| ----------- | ------------------------------------------------------- |
| LSP         | `commands/lsp.rs`、`crates/sidex-lsp`                    |
| 调试（DAP） | `commands/debug.rs`、`crates/sidex-dap`                  |
| 远程开发    | `commands/remote.rs`、`crates/sidex-remote`              |
| 浏览器自动化| `commands/browser.rs`                                    |

### 2.9 打包 / 发布

| 想改什么      | 去哪                                                          |
| ------------- | ------------------------------------------------------------- |
| 前端构建      | `vite.config.ts`、`package.json`（`build`）、`scripts/postbuild.js` |
| Tauri 打包    | `src-tauri/tauri.conf.json`、`Cargo.toml`、`crates/sidex-update` |
| 扩展元数据生成| `scripts/generate-extension-meta.js`、`scripts/setup-extensions.sh` |
| 应用内更新    | `commands/updater.rs`、`crates/sidex-update`                   |
| **快捷启动 / 打包（自定义）** | `package.json` 的 `dev:custom` / `build:custom`、`CUSTOMIZATIONS/scripts/（dev.bat、build.bat）`；版本号四处见 registry `CUSTOM-20261012-004` |

---

## 3. 重点链路详解

> 自定义改动形成的链路在此逐条记录；每条对应 `registry.md` 里的一个 change-id。

### #2 拖拽打开 `.code-workspace`（CUSTOM-20261013-004）

> Tauri `dragDropEnabled: true` 会接管 OS 拖放，HTML5 drop handler 不会收到文件。复用已有的 VSIX Tauri 监听器分流 `.vsix` 与 `.code-workspace`；工作区打开最终通过 `IHostService` → `BrowserHostService` → SideX `workspaceProvider`，避免把工作区文件当普通编辑器文件打开。

```
Tauri onDragDropEvent(drop)
  ├─ .vsix → sidex.extensions.installFromVSIX（原有安装链路，CUSTOM-20261013-001）
  └─ .code-workspace → IHostService.openWindow([{ workspaceUri }])
       └─ BrowserHostService.doOpen → workspaceProvider.open
            └─ URL ?workspace=<URI> → window reload
                 └─ main.ts 初始化 { workspaceUri }
                      └─ web.main.ts resolveWorkspace → getWorkspaceIdentifier
```

启动及切换工作区时用 `workspace` 参数，单文件夹继续兼容旧的 `folder` 参数；provider 导航会删除另一类型的旧参数，避免重载后读到冲突目标。混合多路径拖放中，只打开首个 `.code-workspace`，VSIX 仍逐个安装；普通文件 / 文件夹拖放语义不由本改动扩展。

---

### #1 从本地 VSIX 安装扩展（CUSTOM-20261013-001）

> 背景：SideX 前端是 **Web 模式**（`isWeb === true`），`localExtensionManagementServer` 恒为 `null`，上游 VSIX 链路（`getManifest` / `installVSIX` / `installFromLocation` 的 `file://` 分支）全部走不通，菜单也被 `when` 条件隐藏。详见 `docs/pitfalls.md` #7。

```
用户入口（命令面板 / 扩展视图「...」菜单 / 资源管理器右键 .vsix / 拖拽 .vsix）
  └─ 命令 sidex.extensions.installFromVSIX     [CUSTOMIZATIONS/src/vsix-install/vsixInstall.contribution.ts]
       ├─ 无参数时：IFileDialogService.showOpenDialog（已桥接 Tauri dialog 插件，原生文件框 + .vsix 过滤）
       └─ Tauri invoke('install_extension', { vsixPath })      [src-tauri/src/commands/extensions.rs:64]
            └─ crates/sidex-extensions/src/vsix.rs 解压 → %APPDATA%\SideX\extensions\<publisher>.<name>
       └─ IExtensionManagementServerService.webExtensionManagementServer
            .extensionManagementService.installFromLocation(URI.file(安装目录), currentProfile.extensionsResource)
              ├─ 写扩展视图的已安装记录（WebExtensionsScannerService，IndexedDB；location 指向磁盘目录）
              └─ 触发 onDidInstallExtensions（经 workbench 服务的 multiplexer 冒泡）
                   └─ tauriExtensionHost.contribution.ts 既有热加载：loadExtension + activateExtension + 注册 contributes.commands
卸载（沿用上游链路，无需自定义）：
  扩展视图卸载 → onDidUninstallExtension → invoke('uninstall_extension') → 删除磁盘目录
拖拽：
  Tauri getCurrentWebviewWindow().onDragDropEvent → 筛 .vsix → executeCommand(同上)
  （`tauri.conf.json` 的 dragDropEnabled: true 会吞掉 HTML5 drop，故不能走 extensionsViewlet 的 DropObserver）
```

**关键取舍**：不实现本地扩展管理 server（那要动扩展管理核心，回归风险大），而是直接调 web server 的 `installFromLocation` —— 绕开 `workbench ExtensionManagementService.installFromLocation` 对 `file://` 的 `'Local extension management server is not found'` 抛出，同时复用全部既有事件/热加载/卸载链路。

**配套上游修复（CUSTOM-20261013-002）**：`installFromLocation` 内部会 `scanExtensionManifest(安装目录)` → `getExtensionManifest()` → `extensionResourceLoaderService.readExtensionResource(joinPath(dir,'package.json'))`。该方法原先第一句就调 `FileAccess.uriToBrowserUri()`，而 Tauri 下 `file://` 会被改写成 `sidex-asset://localhost/<encodeURIComponent(fsPath)>`（`base/common/network.ts`），`IFileService` 又没有该 scheme 的 provider（`sidex-asset` 只是 `src-tauri/src/lib.rs` 的 Tauri 自定义协议）→ 读清单必失败，报 `Cannot find a valid extension from the location file:///...`。故在该方法开头加了 Tauri + `file://` 的前置分支直接走 `IFileService`。**凡是"通过扩展资源加载服务读磁盘文件"的路径都受益/都依赖它。**

---

## 4. 数据契约铁律

1. **新增 Rust 命令必须两步**：在 `src-tauri/src/commands/<mod>.rs` 定义 `#[tauri::command]`，并在 **`src-tauri/src/lib.rs` 的 `invoke_handler` 中注册**——只定义不注册，前端调用会失败。
2. **前端 ↔ 后端类型要对齐**：Rust 侧用 `#[serde(rename_all = "camelCase")]` 时，TS 侧字段名必须与之一致。
3. **TS 导入用 `.js` 后缀**（ESM 约定，虽是 `.ts` 源文件）；服务用 `@inject` 装饰器注入。
4. **模型凭据只走进程环境变量**：`SIDEX_PROVIDER_<PROVIDER>_KEY`/`_BASE_URL`/`_AUTH`，绝不写入配置文件、绝不发给 webview。
5. **agent server 只绑回环**：非 `127.0.0.1` 需显式 `SIDEX_ALLOW_UNAUTHENTICATED=1` 才启动——它代用户执行 shell 命令，是安全边界。
6. **`target/` 永不入库**：Rust 构建产物体积巨大，`.gitignore` 已含 `**/target/`。
