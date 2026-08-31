---
id: T14
title: "新版菜单条目（MSIX 枚举与屏蔽）"
labels: [wayfinder:task]
status: closed
assignee: "本会话"
blocked-by: []
---

## Question

按「Win11 新版右键菜单机制调研」补齐新版菜单侧：

1. 枚举打包条目：解析 MSIX 包清单 `desktop4:FileExplorerContextMenus`，与经典条目统一进域模型；
2. 逐项屏蔽：Blocked 键写 CLSID 空串值（UI 标注「对打包条目官方未文档化，实测有效」）；恢复 = 移除该值；
3. 条目详情「出现在：新版菜单 / 仅经典菜单」标注接真数据（shellex 仅经典层）；
4. 系统内置条目只读展示。

## Resolution

完成（双侧编译全绿零警告）：

- **枚举**（`msix.rs`）：`PackageManager::FindPackagesWithPackageTypes(Main)` 枚举全部主包 → 读各包 `AppxManifest.xml` → roxmltree 按本地名解析 `desktop4/desktop5:FileExplorerContextMenus`（同时覆盖文件项与目录背景命名空间），产出（包显示名 · VerbId · CLSID · ItemType 列表 · 包 Logo data URL）。
- **显示名限制（如实记录）**：真实标题在 `IExplorerCommand::GetTitle`，需 COM 激活——v1 显示「包显示名 · VerbId」，来源标签 = 包显示名；未来需要时再做打包 COM 激活。
- **屏蔽/恢复**：复用 Blocked 键机制（`set_entry_enabled` 增加 packaged 分支，与 shellex 同路）；开关初始态 = CLSID 是否在 Blocked 集合；UI 的开关 tooltip 标注「对打包条目官方未文档化，实测有效」。
- **删除**：打包条目 v1 不提供（不卸载应用包），后端显式拒绝。
- **分层标注**（D5）：卡片新增 📍 层级标签——shellex = 仅经典菜单、packaged = 仅新版菜单、动词/自定义 = 新版 + 经典。
- **系统内置**：无公开枚举 API（T1 结论），v1 不出现在管理列表；卡片层级标注覆盖其余条目的分层语义。
- 失败静默：打包枚举失败不影响经典条目展示。
