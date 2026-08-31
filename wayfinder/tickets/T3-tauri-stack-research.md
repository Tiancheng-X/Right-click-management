---
id: T3
title: "Tauri 栈系统层能力调研"
labels: [wayfinder:research]
status: closed
assignee: "research-subagent（绘图期派出）"
blocked-by: []
---

## Question

Tauri v2 做本程序所需的系统层能力：

1. Rust 读取/枚举/写入注册表的成熟方案（winreg 等常用 crate 的能力边界与常见坑）；
2. 「始终管理员运行」如何实现：manifest `requireAdministrator`、Tauri/NSIS 打包配置下的实际行为；
3. 以编程方式可靠导出/导入 .reg 格式的可行性（自动快照的技术底座），或替代的注册表序列化方案；
4. Tauri v2 与主流前端方案（React/Vue/Svelte/原生）的集成现状与推荐；
5. 绿色便携单 exe 的构建配置：能否不装即用、数据目录该放哪。

## Resolution

完整调研（含出处链接与「未确证」清单）见 [research/T3-tauri-stack-research.md](../research/T3-tauri-stack-research.md)（截至 2026-08-30，Tauri 2.11.5）。

- 注册表：选 winreg 0.56（枚举/读写/删除/事务齐全，`KEY_WOW64_32/64KEY` 双视图是一等 API）；windows-registry 0.6（微软官方系）更现代但无一等 WOW64 视图方法。
- 提权：tauri.conf.json 无内置开关（#7173 仍 open）；官方路径是 build.rs 里 `WindowsAttributes::app_manifest` 注入 `requireAdministrator`；manifest 嵌进 exe，裸单 exe 直跑同样弹 UAC；注意保留默认 Common-Controls v6 依赖、dev 模式也会提权（可用 PROFILE 条件化）。
- .reg：导出/导入直接调 reg.exe export/import（静默、退出码 0/1、支持 /reg:32/64）；自动快照主存储用 API 枚举 + 自有 JSON 格式、恢复走 API 精确写回（reg import 是 merge 语义），.reg 仅作人工恢复交付物。
- 前端：Tauri 框架无关，create-tauri-app 模板齐全；本项目自定义 Fluent UI 推荐原生 TS + Vite（vanilla-ts），备选 Svelte/Solid。
- 便携：直接分发 target/release 单 exe，唯一硬依赖 WebView2（Win11 预装）；官方 updater 仅支持安装器形态（自动更新本就 out of scope）；数据目录推荐 exe 同目录 `data\` 子目录（Tauri 的 app_data_dir 恒指 %APPDATA%，需自实现），不可写时回退 %APPDATA%，细节留 T6 定案。
