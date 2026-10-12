# 历史坑点 / Pitfalls

> **这是什么**：本仓库（custom-sidex）开发与排查中**踩过的坑**的沉淀。新会话动手前先扫一眼标题，
> 避免重复踩坑。每条写清：现象 → 根因 → 解法 → 验证方式。
>
> **维护铁律**：每解决一个"反复折腾才定位到"的问题，就来这里加一条（一次没定位到就解决的不算坑）。
> 配套：代码位置反查见 [`../architecture.md`](../architecture.md)。

---

## 1. Windows 首次构建失败：`aws-lc-sys` 需要 NASM，报 "NASM command not found!"

- **日期**：2026-10-12（CUSTOM-20261012-001，首次 `npm run tauri dev` 时发现）
- **现象**：`npm run tauri dev` 在 Rust 编译到 ~718/943 时中断，退出码 101：
  ```
  cargo:warning=NASM command not found or failed to execute.
  thread 'main' panicked at ...\aws-lc-sys-0.41.0\builder\nasm_builder.rs:138:13:
  NASM command not found! Build cannot continue.
  ```
- **根因**：依赖链里 `aws-lc-sys`（`aws-lc-rs` 的 FFI 层，加密相关）包含汇编优化的加密例程。在 `x86_64-pc-windows-msvc` 上，构建脚本默认调用 **NASM 汇编器**把 `.asm` 编译成目标文件；本机没装 NASM → panic。注意这**不是** MSVC 缺失问题（MSVC Build Tools 已装、`rustc` host 也是 msvc）。
- **解法（二选一）**：
  1. **快速**（推荐，零安装）：构建前设环境变量 `AWS_LC_SYS_PREBUILT_NASM=1`，让该 crate 使用自带的预编译 NASM 目标文件，跳过本地汇编。
     ```bash
     AWS_LC_SYS_PREBUILT_NASM=1 npm run tauri dev
     ```
  2. **治本**：安装 NASM 并加入 PATH（`winget install nasm`，或从 nasm.us 下载），之后无需该环境变量。
- **教训**：Windows 上 Rust 构建失败先看是不是**某个 `-sys` crate 要求外部工具链**（NASM/Perl/CMake/LLVM 都常见），不要一上来怀疑 MSVC 或 Rust 版本。
- **验证**：设上环境变量后重跑，`aws-lc-sys` 顺利编过，最终 `Finished dev profile in 3m 05s` 且应用窗口弹出。

## 2. 扩展"装了但没显示"：SideX 不扫描 `~/.vscode/extensions`，且有两份不一致的扫描路径定义

- **日期**：2026-10-12（排查"Claude Code 扩展装了但对话面板不出现"时发现）
- **现象**：在 SideX 里安装 VS Code 扩展后，界面看不到其视图/面板；扩展宿主日志恒为 `discovered 0 extensions`。而 VS Code 的扩展目录里该扩展确实存在。
- **根因（两层）**：
  1. **运行时实际生效的扫描路径不含其他编辑器目录**。`src-tauri/src/commands/extension_platform.rs` 的 `extension_search_paths()` 里有一条显式注释：
     ```
     // NOTE: we intentionally do NOT scan other editors' extension dirs
     // (~/.vscode/extensions, ~/.cursor/extensions, or their app bundles).
     ```
     候选路径只有：用户扩展目录（`%APPDATA%\SideX\extensions`）、内置扩展目录、`extensions-rust/`、`dist/extensions`、`cwd/extensions`、仓库 `extensions/`。
  2. **同一仓库里另有一份 `crates/sidex-extensions/src/registry.rs` 的 `default_search_paths()`**，它**包含** `~/.vscode/extensions` 与 `~/.cursor/extensions`。两份定义不一致——排查时若读到 crate 那份会被误导，**运行时认的是 `src-tauri` 那份**。
- **解法**：
  - **想用某个 VS Code 扩展**：把它复制进 SideX 自己的扩展目录 `%APPDATA%\SideX\extensions\`（应用启动时扫描），重启应用即可。纯 JS/自带原生二进制的扩展（如 Claude Code 是 win32-x64 全量包，含 `claude.exe`）可搬运；依赖 `npm install` 的扩展不行。
  - **想改扫描逻辑**：改 `src-tauri/src/commands/extension_platform.rs` 的 `extension_search_paths()`，并在 `registry.md` 登记（它会与上游合并冲突）。
- **教训**：
  1. 排查扩展问题第一步是看扩展宿主日志的 `discovered N extensions across M search paths`——`N=0` 说明根本没被发现，与"面板渲染坏了"是两回事。
  2. 同仓库存在**同名函数的重复定义**时，先确认哪份真正被打进运行时。
  3. 面板类扩展的落点要看它的 `when` 条件：如 Claude Code 在宿主版本 ≥ 某值时把视图放进 **secondarySidebar（右侧副侧边栏）**，而非左侧活动栏——找不到面板时先看右侧。
- **验证**：复制扩展后重启，日志变为 `discovered 1 extensions`，`extension host started` 且无报错，`claude.exe` 进程随之启动。

## 3. `sidex-server binary not found` —— AI 聊天面板显示未连接，但**不影响编辑器**

- **日期**：2026-10-12（首次运行时发现）
- **现象**：启动日志出现
  ```
  [sidex-server] sidex-server binary not found. Build it with `cd sidexai/sidex-server && go build -tags fts5 -o sidex-server ./cmd/server`
  ```
  聊天面板显示为 disconnected。
- **根因**：AI agent server 是**独立的 Go 进程**（`sidexai/sidex-server`），由 `src-tauri/src/server.rs` 作为子进程拉起。本机未装 Go（需 ≥1.26），二进制没构建，故启动时找不到。
- **解法**：这是**预期行为，非缺陷**。编辑器其余功能全部正常。想启用聊天面板：
  1. 安装 Go ≥1.26
  2. `cd sidexai/sidex-server && go build -tags fts5 -o sidex-server ./cmd/server`
  3. 重启应用（或调 `server_restart`）
- **教训**：SideX 的"AI 能力"与"编辑器能力"是解耦的两块。构建/运行失败时先分清是哪一块——编辑器不需要 Go。
- **验证**：构建出二进制后重启，聊天面板可连接；未构建时其余功能（编辑、终端、Git、搜索）一切正常。
