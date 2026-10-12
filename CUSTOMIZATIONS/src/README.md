# CUSTOMIZATIONS/src —— 自定义源码

新功能优先放这里（独立模块，通过入口挂载），避免直接改上游文件。

- 此目录下的文件冲突策略恒为 `keep-ours`。
- 每个模块登记到 `../registry.md` 的改动总览。
- 挂载点（如前端入口 `src/main.ts`、Rust `lib.rs` 注册）属于对上游文件的修改，需加 `[CUSTOM-BEGIN]` 标记。
