---
id: T1
title: "Win11 新版右键菜单机制调研"
labels: [wayfinder:research]
status: closed
assignee: "research-subagent（绘图期派出）"
blocked-by: []
---

## Question

Win11 新版（折叠）右键菜单的机制全景：

1. 新版菜单条目从哪里来（系统动词、shellex COM 处理器、MSIX/IExplorerCommand 等），各自如何识别；
2. 如何枚举出某个场景（文件/文件夹/桌面/背景）下新版菜单的全部条目并标明来源；
3. 如何逐项屏蔽与恢复——`Shell Extensions Blocked` 键等已知手段的可行性与副作用；
4. 「一键切回经典菜单」的 CLSID `{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}` 方案原理、生效条件（重启 explorer）与 Win11 22H2/23H2/24H2 各版本兼容性、风险；
5. 第三方应用把命令加进新版顶部菜单的 sparse MSIX 路线概貌（v1 不做，只要结论，供未来参考）。

## Resolution

完整报告：[wayfinder/research/T1-win11-new-menu-mechanics.md](../research/T1-win11-new-menu-mechanics.md)（2026-08-30，含全部出处链接与未确证标注）。结论要点：

1. Win11 新菜单是双层结构：只显示系统内置命令、静态动词、文件关联、云文件命令与「IExplorerCommand + 包标识」条目；旧 `IContextMenu` COM 处理器全部折叠进「显示更多选项」，无条目被移除（官方博客）。
2. 枚举无单一公开 API，须分来源聚合：静态动词/COM 处理器扫 HKCR（`*\shell`、`*\shellex\ContextMenuHandlers`、`Directory`、`Directory\Background`、`DesktopBackground`、`Folder`、`Drive` 等），打包条目读 MSIX 清单 `desktop4:FileExplorerContextMenus`。
3. 逐项屏蔽用「写值不删键」：`HKCU\...\Shell Extensions\Blocked` 加 CLSID 空串值（免管理员，对打包条目也实测有效但官方未文档化）；静态动词加 `ProgrammaticAccessOnly`/`Extended`（官方语义）。副作用：按 CLSID 全局生效、误伤共享组件、需重启 Explorer。系统内置项无法屏蔽。
4. 切回经典菜单 = `HKCU\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32` 建**空字符串**默认值（不能是「未设置」）+ 重启 explorer；原理是该 CLSID 指向新菜单实现 `Windows.UI.FileExplorer.dll`（本机 24H2 实测），HKCU 覆盖致激活失败而回退。22H2/23H2/24H2/25H2 稳定版均可用，但属非官方手段，Insider 曾反复，需做失效兜底。
5. sparse MSIX（`IExplorerCommand` + 仅身份包）是第三方进入新菜单顶部的唯一正路，有限制（每处理器 16 项、需签名、DLL 架构匹配）；v1 不做，产品侧对其条目用 Blocked 键/注销包管理即可。
