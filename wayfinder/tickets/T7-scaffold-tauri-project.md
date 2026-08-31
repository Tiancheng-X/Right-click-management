---
id: T7
title: "初始化 Tauri 项目骨架"
labels: [wayfinder:task]
status: closed
assignee: "本会话"
blocked-by: [T3, T4]
---

## Question

纯执行，无决策：按「Tauri 栈系统层能力调研」与「主界面原型」的结论初始化仓库——建 Tauri v2 工程，前端落地为原生 TypeScript + Vite + 自写 Fluent 组件（调研推荐 + 原型已验证可行），以原型的 **变体 A · 资源管理器三栏** 为视觉基准（Mica、圆角、深浅主题、Fluent 控件已在原型定型；B/C 变体留作细节参考）；配置管理员权限 manifest（build.rs `app_manifest` 注入 requireAdministrator）、搭注册表访问模块占位（winreg），`tauri dev` 跑通。完成后在本工单记录实际目录结构与运行方式。

## Resolution

骨架完成（2026-08-30 会话），验证全绿：

- **前端**：`pnpm build`（tsc 严格模式 + vite）通过，产物 dist/；
- **后端**：`cargo check` 通过（54.7s），tauri-build 2.6.3 成功注入 app.manifest，winreg 0.56 接入；
- **数据通路**：`list_menu_entries` 命令 → `src-tauri/src/registry.rs` 只读枚举 HKCR `*` 与 `Directory` 两挂载点的 shell\verb 与 shellex\ContextMenuHandlers（MUIVerb＞默认值＞键名取显示名）→ 前端变体 A 三栏渲染；后端不可达时回退 mock 并显示「示例数据」徽章，连通时显示「已连接注册表」。

**实际结构**：根目录即工程——`index.html` + `src/`（main.ts 变体 A 布局、styles.css Fluent 令牌）+ `src-tauri/`（`app.manifest`、`build.rs` 注入、`registry.rs` 占位模块、`tauri.conf.json` 窗口 1180×760 标题「右键菜单管家」）。命名：package/crate = right-click-manager，lib = right_click_manager_lib，identifier = com.rightclickmanager.app。

**运行方式**：`pnpm install` → `pnpm tauri dev`（**必弹 UAC**——始终管理员运行的既定行为，dev 模式同样注入 manifest）；发布 `pnpm tauri build`。

**pnpm 11 注意**：依赖构建脚本白名单在 `pnpm-workspace.yaml` 的 `allowBuilds`（esbuild: true），package.json 的 `pnpm` 字段在 v11 无效。
