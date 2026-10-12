# CUSTOMIZATIONS/src —— 自定义源码

新功能优先放这里（独立模块，通过入口挂载），避免直接改上游文件。

- 此目录下的文件冲突策略恒为 `keep-ours`。
- 每个模块登记到 `../registry.md` 的改动总览。
- 挂载点（如前端入口 `src/main.ts`、Rust `lib.rs` 注册）属于对上游文件的修改，需加 `[CUSTOM-BEGIN]` 标记。

## 现有模块

| 模块 | 挂载点 | 作用 |
| ---- | ------ | ---- |
| `vsix-install/vsixInstall.contribution.ts` | `src/main.ts`（顶部静态导入，workbench `create()` 之前） | 从本地 `.vsix` 安装扩展：命令面板 / 扩展视图「...」菜单 / 资源管理器右键 `.vsix` / 拖拽 `.vsix` 进窗口 → Tauri `install_extension` 解压 → web 扩展服务 `installFromLocation` 登记并热加载 |

> 前端模块的导入路径必须带 `.js` 后缀（ESM 约定），可用的路径别名：`vs/*` → `src/vs/*`（见 `tsconfig.json` 的 `paths` 与 `vite.config.ts` 的 `resolve.alias`）。

