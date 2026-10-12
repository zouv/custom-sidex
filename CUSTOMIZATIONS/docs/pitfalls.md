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

## 4. `tauri build` 出包失败：MSI 拒绝 `-custom.N` 版本号；且产物根本不在 `src-tauri/target/`

- **日期**：2026-10-12（首次打包 v0.1.3-custom.1 时发现，CUSTOM-20261012-002）
- **现象**：Rust 全部编译成功（`Finished release profile in 12m 34s`、`Built application at: ...\SideX.exe`），却在最后一步出包时终止：
  ```
  Info Patching ...\SideX.exe with bundle type information: msi
  Info Verifying wix package
  Downloading https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314-binaries.zip
  failed to bundle project `optional pre-release identifier in app version must be numeric-only and cannot be greater than 65535 for msi target`
  ```
  **一个安装包都没产出**——MSI 先失败，后面的 NSIS 根本没轮到。
- **根因**：WiX 的 MSI `ProductVersion` 只接受 `major.minor.patch.build` 纯数字（major/minor ≤255，patch/build ≤65535），而自定义版本号 `0.1.3-custom.1` 里的 `custom` 是字母。上游能编过是因为它的 tauri 版本是干净的 `0.1.3`。
- **解法**：保留应用版本 `0.1.3-custom.1`（产物名 / exe 资源 / 关于页都用它），另在 `src-tauri/tauri.conf.json` 用 `bundle.windows.wix.version` 单独给 MSI 一个纯数字版本，把 custom 序号塞进第 4 位：
  ```json
  "windows": {
    "webviewInstallMode": { "type": "downloadBootstrapper" },
    "wix": { "version": "0.1.3.1" }
  }
  ```
  **注意**：该字段是手写常量，不会随 `version` 自动变——**每发一个 custom 序号都要同步改**（custom.2 → `0.1.3.2`），否则 Windows 认为 MSI 版本没变、升级安装会跳过或报重复安装。
- **顺带纠正（不是坑但是弯路）**：产物**不在** `src-tauri/target/release/bundle/`，而在**仓库根** `./target/release/bundle/`。因为根 `Cargo.toml` 是 workspace（`[workspace.package]`），cargo 的 target 目录落在 workspace 根，`src-tauri/target/` 始终是空的——盯着它看会误判成"编译还没开始"。原 `sidex-release` skill 文档写的是 `src-tauri/target/...`，已修正。
- **教训**：
  1. Tauri 的"编译完成"不等于"打包成功"，**必须看最后一行是不是 `Finished N bundles at:`**，否则会拿着一个不存在的安装包路径去汇报。
  2. 带语义后缀的版本号（`-custom.N`/`-beta.N`）在 Windows 安装包链路上是"二等公民"，遇到 `must be numeric-only` 直接找该打包器的 version override 字段，别改应用版本迁就。
  3. 多平台产物路径先 `ls` 确认，别信文档。
- **验证**：加 `wix.version` 后重跑（Rust 增量 5m 48s），`Finished 2 bundles at:` 列出
  `bundle/msi/SideX_0.1.3-custom.1_x64_en-US.msi`（18MB）与 `bundle/nsis/SideX_0.1.3-custom.1_x64-setup.exe`（13MB）；产物名仍是 `-custom.1`，而 MSI 内部版本为 `0.1.3.1`。

## 5. Claude Code 扩展报 "the Windows (win32-x64) build could not be found"：同 ID 同版本两个目录并存时**先扫到的赢**，装错平台的包会顶掉正确的包

- **日期**：2026-10-12（用户装完 Claude Code 扩展后无法登录/使用时排查）
- **现象**：Claude Code 面板能打开，但直接红框报错，登录按钮点不动：
  ```
  The Claude Code program for this computer, the Windows (win32-x64) build, could not be found.
  The installed extension holds the Linux build instead. ...
  To fix this, reinstall the extension, or set claudeCode.claudeProcessWrapper to a program that runs Claude Code.
  ```
- **根因（两层）**：
  1. **扩展目录里有两个同 id 的包**：`%APPDATA%\SideX\extensions\` 下同时存在
     `Anthropic.claude-code`（2.1.296，实为 **Linux-arm64** 包）与 `anthropic.claude-code-2.1.296-win32-x64`（2.1.296，正确的 Windows 包）。
     用 `file` 看 native 二进制即可分辨：前者 `ELF 64-bit LSB executable, ARM aarch64, /lib/ld-musl-aarch64.so.1`，`audio-capture/arm64-linux`；后者 `PE32+ executable for MS Windows, x86-64`，`audio-capture/x64-win32`。
     —— 用户拿到的是 **alpine-arm64**（musl ARM）变体。**注意：这不是用户下错包，是 SideX 自己请求错的平台**，见下方"真正的根因"。
  2. **谁赢由扫描顺序决定，不是版本高低**：`scan_extensions()`（`src-tauri/src/commands/extension_platform.rs:548`）用 `HashMap<id, manifest>` 去重，替换条件是
     `is_version_greater(new, existing)`；两个包版本**都是 2.1.296**，`is_version_greater` 在相等时返回 `false` → **不替换 → 先扫到的胜出**。
     而 `fs::read_dir` 在 NTFS 上按字母序返回，`Anthropic.claude-code` 排在 `anthropic.claude-code-2.1.296-win32-x64` 之前 → **坏的 Linux 包赢**，把正确的 Windows 包顶掉。
- **解法**：把错平台的包**移出** `extensions/`（SideX 只扫 `%APPDATA%\SideX\extensions`，移到同级目录即失效，可直接还原）：
  ```bash
  cd "$APPDATA/SideX" && mkdir -p extensions-disabled
  mv extensions/Anthropic.claude-code extensions-disabled/Anthropic.claude-code.linux-arm64-BROKEN
  ```
  正确包保持 VS Code 目录命名 `<publisher>.<name>-<version>-<platform>` 即可被识别。
  **以后下载 VSIX 必须显式带平台**：`.../vsextensions/claude-code/<版本>/vspackage?targetPlatform=win32-x64`
  （marketplace 网页上的 Download 按钮给的不一定是本平台包，这是本次踩坑的来源）。
  兜底方案：在设置里把 `claudeCode.claudeProcessWrapper` 指向一个能跑的 claude（如本机 CLI `~/.local/bin/claude.exe`）。
- **教训**：
  1. 扩展报"平台不对 / 找不到本平台二进制"时，**先 `file` 一下扩展里的 native 二进制**，一眼分辨 ELF/PE、x64/arm64，比读日志快。
  2. 同 id 多副本并存时**别假设"版本高的赢"**——本仓库的实现是"同版本不替换，先扫到的赢"，所以**多余副本必须清掉，不能靠留新版覆盖旧版**。
  3. `%APPDATA%\SideX\extensions\` 下出现两种命名（`<id>` 与 `<id>-<version>-<platform>`）时就是重复安装的信号。
- **验证**（release 包无日志，`tauri_plugin_log` 只在 `debug_assertions` 下启用，所以靠日志和产物文件取证）：
  1. 移走错包后 `extensions/` 只剩 `anthropic.claude-code-2.1.296-win32-x64`。
  2. **抓 init 数据取证**（后端把扩展列表写进 `%TEMP%\sidex-init-<uuid>.json` 交给宿主，宿主读完即 `unlinkSync`，必须在启动瞬间抢读）：
     ```powershell
     $probe='C:\...\Temp\sidex-probe-tmp'; Remove-Item $probe -Recurse -Force; New-Item -ItemType Directory $probe|Out-Null
     $env:TMP=$probe; $env:TEMP=$probe        # 让子进程的 std::env::temp_dir() 指向空目录
     Start-Process '...\target\release\SideX.exe'
     while(...){ $f=Get-ChildItem $probe -File|?{$_.Name -like 'sidex-init-*'}; if($f){Copy-Item $f[0].FullName $out;break} }
     ```
     结果：`extensions` 只有 **1 条** `Anthropic.claude-code` 2.1.296，`extensionLocation.path` = `/C:/Users/.../SideX/extensions/anthropic.claude-code-2.1.296-win32-x64`（**正确的 Windows 目录**）。
     踩过的两个小坑：① `Register-ObjectEvent -Action` 在独立 runspace 执行，引用外部变量得用 `$using:` 或硬编码路径；② 轮询别用 `Get-ChildItem | Select -First 1` 抓"第一个文件"就退出——temp 里有近 10 万个杂项文件，会抓到无关 `.tmp` 而漏掉真正的 init 文件，**必须按文件名过滤**。
  3. **按扩展自身逻辑 A/B 实测**（复刻 `extension.js` 里的解析函数）：
     | 包 | ① `resources/native-binaries/win32-x64/claude.exe` | ③ `resources/native-binary/claude.exe` | 结果 |
     | --- | --- | --- | --- |
     | 好包 win32-x64 | ✗ | **✓** | 解析成功 |
     | 坏包 linux-arm64 | ✗ | ✗ | `undefined` → 抛 `unsupported_platform` |
     坏包一列正是用户截图那句报错的来源，好包走 ③ 回退命中。
  4. 好包自带二进制可执行：`resources/native-binary/claude.exe --version` → `2.1.296 (Claude Code)`。
  5. 启动 SideX 后扩展宿主以子进程形式起来（`node --max-old-space-size=3072 target/release/extension-host/server.cjs`），无扩展加载报错。
- **未验证**：登录环节需在 GUI 里点，无法自动化；修完后用户重启 SideX 方可确认端到端可用。
- **真正的根因（后端日志实锤，2026-10-12 追加）**：日志 `%LOCALAPPDATA%\dev.sidex.app\logs\SideX.log` 里留下了安装时的原始请求：
  ```
  [sidex_lib::commands::extensions][INFO] downloading extension from
    https://marketplace.siden.ai/api/asset/openvsx/vsix-<base64>/Microsoft.VisualStudio.Services.VSIXPackage?redirect=true
  [sidex_lib::commands::extensions][INFO] installed extension Anthropic.claude-code to ...\extensions\Anthropic.claude-code
  ```
  base64 解出来是：`https://open-vsx.org/api/Anthropic/claude-code/alpine-arm64/2.1.296/file/Anthropic.claude-code-2.1.296@alpine-arm64.vsix`
  —— **应用主动请求了 `alpine-arm64` 变体**，装完后那份包自然带的是 musl-ARM 二进制。所以这是**平台选择缺陷，不是用户操作问题**。
  调用链：前端 gallery service（`src/vs/platform/extensionManagement/common/extensionGalleryService.ts`，`CURRENT_TARGET_PLATFORM = getTargetPlatform(platform, arch)`，第 91 行）→ `marketplace.siden.ai/api/gallery`（`product.ts:76` 的 `serviceUrl`）→ 服务端返回 `/api/asset/openvsx/vsix-<base64>/…` 资源 URL → 前端把整条 URL 交给 Rust 的 `install_extension_from_url`（`src-tauri/src/commands/extensions.rs:81`，只负责下载，不参与平台选择）。
  平台值来源：`getTargetPlatform(platform | 'alpine', arch)`（`extensionManagement.ts:115`），其中 `arch` 来自 `base/common/process.js` —— **源码注释明确写着 "arch is undefined in web"**，webview 里它很可能是 `undefined`。`alpine` 分支由 `isAlpineLinux()`（`extensionManagementUtil.ts:209`，读 `/usr/lib/os-release` 判 `ID=alpine`）触发。
  ⚠️ **尚未完全钉死**：`alpine-arm64` 究竟是在客户端算出来的，还是 `marketplace.siden.ai` 服务端在客户端没给出平台时的默认值，还需最后一步确认（下一步可查 `getTargetPlatform` 的实际入参）。**若要根治，应从这里入手，而不是重复删目录。**
- **备注**：早期我曾判断"后端没有 VSIX 安装代码、包是外部放进来的"——**那是错的**，只 grep 了 `extension_platform.rs`，漏了 `src-tauri/src/commands/extensions.rs`（`install_extension` / `install_extension_from_url` / `uninstall_extension` 三个命令，均已在 `lib.rs` 注册）。教训：找功能别只搜一个模块，先按命令名在 `lib.rs` 的注册表里反查。

## 6. 排障时"日志在哪"：日志**只在 dev 构建**输出，release 包零日志；落点是 `%LOCALAPPDATA%\<identifier>\logs\`

- **日期**：2026-10-12（排查 Claude Code 扩展问题时顺带摸清）
- **现象**：跑 release 版 `target/release/SideX.exe` 时 stdout 全空、也找不到日志文件，一度以为"这仓库没有日志系统"。
- **根因**：`tauri-plugin-log` 的注册被 `if cfg!(debug_assertions)` 包住（`src-tauri/src/lib.rs:655` 附近）。**release 构建里它根本不注册**，于是所有 `log::info!/warn!/error!`（log crate 的 no-op 实现）被直接丢弃——不是没写日志，是没人接收。
- **落点**（**仅 dev 构建**）：
  - 文件：`%LOCALAPPDATA%\<identifier>\logs\SideX.log`，本机即 `C:\Users\<用户>\AppData\Local\dev.sidex.app\logs\SideX.log`（identifier 来自 `tauri.conf.json` 的 `dev.sidex.app`）
  - 同时输出到 stdout（`npm run tauri dev` 的终端里能直接看到）
  - 配置为 `tauri_plugin_log::Builder::default().level(LevelFilter::Info)`，默认 targets = **Stdout + LogDir**（见 `tauri-plugin-log-2.8.0/src/lib.rs:50-51`）
  - 注意：`%APPDATA%\SideX\`（扩展目录、state db 所在）和日志目录**不是同一个**——前者是 SideX 自定义的路径（`sidex_extensions::paths`），后者是 Tauri 按 identifier 算的。
- **覆盖范围**：Rust 后端全量（扩展发现/安装、sidex-server、命令层）。本次 Claude Code 的根因就是从这个文件直接读到的（`downloading extension from ...alpine-arm64...`）。
- **不覆盖**：
  - 前端 TS 的 `ILogService` —— 只出现在 webview console，**不写进这个文件**
  - 扩展宿主的 stdout/stderr —— 被 Rust 读进内存 buffer（`src-tauri/src/commands/ext_host.rs:298` 的 `stderr_buf`），不落盘
- **其他可用取证点**：`%TEMP%\sidex-init-<uuid>.json`（宿主读完即 `unlinkSync`，要抢读，手法见 #5）；devtools 由 `tauri` 的 `devtools` feature 提供，且在 setup 里**无条件** `window.open_devtools()`（没被 `debug_assertions` 包住，见 `lib.rs` 该段），所以 release 启动时也会多开一个 devtools 窗口。
- **解法/建议**：
  1. 排障、验收时**用 dev 构建**（`npm run tauri dev`）而不是 release exe，否则什么都看不到。
  2. 想长期靠日志排查 release 问题，需要一处自定义改动：把 plugin 注册从 `cfg!(debug_assertions)` 解绑（例如加 `SIDEX_LOG=1` 环境变量或设置项开关）。这是**改上游文件**，要走 `[CUSTOM-BEGIN]` 标记 + registry 登记。
- **教训**：判断"有没有日志"要看**注册代码**而不是搜 `log::` 调用点——调用点满仓库都是，但 release 里全是空操作。
- **验证**：`ls %LOCALAPPDATA%\dev.sidex.app\logs\` → `SideX.log`（含 15 行，从 dev 运行产生）；对照 release exe 运行后该文件无新增行。




## 7. 「没法安装 VSIX 扩展」：前端跑在 Web 模式，VSIX 安装链路整体缺 server；拖拽还被 Tauri 吞掉

- **日期**：2026-10-13（CUSTOM-20261013-001，排查用户安装 `vscode-acp-client-1.0.1.vsix` 失败）
- **现象**：安装本地 `.vsix` 无从下手——「Install from VSIX...」在命令面板和扩展视图「...」菜单里都**不出现**；硬走别的入口（拖拽）也失败（`No Servers` / 无反应）。上游 issue #12「不支持 vsix」关闭时维护者的答复就是 "VSIX installation isn't fully wired up yet, it's planned as the next thing"。
- **根因**（三处叠加，缺一不可）：
  1. **前端是 Web 模式**：Tauri webview 里 `platform.ts` 走 "Web environment" 分支（没有 `process`、非 electron renderer）→ `isWeb === true`。于是 `ExtensionManagementServerService`（`src/vs/workbench/services/extensionManagement/common/extensionManagementServerService.ts:26`）**只注册 `webExtensionManagementServer`**，`localExtensionManagementServer` 恒为 `null`。
  2. **上游 VSIX 链路强依赖 local/remote server**：
     - 菜单 `when` 是 `hasLocalServer || hasRemoteServer`（`extensions.contribution.ts:850/853/879`）→ 两项都为 false，命令与菜单项被隐藏；
     - `installFromVSIX()` → `getManifest(vsix)`（`extensionManagementService.ts:540`）对 `file://` 无分支可走，直接 `Promise.reject('No Servers')`；`installVSIX()` → `getServersToInstall()` 返回 undefined → `reject('No Servers to Install')`；
     - `installFromLocation(location)` 对 `file://` 同样直接 `throw new Error('Local extension management server is not found')`（`extensionManagementService.ts:522`）。
  3. **拖拽进不来**：`src-tauri/tauri.conf.json` 的 `dragDropEnabled: true`（默认值）表示由 **Tauri 接管 OS 拖放**，webview 收不到 HTML5 drop 事件。所以 `extensionsViewlet.ts:684` 那个 DropObserver **在本项目里永远不会触发**；而 `web.main.ts:201` 监听的那个 `sidex-open-paths` 自定义事件**仓库里没有任何地方派发**（死代码）——两头都不通。
  - 补充：Rust 侧能力**早就写好了**（`src-tauri/src/commands/extensions.rs:64` 的 `install_extension` → `crates/sidex-extensions/src/vsix.rs`，解压到 `%APPDATA%\SideX\extensions\<publisher>.<name>`），只是前端从没接过线；`sidexExtensionService.ts:95` 那处 `invoke('install_extension', { extensionId: id })` 参数名与 Rust 的 `vsix_path` 不匹配，是**从未生效的死代码**（别照着它改）。
- **解法**（CUSTOM-20261013-001）：新增自定义模块 `CUSTOMIZATIONS/src/vsix-install/vsixInstall.contribution.ts`（挂在 `src/main.ts`），把两端接起来而不动上游扩展管理逻辑：
  1. 自定义命令 `sidex.extensions.installFromVSIX`（命令面板 + 扩展视图标题「...」菜单 + 资源管理器右键 `.vsix`），文件选择走已桥接 Tauri 的 `IFileDialogService`；
  2. Tauri `install_extension` 解压落盘；
  3. **关键一步**：拿 `IExtensionManagementServerService.webExtensionManagementServer` 直接调 `extensionManagementService.installFromLocation(URI.file(安装目录), currentProfile.extensionsResource)` —— 绕开 workbench 服务对 `file://` 的 `'Local extension management server is not found'` 分支，同时事件仍会经 workbench 服务的 multiplexer 冒泡，触发 `tauriExtensionHost.contribution.ts` **既有**的热加载（`loadExtension` + `activateExtension`）；
  4. 拖拽用 `getCurrentWebviewWindow().onDragDropEvent()` 监听（Tauri 事件，路径里筛 `.vsix`），补上 `dragDropEnabled: true` 下的缺环。
- **验证**：`npx eslint` 自定义模块无告警；`npm run build` 通过且 `dist/assets/index-*.js` 里能 grep 到 `sidex.extensions.installFromVSIX`。
- **教训**：
  1. 判断"某个 VSCode 功能在 SideX 里能不能用"，**先看它依赖哪个扩展管理 server**——`isWeb === true` 意味着一切 `file://` 相关的安装/卸载入口都可能是断的。
  2. Tauri 里排查"拖拽不管用"，先看 `tauri.conf.json` 的 `dragDropEnabled`：为 `true` 时前端拿不到 HTML5 drop 事件，只能监听 Tauri 的 `onDragDropEvent`。
  3. 仓库里存在**只有监听者、没有派发者**的死代码（`sidex-open-paths`）——别看到监听器就以为链路是通的。

### 追加（CUSTOM-20261013-002）：`file://` 被改写成 `sidex-asset://`，凡是"读磁盘扩展文件"的路径都会炸

- **现象**：从 VSIX 装完（Rust 侧已落盘、日志有 `installed extension … to …`）后，前端登记阶段抛
  `Cannot find a valid extension from the location file:///c%3A/Users/…/extensions/<id>`。
- **根因**：链路是 `WebExtensionManagementService.install()` → `webExtensionsScannerService.scanExtensionManifest(安装目录)` → `getExtensionManifest()` → `joinPath(location,'package.json')` → `extensionResourceLoaderService.readExtensionResource(uri)`。而该方法第一句是 `uri = FileAccess.uriToBrowserUri(uri)`，SideX 在 `src/vs/base/common/network.ts:328` 加了一处分支：**Tauri 下任何 `file://` 都改写成 `sidex-asset://localhost/<encodeURIComponent(fsPath)>`**（本意是让 `<img src>`/`fetch` 走 Tauri 自定义协议）。改写后 scheme 既不是 http/https/data/tauri，代码便走 `IFileService.readFile(uri)` —— 但 **`sidex-asset` 没有注册 FileSystemProvider**（它只是 `src-tauri/src/lib.rs:477` 的 `register_asynchronous_uri_scheme_protocol`，属于 webview 网络层，不是 workbench 文件层）→ 读取必然失败 → `scanExtensionManifest` 返回 null → 上层报"找不到有效扩展"。
- **解法**：`extensionResourceLoaderService.ts` 的 `readExtensionResource()` 开头加前置分支（`[CUSTOM-BEGIN] CUSTOM-20261013-002`）：Tauri 且 `uri.scheme === Schemas.file` 时直接 `this._fileService.readFile(uri)`，跳过浏览器 URI 改写。
- **教训（重要）**：**别用 `FileAccess.uriToBrowserUri()` 的结果去喂 `IFileService`**。前者返回的是"浏览器/网络层"用的 URI（Tauri 下会变成 `sidex-asset://`），后者只认注册过 provider 的 scheme（`file`/`vscode-file`/`vscode-userdata`/扩展贡献的 scheme）。看到 `sidex-asset://` 出现在 `IFileService` 的报错里，就回到这里找改写点。
- **验证**：待复测（dev 实例安装该 VSIX 后扩展视图出现且激活）。
