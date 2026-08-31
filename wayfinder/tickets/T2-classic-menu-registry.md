---
id: T2
title: "经典菜单注册表全景调研"
labels: [wayfinder:research]
status: closed
assignee: "research-subagent（绘图期派出）"
blocked-by: []
---

## Question

经典右键菜单的注册表全景：

1. 全部挂载点清单：`HKCR\*`、`AllFilesystemObjects`、`Directory`、`Directory\Background`、`Desktop`、`Drive`、`Folder`、`lnkfile` 等下的 `shell\verb` 与 `shellex\ContextMenuHandlers`，以及 HKCU 与 HKLM 的视图合并规则；
2. 每类条目（verb / shellex / RunAs / SkipExplorerPage 等）如何新增、禁用、删除、恢复；
3. 常见第三方软件（7-Zip、VSCode、微信、WinRAR、Notepad++ 等）通常把菜单项写在哪里；
4. 如何从注册表键推导出用户可见的「名称、图标、命令行、适用场景」，包括 MUIVERB、Icon 资源引用、`Position`、`Extended` 等修饰值的含义。

## Resolution

调研完成，全文见 [wayfinder/research/T2-classic-menu-registry.md](../research/T2-classic-menu-registry.md)（含 47 条出处与本机 Win11 只读采样佐证）。要点：

1. 经典菜单只有两种挂载机制：`shell\<verb>\command` 静态动词与 `shellex\ContextMenuHandlers\<名称>`（默认值=CLSID 的 COM 处理器）；挂载点为 `*`、`AllFilesystemObjects`、`Folder`、`Directory`、`Directory\Background`、`DesktopBackground`（并非 `Desktop`，本机已实测）、`Drive`、`lnkfile` 及任意 ProgID/SystemFileAssociations。
2. HKCR 是 `HKLM\SOFTWARE\Classes` 与 `HKCU\Software\Classes` 的合并视图，官方规则为按键级「HKCU 优先」；管理器写入应显式走 HKCU/HKLM，不直接写 HKCR。
3. 禁用按可逆性排列：`LegacyDisable`/`ProgrammaticAccessOnly`/`Extended`（官方确证，本机各有活样本）→ 导出 .reg 后删键；shellex 可删子键、写入 `Blocked` 键或改无效 CLSID（本机 AccExt/ModernSharing 两例实证）；`NoViewContextMenu`/`NoTrayContextMenu` 策略可整层关菜单，排查时先查策略。
4. 第三方分两派：压缩/网盘类（7-Zip、WinRAR、OneDrive）走多挂载点 shellex；编辑器类（VS Code、Notepad++）走静态动词（VS Code 亦有 ProgID 关联形态）；微信无可靠公开资料（未确证，本机未安装）。
5. 可见属性可完全推导：名称=MUIVerb（含 `@dll,-id` 间接字符串）＞动词键默认值；图标=`Icon`（`路径,-资源ID`）；命令=`command`+`%1`/`%V` 等占位符；适用场景=挂载点；`Position`/`Extended`/`CommandFlags`/`MultiSelectModel` 控制位置与显隐。
6. `SkipExplorerPage` 查无任何出处且本机全挂载点扫描 0 命中，建议从工单术语表移除。
