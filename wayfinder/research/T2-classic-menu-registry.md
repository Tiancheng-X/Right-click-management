# T2 调研：经典右键菜单注册表全景

> 调研对象：Windows 11 上「经典（Win32/Explorer）右键菜单」的全部注册表挂载点、条目生命周期（新增/禁用/删除/恢复）、典型第三方落点、以及由注册表推导用户可见属性（名称/图标/命令行/适用场景）。
> 本文是右键菜单管理器（Tauri v2）领域模型的证据基础；Windows 11 新版菜单（IExplorerCommand / `{86ca1aa0-…}` 开关等）机制归工单 T1，本文仅在边界处提及。

**TL;DR**

- 经典菜单条目只有两种挂载机制：静态动词 `shell\<verb>\command`（纯注册表键值）与 COM 处理器 `shellex\ContextMenuHandlers\<名称>`（默认值 = CLSID，DLL 挂在 `HKCR\CLSID\<clsid>\InprocServer32`，ThreadingModel 必须 Apartment）[来源 1][2]。
- 挂载点是「对象键 + shell/shellex」的组合：`*`、`AllFilesystemObjects`、`Folder`、`Directory`、`Directory\Background`、`DesktopBackground`、`Drive`、`lnkfile` 以及任意 ProgID（`exefile`、`txtfile`、压缩包 ProgID 等）[来源 2][3]。
- HKCR 不是真实 hive：它是 `HKLM\SOFTWARE\Classes` 与 `HKCU\Software\Classes` 的**合并视图**，HKCU 优先；交互用户的写入应写 `HKCU\Software\Classes`（即通过 HKCR 写入会落到 HKCU）[来源 10][11][12]。
- 禁用/删除手段按「可逆性」排序：加 `LegacyDisable`（隐藏但保留）＞ `ProgrammaticAccessOnly`（菜单不显示、编程仍可调）＞ 改 `Extended`（Shift 才显示）＞ 导出 .reg 后删键（彻底但可恢复）；shellex 则是删子键 / 加 Blocked 键 / 策略键。改名、ACL 拒绝等手法属社区经验，官方未确证。
- 策略层总开关：`NoViewContextMenu`（Explorer 内右键）与 `NoTrayContextMenu`（任务栏右键）都是 ADMX 策略，落在 `…\Policies\Explorer`，能一键废掉整个菜单，管理器必须先读策略再谈条目 [来源 18][19]。
- 第三方落点有清晰派系：压缩工具（7-Zip/WinRAR/OneDrive）走 `shellex\ContextMenuHandlers` 多挂载点；编辑器类（VS Code/Notepad++ 的静态形态）走 `*\shell`、`Directory(\Background)\shell`；微信未找到可靠公开资料（未确证）。
- 用户可见属性全部可推导：名称 = `MUIVerb`（含 `@dll,-id` 间接字符串）或动词键默认值；图标 = `Icon`；命令 = `command` 默认值 + `%1` 等占位符；适用场景 = 挂载点；显隐/位置修饰 = `Position`/`Extended`/`CommandFlags`/`MultiSelectModel` 等 [来源 1][14][16]。

---

## 1. 挂载点全景（shell\verb 与 shellex\ContextMenuHandlers）

### 1.1 两种挂载机制

| 机制 | 键结构 | 载体 | 说明 |
| --- | --- | --- | --- |
| 静态动词（static verb） | `<挂载点>\shell\<verb>`，其下 `command` 子键默认值 = 命令行 | 纯注册表 | 动词子键默认值（REG_SZ）= 菜单显示文本；规范动词（open/print/edit…）可省略显示文本，系统自动本地化；非规范动词省略则直接显示动词名 [来源 1][4] |
| COM 处理器（shellex） | `<挂载点>\shellex\ContextMenuHandlers\<名称>`，子键默认值 = CLSID 字符串 | 进程内 COM DLL | CLSID 在 `HKCR\CLSID\<clsid>` 下注册，`InprocServer32` 默认值 = DLL 路径，`ThreadingModel` **必须为 `Apartment`**（官方强调，否则间歇性失败/死锁/崩溃）；处理器子键名 Shell 不使用，仅需在同一父键下唯一 [来源 2][3] |

- 其他壳扩展子键（`DropHandler`、`IconHandler`、`PropertySheetHandlers` 等）与 `ContextMenuHandlers` 同属 `shellex` 下，但注册形态不同（部分类型默认值直接写在 handler 键上）[来源 2]。
- 修改/删除注册后，官方要求调用 `SHChangeNotify(SHCNE_ASSOCCHANGED)` 通知 Shell，否则更改可能直到重启才生效——对应到管理器：改完注册表要触发关联刷新 [来源 2]。
- 动态动词（IContextMenu 实现返回菜单项）与静态动词的选型对比见官方「Choosing a Static or Dynamic Verb」；ProgID 层还有 `shellex` 下的 `MayChangeDefaultMenu` 值用于声明处理器可能改变默认菜单 [来源 5]。

### 1.2 对象级挂载点（官方「Predefined Shell Objects」表）

官方在 Registering Shell Extension Handlers 中给出可直接挂 shell/shellex 的预定义对象表 [来源 2][3]：

| 挂载点 | 含义 | 官方允许的处理器类型 |
| --- | --- | --- |
| `HKCR\*` | 所有文件 | Shortcut Menu、Property Sheet、Verbs |
| `HKCR\AllFilesystemObjects` | 所有文件 + 文件文件夹（文件系统对象） | Shortcut Menu、Property Sheet、Verbs |
| `HKCR\Folder` | 所有文件夹（含虚拟文件夹） | Shortcut Menu、Property Sheet、Verbs |
| `HKCR\Directory` | 文件系统目录 | Shortcut Menu、Property Sheet、Verbs |
| `HKCR\Directory\Background` | 目录空白处（右键点在目录内空白处） | **仅 Shortcut Menu** |
| `HKCR\DesktopBackground` | 桌面背景（Windows 7+；官方示例即用它演示 `shell` 动词排序，如 Display/Gadgets/Personalization） | Shortcut Menu、Verbs |
| `HKCR\Drive` | 「我的电脑」中所有驱动器（如 C:\） | Shortcut Menu、Property Sheet、Verbs |
| `HKCR\Network`、`Network\Type\#`、`NetShare`、`NetServer`、`<网络提供程序名>` | 网上邻居各类对象 | Shortcut Menu、Property Sheet、Verbs |
| `HKCR\Printers` | 所有打印机 | 仅 Property Sheet |
| `HKCR\AudioCD` | CD 音频 | 仅 Verbs |
| `HKCR\DVD` | DVD 驱动器 | Shortcut Menu、Property Sheet、Verbs |

注意事项：

- 任务书提到的 `HKCR\Desktop`：官方预定义对象表中的正式名称是 **`DesktopBackground`**（Win7+）。实践中还存在 `Desktop\Background\Shell` 一类键的报告，官方文档未见收录。**本机实测：`HKCR\Desktop\shell` 与 `HKCR\Desktop\shellex\ContextMenuHandlers` 均不存在，而 `HKCR\DesktopBackground`（含 `Shell`、`shellex` 子键）真实存在并承载 Display/Personalize 等动词——桌面菜单的实际挂载点就是 `DesktopBackground`**（详见「本机注册表采样佐证」）。
- `Directory\Background` 官方明确「右键点在文件文件夹内、但不在任何内容上」即目录/桌面背景菜单；桌面背景菜单同时受 `DesktopBackground\shell` 静态动词影响 [来源 2][1]。
- `*` 与 `AllFilesystemObjects` 的分工：`*` 覆盖命名空间内一切条目（含虚拟对象），`AllFilesystemObjects` 仅文件系统对象；想「只对文件+目录生效」的第三方常选后者，避免污染虚拟对象菜单（此为社区通行理解，官方只给出定义本身 [来源 2]）。

### 1.3 ProgID 级与特殊键

| 键 | 性质 | 典型用途 |
| --- | --- | --- |
| `HKCR\<ProgID>` 下的 `shell` / `shellex`（`txtfile`、`exefile`、`WinRAR.ZIP`、`CABFolder` 等） | 文件类型/类注册 | 每个文件类型键下都可挂 `shell\<verb>` 静态动词与 `shellex\ContextMenuHandlers`，优先级最高（关联数组首位）[来源 7][32] |
| `HKCR\SystemFileAssociations\<.ext>\shell` | 系统级按扩展名的「兜底」注册 | 为某扩展名注册「次要动词」，位于关联数组中 ProgID 之后，**用户改默认程序也不失效**；感知类型（perceived type，如 `text`）也注册于此 [来源 7] |
| `HKCR\Applications\<app.exe>\shell\<verb>` | 应用程序注册 | 让应用出现在「打开方式」；可带 `FriendlyAppName`；`shell\verb` 说明应用如何被调用 [来源 6][7] |
| `HKCR\lnkfile` | 快捷方式（.lnk）类键 | 快捷方式菜单的动词（如 runas）挂点；含 `IsShortcut` 值（快捷方式箭头/语义的标志）[来源 23] |
| `HKCR\CLSID\{对象clsid}\Shell\<verb>` | 命名空间虚拟对象 | 「计算机」「回收站」等虚拟图标菜单；官方 Launching Applications 明确此处查找可用动词 [来源 8] |
| `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\CommandStore\shell\<verb>` | 系统动词仓库 | 级联菜单 `SubCommands` 引用的自定义动词集中注册处（需 HKLM 权限）[来源 1] |

- `CABFolder` 等系统 ProgID：即 .cab 的 ProgID，同其他 ProgID 一样可携带 `shell/shellex`（结构性结论，源自 ProgID 挂载机制本身 [来源 2][7]；CABFolder 特有行为未见官方专文，未确证）。
- exefile 注意点：`exefile\shellex\ContextMenuHandlers\Compatibility`（兼容性疑难解答）是系统自带 shellex 示例，损坏 exefile 注册的修复方案中可见 [来源 43]。

### 1.4 HKCR 合并规则（官方）

`HKEY_CLASSES_ROOT` 自 Windows 2000 起不再是独立 hive，而是 `HKLM\SOFTWARE\Classes`（本机默认，对所有用户生效）与 `HKCU\Software\Classes`（交互用户覆盖）的合并视图 [来源 10][12]。官方合并规则（Merged View of HKEY_CLASSES_ROOT，原文规则）[来源 11]：

1. 合并视图包含 `HKCU\Software\Classes` 的**全部子键**；
2. `HKLM\Software\Classes` 的**直接子键**若不与 HKCU 重复则包含；
3. 对一批「双侧共有的键」（官方枚举：`*`、`*\shellex`、`*\shellex\ContextMenuHandlers`、`*\shellex\PropertySheetHandlers`、`Drive`、`Drive\shellex`、`Drive\shellex\ContextMenuHandlers`、`Folder`、`Folder\shellex` 及其子键、`AppID`、`ClsID`、`Interface`、`Typelib`、`Mime\…`、`Installer\…` 等），**HKLM 侧的直接子键仅在不是 HKCU 重复项时并入；重复子键只取 HKCU 内容**——即按键级「HKCU 赢者通吃」，不是逐值合并；
4. UAC 例外：以管理员运行且 **UAC 被禁用**时，COM 运行时忽略 per-user COM 配置，只访问 HKLM 侧 [来源 11]。

写入方向：官方明确「要为交互用户更改设置，必须写在 `HKCU\Software\Classes` 之下」；对交互进程以 `HKCR` 打开的写入实际落在 `HKCU\Software\Classes`（服务/非交互进程则可用 `RegOpenUserClassesRoot` 指定目标用户的合并视图）[来源 10][11][12]。**管理器应优先写 HKCU/HKLM 实键而非 HKCR**，以明确落点与权限需求 [来源 10][12]。

### 1.5 Wow6432Node 与 32 位 shellex

- WOW64 为 32 位程序呈现独立的 `HKLM\Software\WOW6432Node` 视图；HKCR 的本机部分因位于 `HKLM\Software` 之下而被隔离；Registry Reflector 只镜像 COM 激活数据，并会把 system32 路径改写为 SysWOW64 [来源 13]。
- 因此 32 位安装器写出的处理器 CLSID/InprocServer32 落在 32 位视图（`Classes\WOW6432Node\CLSID\…`），其挂载点条目常见两种形态：同层并列子键（如 WinRAR 的 `WinRAR`＝64 位 CLSID 与 `WinRAR32`＝32 位 CLSID，实测样本 [来源 32]）；或在挂载点下出现名为 `Wow6432Node` 的子键聚集 32 位条目——后者在官方文档未见专门说明，**机制未确证（官方）**，属社区/实测观察。
- 对管理器的含义：枚举与禁用 shellex 时需同时检查两套视图（64 位 explorer 只加载 64 位进程内处理器，32 位条目对它不可见或需代理），`Wow6432Node` 侧不可忽略。

### 1.6 Approved 与 Blocked 键

| 键 | 作用 | 出处性质 |
| --- | --- | --- |
| `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved` | shell 扩展 CLSID 的「批准名单」，值为 CLSID→友好名；历史上与策略 `EnforceShellExtensionSecurity` 配合：在 Windows XP 上不在名单即不加载（7-Zip 实测被拦截），Win7/10/2019 实测该策略已不再拦截（7-Zip 删除名单条目后仍加载）[来源 41] | 微软 Q&A（社区实测），官方强制文档未见 |
| `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked` | 显式黑名单：以 CLSID 为值名即禁止加载（如 360/安全软件写入、系统更新误伤 `Sharing` tab 的 `{f81e9010-…}` 案例）；排查菜单缺失时先查此键 [来源 42][43] | 微软 Q&A（社区实测），官方专文未见 |

管理器应把 `Blocked` 命中视为独立于挂载点的「隐藏原因」，并在恢复流程中检查它。

---

## 2. 每类条目的新增、禁用、删除、恢复

### 2.1 shell\verb（静态动词）

**新增**（官方步骤 [来源 1][4]）：

1. 选挂载点（1.2 表），在其 `shell` 下建动词子键：`<挂载点>\shell\<verb>`；
2. 动词子键默认值（REG_SZ）= 显示文本（规范动词可省略；级联菜单则要求默认值留空并用 `MUIVerb`）[来源 1]；
3. 建 `command` 子键，默认值 = 命令行，含空格的路径与 `%1` 都要加引号（官方强调）[来源 6]；
4. （可选）`Shell` 键默认值 = 动词序列，控制排序与默认动词；默认动词选择顺序：Shell 默认值 → 注册表首个动词 → open → openwith [来源 1][4]；
5. 级联菜单：`SubCommands`（分号分隔，可引用 CommandStore 系统动词）或 `ExtendedSubCommandsKey`（可嵌套，且可注册在 HKCU 免提权）[来源 1]。

**禁用/隐藏手法（按官方确证程度排列）**：

| 手法 | 操作 | 效果 | 出处 |
| --- | --- | --- | --- |
| 加空 `LegacyDisable`（REG_SZ，无数据） | 写在动词子键下 | 菜单不再显示该条；社区解释为「让 Shell 忽略该键及其值」，删除该值即恢复。Visual Studio「Open in Visual Studio」条目的官方开发者社区答复即用此法（`Directory(\Background)\shell\AnyCode`）[来源 25][26] | SuperUser（社区定义）+ 微软 Developer Community（实例）|
| 加 `ProgrammaticAccessOnly`（REG_SZ，无数据） | 写在动词子键下 | **官方**：永不显示在菜单，但仍可用 ShellExecuteEx 以动词名调用 [来源 1] | 官方 |
| 加 `Extended`（REG_SZ，无数据） | 写在动词子键下 | **官方**：仅 Shift+右键时显示（扩展动词）[来源 1] | 官方 |
| `AppliesTo` AQS 条件 | 写在动词子键下 | **官方**：按条件表达式决定显示/隐藏（如 BitLocker 动词）[来源 1] | 官方 |
| 导出 .reg 后删除键 | regedit 导出备份→删除 | 彻底移除；恢复=重新导入。属通用工具操作，无需专门出处 | — |
| 动词键改名（如加前缀） | 重命名子键 | 社区常用（非规范动词名不匹配即不显示）；**官方未记载此手法，未确证** | — |
| 动词键 ACL 拒绝读取 | 拒绝 Users 读权限 | 社区流传；**未见可靠出处，未确证** | — |

**恢复**：删 `LegacyDisable`/`ProgrammaticAccessOnly`/`Extended` 值；或重新导入先前导出的 .reg；官方提醒改后应触发 `SHCNE_ASSOCCHANGED` 刷新 [来源 2]。

### 2.2 shellex\ContextMenuHandlers

**新增**：`<挂载点>\shellex\ContextMenuHandlers\<唯一名称>`，默认值 = CLSID 字符串；CLSID 在 `HKCR\CLSID\<clsid>\InprocServer32` 注册（默认值 = DLL 路径，`ThreadingModel=Apartment`）；改后触发关联刷新 [来源 2][3]。

**禁用手法**：

| 手法 | 操作 | 出处/确证度 |
| --- | --- | --- |
| 删除处理器子键（先导出 .reg） | 子键没了即不再加载；恢复=重导入 | 通用工具操作；等效于社区清理教程通行做法 [来源 47] |
| 默认值改成无效 CLSID | Shell 找不到 COM 类即跳过 | 社区通行理解，官方未记载，**未确证（官方）**；**本机存在两例真实样本**：`*\…\AccExt` 与 `AllFilesystemObjects\…\ModernSharing` 的默认值均被改为尾随 `-` 的无效 CLSID（详见本机采样） |
| 写入 `Blocked` 键 | 值名 = CLSID 即全局禁止加载（跨挂载点） | 微软 Q&A 实测（7-Zip 被 Block 的案例）[来源 42]；专文未确证 |
| 处理器子键下加 `SuppressionPolicy`（REG_DWORD） | 策略 GUID 关联的策略启用时抑制该处理器；实测样本：`{20D04FE0-…}\shell\Manage` 上 `SuppressionPolicy=dword:4000003c`、另一例 `dword:80` [来源 43][44] | 值与用法来自微软 Q&A 实例，**官方文档未逐项确证** |
| 动词键 ACL 拒绝 | 同 2.1 | **未确证** |

**恢复**：重导入 .reg / 重装软件 / `regsvr32` 自注册（Notepad++ 手册即要求安装器卸载时执行 `regsvr32 /u`、安装时注册 [来源 35]）；移出 Blocked 键后重启 Explorer。

### 2.3 RunAs（以管理员身份运行）

- `runas` 是规范动词：官方 ShellExecute 文档定义其语义「以管理员身份启动应用，UAC 弹出同意/凭据提示」；动词本身仍来自注册表（`HKCR\<对象>\shell\runas\command`、虚拟对象为 `HKCR\CLSID\{clsid}\Shell\runas`）[来源 8][9]。
- 新增：给某对象补 runas 动词 = 在挂载点 `shell` 下建 `runas` 动词键 + `command`（系统会自动渲染盾牌/本地化文本，因为 runas 是规范动词，不需显示名 [来源 1]）。快速样本：微软 Q&A 中 `CLSID\{20D04FE0-…}\shell\Manage` 修复样本展示了动词键 + `command` + `HasLUAShield` 组合 [来源 43]。
- `HasLUAShield`（REG_SZ，无数据）：官方在 AQS 小节定义「显示 UAC 盾牌图标」（`HasLUAShield` 作为值配合 `AppliesTo`）；作为无条件静态值直接写在动词键上属社区通行写法 [来源 1][44]。
- 移除：删除 `runas` 动词子键即可（恢复=重导入）。
- 快捷方式的「以管理员身份运行」：挂在 `lnkfile`（及兼容性设置层），注册表层面同上（`lnkfile` 定义见 [来源 23]）。**本机实测：`HKCR\lnkfile\shell` 不存在**（lnkfile 键只有 `(default)`/`EditFlags`/`FriendlyTypeName`/`IsShortcut`/`NeverShowExt` 值），可观察的静态 runas 动词在 `exefile\shell\runas`；快捷方式的 runas 来自规范动词合并与兼容性设置层。

### 2.4 SkipExplorerPage

**未确证**。多轮检索（web 检索 + 微软文档站内）未找到任何名为 `SkipExplorerPage` 的注册表值/键的官方或可靠社区出处；**本机采样亦在全部主挂载点（`*`、`AllFilesystemObjects`、`Directory`、`Directory\Background`、`Desktop`、`DesktopBackground`、`Drive`、`Folder`、`lnkfile`、`exefile`、`txtfile`）递归深度 3 的所有值名中扫描，0 命中**。结论：查无出处且本机不存在，应从工单术语表移除该名称。

### 2.5 策略级全局开关

| 策略 | CSP 路径 | ADMX 映射 | 效果 |
| --- | --- | --- | --- |
| `NoViewContextMenu` | `./User/.../ADMX_WindowsExplorer/NoViewContextMenu`（仅用户配置） | WindowsExplorer.admx；注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer`，值 `NoViewContextMenu`；友好名「Remove File Explorer's default context menu」[来源 18] | 禁用 Explorer 默认右键菜单（文件/文件夹等） |
| `NoTrayContextMenu` | `./Device` 或 `./User` 下的 `.../ADMX_StartMenu/NoTrayContextMenu` | StartMenu.admx；同一 Policies\Explorer 键；「Remove access to the context menus for the taskbar」[来源 19] | 禁用任务栏/开始按钮右键菜单 |
| Start `DisableContextMenus` | `./Device` 或 `./User` 下的 `.../Policy/Config/Start/DisableContextMenus` | GPO「Disable context menus in the Start Menu」[来源 21] | 禁用开始菜单右键 |
| `EnforceShellExtensionSecurity` | （ADMX 政策）`…\Policies\Explorer` | 配合 Approved 名单强制校验；XP 生效，Win7+ 实测失效 [来源 41] | 历史机制，管理器可作为兼容知识保留 |

两策略均在 ADMX-backed Policy CSP 索引中收录 [来源 20]。**管理器在判定「菜单为什么空/条目为什么不显示」时，应把策略键排在逐条目检查之前。**

### 2.6 与 Windows 11 新版菜单的边界

- Win11 新菜单的条目来自 `IExplorerCommand`（含 `EnumerSubCommands`），打包应用用 `desktop4:FileExplorerContextMenus`/MSIX 清单注册，传统 IContextMenu DLL 在打包后需 `com:`/`windows.comServer` 清单扩展才能继续工作 [来源 1][22]。这部分机制归工单 T1。
- 社区广传的 `{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32`（空默认值）键用于永久切回经典菜单——属于新菜单开关而非经典条目管理，本文仅作边界提示 [来源 45][46]。
- Notepad++ 的 NppShell 同时实现了 `ModernEditWithNppExplorerCommandHandler`（新菜单条目）与经典菜单注册，是两套机制并存的实际样本 [来源 34]。

---

## 3. 常见第三方软件的典型落点

> 依据：安装器/脚本、源码仓库、官方手册或可信技术站；本机实际观测由主 agent 补充，此处不写本机数据。

### 3.1 7-Zip

| 项 | 内容 |
| --- | --- |
| 机制 | 单一 shellex 处理器（`7-Zip Shell Extension`），菜单项由 DLL 动态生成（非静态 verb）[来源 30] |
| CLSID | `{23170F69-40C1-278A-1000-000100020000}` [来源 29][30][31] |
| 落点 | `HKLM\SOFTWARE\Classes` 下多挂载点 `shellex\ContextMenuHandlers\7-Zip`：`*`（所有文件）、`Directory`、`Folder`、`Drive` 等文件夹侧挂载（Scoop 的官方化注册脚本即按此写入并注册 CLSID [来源 29]；NanaZip issue 中亦以该键集合作为参照 [来源 31]） |
| 视图 | HKLM（机器级安装器写入）；scoop 等用户级安装需手动补键 [来源 29][31] |

### 3.2 WinRAR

| 项 | 内容 |
| --- | --- |
| 机制 | shellex 处理器 `rarext.dll`；双 CLSID 并存：`{B41DB860-64E4-11D2-9906-E49FADC173CA}`（64 位，子键 `WinRAR`）与 `{B41DB860-8EE4-11D2-9906-E49FADC173CA}`（32 位，子键 `WinRAR32`）[来源 32][33] |
| 落点（实测样本） | `*\shellex\ContextMenuHandlers\WinRAR(+WinRAR32)`、`lnkfile\shellex\…`、`Folder\shellex\…`、`Directory\shellex\…`；另在压缩包 ProgID（`WinRAR`、`WinRAR.ZIP`）下以 CLSID 命名的处理器子键出现 [来源 32][33] |
| 视图 | `HKLM\SOFTWARE\Classes` [来源 32] |
| 备注 | `Drive` 挂载在本轮样本中未直接见到——**未确证**；本机未安装 WinRAR，无法本机核验 |

### 3.3 Notepad++

| 项 | 内容 |
| --- | --- |
| 机制 | 官方手册：壳扩展 DLL，v8.5.1 前为安装目录下 `NppShell_##.dll`，之后为 `contextMenu\NppShell.dll`；经 `regsvr32` 自注册/反注册（安装器卸载时必须先反注册再删文件）[来源 35][34] |
| 落点 | regsvr32 自注册 → `HKCR\*\shellex\ContextMenuHandlers`（确切子键名手册未列出，源码仓库含 `RegistryKey.cpp`，可由本机采样核对）；GitHub issue 中官方认可的静态替代写法为 `HKCR\*\shell\EditWithNpp` [来源 36] |
| 新菜单并存 | 同一 DLL 含 `ModernEditWithNppExplorerCommandHandler`（Win11 新菜单条目，归 T1）[来源 34] |

### 3.4 VS Code

| 项 | 内容 |
| --- | --- |
| 机制 | 安装器勾选项「Add to context menu / Open with Code」写入静态动词（社区教程与安装体验一致确认 [来源 39]） |
| 落点（社区模式） | 文件：`*\shell\<verb>`；目录：`Directory\shell\<verb>`；目录背景：`Directory\Background\shell\<verb>`；`command` 默认值 = `"…\Code.exe" "%1"`，`Icon` 指向 Code.exe [来源 39][40] |
| 视图 | 用户级安装应落 HKCU、系统级落 HKLM——官方未公开键位文档，**确切键名与视图未确证**；本机装了 VS Code 但未安装「Open with Code」右键项（合并视图各挂载点均无 VS Code 动词），仅有 `VSCode.<ext>` 系列 ProgID（HKCU+HKLM 均有）用于「打开方式」关联 |

### 3.5 微信（WeChat）

**未确证**。本轮检索未找到微信 PC 端右键菜单注册（挂载点/CLSID/键名）的一手或可靠二手资料；中文社区文章仅覆盖通用清理路径（`*\shellex`、`Directory\shellex` 等）[来源 47]。结论留待主 agent 本机采样（重点检查 `*\shellex\ContextMenuHandlers`、`Directory(\Background)\shell` 与 `WeChat`/`Weixin` 相关 CLSID）。**本机采样：微信未安装**，HKCU/HKLM 指纹扫描（`wechat`/`微信`）0 命中，无法本机佐证。

### 3.6 Office / OneDrive

| 软件 | 落点 |
| --- | --- |
| OneDrive（消费版） | shellex 处理器 `FileSyncEx`，CLSID `{CB3D0F55-BC2C-4C1A-85ED-23ED75B5106B}`，挂在 `*\shellex\ContextMenuHandlers\`（"Move to OneDrive" 等）；OneDrive for Business 走同族另一 CLSID [来源 37][38] |
| Office | 本轮未找到 Office 自身菜单条目的可靠键位资料——**未确证**；本机未安装 OneDrive（`FileSyncEx`={CB3D0F55…} 处理器在全部挂载点指纹扫描 0 命中）；现代 Office/OneDrive 的「始终保留在此设备」等条目已迁往 Win11 新菜单机制（IExplorerCommand），归 T1 边界 [来源 22] |

---

## 4. 从注册表推导用户可见属性

### 4.1 显示名称

| 优先级/来源 | 机制 | 出处 |
| --- | --- | --- |
| `MUIVerb`（REG_SZ） | 级联菜单小节官方定义「作为其在菜单上的名字」；也是间接字符串的常规入口 [来源 1] | 官方 |
| 动词键默认值（REG_SZ） | 静态动词小节：动词子键需有 REG_SZ 默认值作显示文本 [来源 1] | 官方 |
| 规范动词本地化 | open/print/edit… 不写显示名时系统自动给本地化文本 [来源 1] | 官方 |
| 间接字符串 `@dll,-resourceID` | `SHLoadIndirectString`：`@文件名,资源` 形式；数值 ≥0 为字符串表索引，负数为资源 ID；支持 `;v2` 版本修饰维护 MUI 缓存 [来源 14][15]（实测样本：`MUIVerb1="@shell32.dll,-30329"` [来源 27]） | 官方 API 文档 |
| `MUIVerb > 默认值 > 动词键名` 的优先级序列 | 社区共识/实测行为，官方文档未明确逐级规定——**未确证（官方）** | — |

### 4.2 图标

- 动词键 `Icon` 值：格式 `<路径>,-索引` 或 `<路径>,索引`；索引为负表示资源 ID（与间接字符串同规则）。实测样本：`"Icon"="imageres.dll,-5302"` [来源 27]；教程样本 `"Icon"="C:\Program Files\Microsoft VS Code\Code.exe,0"` [来源 40]。
- 文件类图标走 ProgID 下 `DefaultIcon` 键（决定资源管理器中文件图标，而非菜单项图标）；其官方专文本轮未直接核验——**DefaultIcon 细节未确证（官方）**，菜单图标以上述 `Icon` 值为準。
- `Icon` 支持引用带参资源（如 `%1` 指向的文件自身图标）属社区实践，未确证。

### 4.3 命令行（command 默认值与占位符）

- `command` 默认值 = 激活命令行；官方强调含空格元素必须加引号，`"%1"` 一类会被 Shell 展开的参数必须带引号 [来源 6][1]。
- 官方示例只明示 `%1`＝被操作文件、`%2`＝打印机名（printto）[来源 1][6]。
- MSI 安装的软件其动词下常另有 REG_MULTI_SZ `command` 值（Darwin 描述符，乱码样字符串）——Raymond Chen 释疑：MSI 用于按需安装/路径重定位，默认值仅为兼容直读注册表的程序保留的近似路径 [来源 24]。管理器**不应把该值当作命令行展示或修改目标**。
- `%V / %L / %W / %D` 等占位符逐项语义（如 %V＝文件或目录（不定形态）、%L＝长路径、%W＝工作目录）为社区长期共识，官方文档未见逐项定义——**未确证（官方）**；解析时建议按「%1 同源的路径类参数」宽松处理。

### 4.4 适用场景（由挂载点决定，简表复述）

| 挂载点 | 右键对象 | 管理器「适用场景」推导 |
| --- | --- | --- |
| `*` / ProgID | 文件（ProgID 精确到类型） | 「文件」；ProgID 级可精确到「某类型文件」 |
| `AllFilesystemObjects` | 文件+目录 | 「文件与文件夹」 |
| `Folder` | 一切文件夹（含虚拟） | 「文件夹（含虚拟）」 |
| `Directory` | 文件系统目录 | 「文件夹」 |
| `Directory\Background` | 目录/资源管理器空白处 | 「目录背景」 |
| `DesktopBackground` | 桌面空白处 | 「桌面」 |
| `Drive` | 驱动器 | 「驱动器」 |
| `lnkfile` | 快捷方式 | 「快捷方式」 |
| `Applications\<exe>` | 「打开方式」列表 | 「打开方式注册」 |
| `CLSID\{…}` | 虚拟对象（此电脑等） | 「系统对象」 |

依据见 1.2/1.3 表 [来源 2][7][8]。

### 4.5 修饰值速查表

| 值 | 类型 | 含义 | 确证度与出处 |
| --- | --- | --- | --- |
| `Position` | REG_SZ | `Top`/`Bottom`，置于菜单顶/底；多个条目设置时最后一个生效 | 官方 [来源 1] |
| `Extended` | REG_SZ（无数据） | 仅 Shift+右键显示 | 官方 [来源 1] |
| `CommandFlags` | REG_DWORD | EXPCMDFLAGS 位掩码；`0x20`=ECF_SEPARATORBEFORE（仅顶层）、`0x40`=ECF_SEPARATORAFTER；详见 IExplorerCommand::GetFlags | 官方 [来源 1][17] |
| `MultiSelectModel` | REG_SZ | `Single`/`Document`/`Player`；不写则推断（COM 型=Player，其余=Document）；上限 Legacy: 15/100，COM: 15/无限 [来源 16] | 官方 [来源 1][16] |
| `NeverDefault` | REG_SZ | 社区共识：阻止该动词被选为默认动词 | **官方未确证**（本轮检索未见官方页面） |
| `LegacyDisable` | REG_SZ（无数据） | 隐藏条目（兼容性开关） | 社区定义 [来源 25] + 微软开发者社区实例 [来源 26] |
| `ProgrammaticAccessOnly` | REG_SZ（无数据） | 菜单不显示、编程可用 | 官方 [来源 1] |
| `HasLUAShield` | REG_SZ（无数据） | 显示 UAC 盾牌 | 官方（AQS 形态）[来源 1]；静态用法为社区实践 [来源 43][44] |
| `AppliesTo` / `DefaultAppliesTo` | REG_SZ | AQS 条件决定显隐/默认 | 官方 [来源 1] |
| `SubCommands` / `ExtendedSubCommandsKey` | REG_SZ | 级联子菜单定义 | 官方 [来源 1] |
| `SuppressionPolicy` | REG_DWORD | 关联策略启用时抑制条目 | 微软 Q&A 实例 [来源 43][44]；官方文档未确证 |
| `ExplorerFlags` | REG_DWORD | 社区见于个别菜单键 | **未确证（官方/语义均未确证）** |
| `HideBasedOnVelocityId` | REG_DWORD | 速度（Velocity）功能 ID 命中时隐藏；实测 `0x639bc8` 隐藏「复制文件地址/命令提示符」类条目，第三方菜单实现（Directory Opus）不受其影响 [来源 27][28] | 社区实测；**官方未确证** |
| `DefaultIcon` | 键（ProgID 下） | 文件类图标 | 官方专文本轮未核验——**未确证（官方）** |
| `MayChangeDefaultMenu` | REG_SZ | 声明 ProgID 级处理器可能改默认菜单 | 官方 [来源 5] |

### 4.6 出错与边界注意事项

- **写 HKCU/HKLM 而非 HKCR**：官方要求交互用户设置写 `HKCU\Software\Classes`；HKCR 是合并视图，直接写会落到 HKCU 且可能被「HKCU 赢者通吃」规则遮蔽 HKLM 同名条目 [来源 10][11][12]。
- **权限**：HKLM 侧与 CommandStore 需管理员；官方推荐 HKCU 注册级联菜单以避免提权 [来源 1]。
- **刷新**：改注册后调用 `SHChangeNotify(SHCNE_ASSOCCHANGED)`，否则可能重启才生效 [来源 2]。
- **ThreadingModel**：处理器必须 `Apartment`，写错会间歇性崩溃 [来源 2]。
- **MSI 双 `command`**：REG_MULTI_SZ Darwin 描述符不是可编辑命令行 [来源 24]。
- **策略优先**：`NoViewContextMenu` 等策略直接整层关菜单，先查策略再查条目 [来源 18][19]。
- **Blocked/Approved**：条目在挂载点完好仍可能被 Blocked 键拦截 [来源 42][43]。
- **打包（MSIX）应用**：传统 IContextMenu DLL 在打包后需清单声明才被加载；新条目建议 IExplorerCommand + `desktop4:FileExplorerContextMenus`——管理器对这类条目应标记「注册源为清单而非注册表」[来源 22]。
- **UWP/打包应用例外**：见上条；普通 UWP 应用不经这些注册表挂载点添加菜单（未找到进一步官方细则，未确证）。

---

## 本机注册表采样佐证

### 采样方法与环境

- 环境：Windows 11（build 26100.8894 / 24H2；注册表 `ProductName` 字段为历史遗留的 "Windows 10 Enterprise LTSC 2024" 字样）。采样日期 2026-08-30。
- 全程只读（PowerShell `Get-Item` / `Get-ChildItem` / `Get-ItemProperty`），未做任何注册表写入。
- 覆盖：`*`、`AllFilesystemObjects`、`Directory`、`Directory\Background`、`Desktop`、`DesktopBackground`、`Drive`、`Folder`、`lnkfile`、`exefile` 下的 `shell` 与 `shellex\ContextMenuHandlers`；HKCU/HKLM 指纹扫描（7-zip / winrar / notepad / vscode / wechat / 微信 / onedrive / filesyncex）；`SkipExplorerPage` 值名递归扫描（深度 3）。

### 1）挂载点实测（与 §1.2/§1.3 互证）

- `HKCR\Desktop\shell` 与 `HKCR\Desktop\shellex\ContextMenuHandlers` **不存在**；`HKCR\DesktopBackground\{Shell,shellex}` 存在，其中 `shellex\ContextMenuHandlers\DesktopSlideshow`（={0bf754aa-c967-445c-ab3d-d8fda9bae7ef}）→ 桌面菜单的实际挂载点就是 `DesktopBackground`，印证 §1.2 官方表。
- `HKCR\lnkfile\shell` **不存在**；lnkfile 键仅有 `(default)`、`EditFlags`、`FriendlyTypeName`、`IsShortcut`、`NeverShowExt` 值 → 快捷方式的菜单动词来自规范动词合并/兼容性层，而非 lnkfile 下的静态键。
- `Directory\Background\shellex\ContextMenuHandlers` 实测：`New`（={D969A300-E7FF-11d0-A93B-00A0C90F2719}）、`NvAppDesktopContext`、`NvCplDesktopContext`、`Sharing`、`WorkFolders` → 「新建」菜单本身就是 Background 挂载点下的 shellex 处理器。

### 2）shellex 处理器清单摘录（HKCR 合并视图）

| 挂载点 | 处理器子键（默认值=CLSID） |
| --- | --- |
| `*` | AccExt、cloudmusic（网易云音乐）、HRShredShell、Open With、EncryptionMenu、Sharing、WorkFolders、{90AA3A4E…}（Taskband Pin）、{a2a9545d…}（Start Menu Pin） |
| `AllFilesystemObjects` | CopyAsPathMenu、HRShredShell、ModernSharing、SendTo、{474C98EE…}、{596AB062…}、{a2a9545d…} |
| `Directory` | EncryptionMenu、HRShredShell、Offline Files、Sharing、WorkFolders、{596AB062…} |
| `Directory\Background` | New、NvAppDesktopContext、NvCplDesktopContext、Sharing、WorkFolders |
| `Drive` | EnhancedStorageShell、HRShredShell、Sharing、{596AB062…}、{D6791A63…}、{fbeb8a05…} |
| `Folder` | AccExt、Library Location、Offline Files、PintoStartScreen、{a2a9545d…} |
| `lnkfile` | NvAppShExt、OpenContainingFolderMenu、OpenGLShExt、{00021401…} |
| `exefile` | Compatibility、NvAppShExt、OpenGLShExt、PintoStartScreen |
| `DesktopBackground` | DesktopSlideshow |

观察：

- **「无效 CLSID 禁用」手法两例活样本**：`*\…\AccExt` 默认值 `{2A118EB5-5797-4F5E-8B3D-F4ECBA3C98E4-}`、`AllFilesystemObjects\…\ModernSharing` 默认值 `{e2bf9676-5f8f-435c-97eb-11607a5bedf7-}` 均为尾随 `-` 的无效 CLSID，且 `HKCR\CLSID` 下查无该键 → 印证 §2.2「改默认值为无效 CLSID」是现实中真实存在的禁用形态。
- **处理器子键的另一种形态**：`Drive\shellex\ContextMenuHandlers\{596AB062-B4D2-4215-9F74-E9109B0A8153}` 默认值为空、无任何值——子键名本身就是有效 CLSID（`HKCR\CLSID` 下存在）→ 「CLSID 作子键名 + 空默认值」的注册形态实测存在。
- `HKLM\…\Shell Extensions\Blocked` 键不存在；`Approved` 名单 32 条；`NoViewContextMenu`/`NoTrayContextMenu` 策略值均未设置 → 本机不处于策略/黑名单拦截状态。

### 3）静态动词实测（HKCR 合并视图）

- **RunAs**：`exefile\shell` 实测 `open`、`runas`、`runasuser`（后者默认值=`@shell32.dll,-50944`）——规范动词不写显示名、由系统本地化的直接例证。
- 修饰值逐项命中：
  - `HideBasedOnVelocityId=6527944`（=0x639BC8，与来源 27 记载同值）：`Directory\shell\cmd`、`Directory\Background\shell\cmd`、`Drive\shell\cmd`（隐藏「在此处打开命令窗口」）。
  - `Position='Bottom'`：`*\shell\UpdateEncryptionSettingsWork`、DesktopBackground 的 `Display`/`Personalize`/`.Spotlight*`。
  - `ProgrammaticAccessOnly`：`*\shell\removeproperties`（值数据为 `Apartment`——该值可携带数据，非必须空值）。
  - `MultiSelectModel`：`Player`（OppoConnectShare）、`Document`（`Folder\shell\open` 等）、`Single`（Drive 的 BitLocker 动词族）。
  - `Icon` 两种形态并存：纯路径（`C:\Program Files\PowerShell\7\pwsh.exe`）与 `路径,-资源ID`（`C:\WINDOWS\System32\display.dll,-1`）。
  - 间接字符串名称（MUIVerb/默认值）：`@shell32.dll,-8506`（cmd）、`@efscore.dll,-101`、`@%SystemRoot%\system32\WorkfoldersControl.dll,-1`（含环境变量的间接字符串实测有效）、`@C:\WINDOWS\System32\fvewiz.dll,-920`（BitLocker）。
  - `command` 占位符实测：`cmd.exe /s /k pushd "%V"`（Background/Directory/Drive 的 cmd 用 `%V`）、`"D:\Git\git-bash.exe" "--cd=%1"`（Directory 用 `%1`）、`"%1" %*`（exefile\open）。
- **`LegacyDisable` 活样本**：`HKCU\Software\Classes\Directory\shell\OppoConnectShare`（MUIVerb=「使用 OPPO 互联分享」+`Icon`+`MultiSelectModel=Player`）上存在空 `LegacyDisable`（REG_SZ，无数据）→ §2.1 隐藏手法在真实机器上的实例；该动词为「嵌套 shell 子键」式级联（`shell\ShareToDevice`，MUIVerb=「连接设备」），自身无 `command` 子键、未检出 `DelegateExecute`。

### 4）HKCU/HKLM 分工实测（与 §1.4 合并规则互证）

- `HKCU\Software\Classes\Directory\shell\OppoConnectShare`：用户级软件写 HKCU 的实例；同一键在合并视图 `HKCR\Directory\shell`、`*\shell`、`AllFilesystemObjects\shell` 中均可见。
- `HKLM\SOFTWARE\Classes\Directory\shell` 子键：`cmd`、`find`、`git_gui`、`git_shell`、`Powershell`、`PowerShell7x64`、`UpdateEncryptionSettings`；`Directory\Background\shell`：`cmd`、`git_gui`、`git_shell`、`Powershell`、`PowerShell7x64` → Git for Windows 与 PowerShell 7（MSI）的菜单写在 HKLM 侧。
- `HKCU\Software\Classes\Directory\Background\shell` 不存在 → 本机 Background 挂载点无用户级条目。

### 5）第三方软件本机现状（补 §3 核验结论）

| 软件 | 本机状态 | 观测 |
| --- | --- | --- |
| 7-Zip | 未安装 | 安装目录、CLSID {23170F69-…}、`7-Zip` 处理器子键均不存在 |
| WinRAR / Notepad++ | 未安装 | 安装目录与 HKCU/HKLM 指纹 0 命中 |
| VS Code | 已装，未装右键项 | HKCU+HKLM 均有 `VSCode.<ext>` 系列 ProgID（百余个扩展名，供「打开方式」）；合并视图 `*\shell`、`Directory\shell`、`Directory\Background\shell` 无任何 VS Code 动词 → 右键项是独立安装选项且只写静态动词 |
| 微信 / OneDrive / Office | 未安装 | `wechat`/`微信`/`filesyncex` 指纹 0 命中；OneDrive `FileSyncEx` CLSID {CB3D0F55-…} 在 `HKCR\CLSID` 亦不存在 |
| 本机实测的其他第三方 | — | 网易云音乐（cloudmusic，`*\shellex`）；文件粉碎工具（HRShredShell，`*`/`AllFilesystemObjects`/`Directory`/`Drive` 四挂载点 shellex）；NVIDIA（`Directory\Background\shellex` 两条 + `lnkfile`/`exefile\shellex` 各两条）；OPPO 互联分享（HKCU 静态动词，已带 LegacyDisable）；Git for Windows 与 PowerShell 7（HKLM 静态动词） |

---

## Sources

1. [Creating Shortcut Menu Handlers — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/context-menu-handlers)：规范/扩展/编程专用动词、静态动词结构、Position、级联菜单（SubCommands/ExtendedSubCommandsKey/CommandStore）、CommandFlags=EXPCMDFLAGS（0x20/0x40）、MultiSelectModel、HasLUAShield（AQS）、DesktopBackground 示例。
2. [Registering Shell Extension Handlers — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/reg-shell-exts)：CLSID/InprocServer32/Apartment 要求、处理器子键命名规则、预定义 Shell 对象挂载点表、SHChangeNotify、MSIX 提示。
3. [Creating Shell Extension Handlers（handlers）— Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/handlers)：处理器注册总览与「Verbs = HKCR\Subkey\Shell\Verb」定义。
4. [Extending Shortcut Menus — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/context)：默认动词选择顺序、预定义对象动词挂载。
5. [Customizing a Shortcut Menu Using Dynamic Verbs — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/shortcut-menu-using-dynamic-verbs)：动态动词、MayChangeDefaultMenu。
6. [Verbs and File Associations — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/fa-verbs)：verb 定义、command 引号规则、Applications/FriendlyAppName。
7. [Application Registration — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/app-registration)：SystemFileAssociations 机制与关联数组优先级、OpenWith 的 shell\verb 表。
8. [Launching Applications (ShellExecute, ShellExecuteEx…) — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/launch)：runas 动词定义、虚拟对象 `CLSID\{clsid}\Shell\verb` 查找。
9. [ShellExecuteA function — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutea)：runas=以管理员启动、UAC 提示；默认动词回退顺序。
10. [HKEY_CLASSES_ROOT Key — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/sysinfo/hkey-classes-root-key)：HKCR=HKLM+HKCU 合并；交互用户改动应写 HKCU\Software\Classes。
11. [Merged View of HKEY_CLASSES_ROOT — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/sysinfo/merged-view-of-hkey-classes-root)：合并三规则、按键级 HKCU 优先的共有键清单、UAC 禁用时忽略 per-user COM。
12. [Windows registry information for advanced users — Microsoft Learn](https://learn.microsoft.com/en-us/troubleshoot/windows-server/performance/windows-registry-advanced-users)：HKLM=默认、HKCU=覆盖、HKCR=合并视图。
13. [View registry keys with 64-bit versions of Windows — Microsoft Learn](https://learn.microsoft.com/en-us/troubleshoot/windows-client/performance/view-system-registry-with-64-bit-windows)：WOW64 注册表视图、WOW6432Node、Registry Reflector 只镜像 COM 激活数据。
14. [SHLoadIndirectString function — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/shlwapi/nf-shlwapi-shloadindirectstring)：`@文件,-ID` 间接字符串解析规则、负数=资源 ID、`;v2` 版本修饰。
15. [Shell String Handling Functions — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/shlwapi-string)：SHLoadIndirectString 在 Shell 字符串函数族中的定位。
16. [How to Employ the Verb Selection Model — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/how-to-employ-the-verb-selection-model)：MultiSelectModel 取值、上限表、推断规则。
17. [IExplorerCommand::GetFlags — Microsoft Learn](https://learn.microsoft.com/en-us/windows/desktop/api/shobjidl_core/nf-shobjidl_core-iexplorercommand-getflags)：EXPCMDFLAGS/ECF 标志官方参考。
18. [Policy CSP — ADMX_WindowsExplorer — Microsoft Learn](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-windowsexplorer)：NoViewContextMenu 映射（Policies\Explorer、WindowsExplorer.admx、仅用户配置）。
19. [Policy CSP — ADMX_StartMenu — Microsoft Learn](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-startmenu)：NoTrayContextMenu 映射（任务栏右键、Device+User）。
20. [ADMX-backed policies in Policy CSP — Microsoft Learn](https://learn.microsoft.com/en-us/windows/client-management/mdm/policies-in-policy-csp-admx-backed)：NoViewContextMenu/NoTrayContextMenu 收录于 ADMX 策略索引。
21. [Start menu policy settings — Microsoft Learn](https://learn.microsoft.com/en-us/windows/configuration/start/policy-settings)：Start/DisableContextMenus（开始菜单右键）CSP/GPO。
22. [Support legacy context menus — MSIX — Microsoft Learn](https://learn.microsoft.com/en-us/windows/msix/packaging-tool/support-legacy-context-menus)：打包应用 IContextMenu 需清单声明；推荐 IExplorerCommand + desktop4:FileExplorerContextMenus。
23. [IsShortcut（Links/Shell Links）— Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/shell/links)：lnkfile 的 IsShortcut 条目说明。
24. [What is the strange garbage-looking string in the "command" value of a static verb? — The Old New Thing](https://devblogs.microsoft.com/oldnewthing/20140731-00?p=363)：REG_MULTI_SZ command=Darwin 描述符（Raymond Chen）。
25. [In the Windows Registry, what is the "LegacyDisable" string value — Super User](https://superuser.com/questions/1183842/in-the-windows-registry-what-is-the-legacydisable-string-value-and-what-exact)：LegacyDisable＝让 Shell 忽略该键（社区）。
26. [Disable context menu for "Open in Visual Studio" — Microsoft Developer Community](https://developercommunity.visualstudio.com/t/Disable-context-menu-for-Open-in-Visual/26397)：用 LegacyDisable 隐藏 `Directory(\Background)\shell\AnyCode` 并给出恢复 .reg（实例）。
27. [Right-click menu ignore HideBasedOnVelocityId — Directory Opus 论坛](https://resource.dopus.com/t/right-click-menu-ignore-hidebasedonvelocityid-registry-entry/52759)：`HideBasedOnVelocityId=0x639bc8` 在 Explorer 隐藏 copy-path 条目；含 Icon/MUIVerb 样例。
28. [Why is hidebasedonvelocityid flag set in registry entry? — Super User](https://superuser.com/questions/1152821/why-is-hidebasedonvelocityid-flag-set-in-registry-entry)：HideBasedOnVelocityId 社区讨论。
29. [7-zip install-context.reg — ScoopInstaller/Main（GitHub）](https://github.com/ScoopInstaller/Main/blob/master/scripts/7-zip/install-context.reg)：7-Zip shellex 注册脚本，CLSID `{23170F69-40C1-278A-1000-000100020000}`（"7-Zip Shell Extension"）与文件夹侧挂载。
30. [Where in the registry are the context menu options for 7zip? — Super User](https://superuser.com/questions/1692977/where-in-the-registry-are-the-context-menu-options-for-7zip)：7-Zip 菜单由该 CLSID DLL 动态生成（非静态）。
31. [No Context Menu in Scoop Install — NanaZip issue #787（GitHub）](https://github.com/M2Team/NanaZip/issues/787)：以 7-Zip 键集合为参照的 shellex 注册清单。
32. [WinRAR option not showing in right click menu — Ten Forums](https://www.tenforums.com/software-apps/152228-winrar-option-not-showing-right-click-windows-explorer-menu.html)：WinRAR 双 CLSID（WinRAR/WinRAR32）在 `*`、`lnkfile`、`Folder` 等挂载点的 .reg 实录。
33. [Strange registry-editing thing — Icrontic](https://icrontic.com/discussion/69965/strange-registry-editing-thing-dangerous-or-not)：rarext.dll 与 WinRAR CLSID、Folder/Directory 挂载实测。
34. [notepad-plus-plus/nppShell（GitHub）](https://github.com/notepad-plus-plus/nppShell)：NppShell.dll regsvr32 注册/反注册要求；含 Win11 新菜单 IExplorerCommand 处理器源码。
35. [Right Click - Edit With Notepad++ — Notepad++ 官方手册](https://npp-user-manual.org/docs/shell-extension)：NppShell DLL 位置/版本演变（8.5.1 前后）与安装/卸载命令。
36. [Use registry for Explorer context menu instead of NppShell — notepad-plus-plus issue #92（GitHub）](https://github.com/notepad-plus-plus/notepad-plus-plus/issues/92)：静态注册替代写法 `HKCR\*\shell\EditWithNpp`。
37. [Change mnemonic for "move to OneDrive" — Super User](https://superuser.com/questions/1696510/change-mnemonic-for-move-to-onedrive-in-windows-explorer-context-menu)：OneDrive `FileSyncEx`={CB3D0F55-BC2C-4C1A-85ED-23ED75B5106B} 挂在 `*\shellex\ContextMenuHandlers`。
38. [Missing OneDrive for Business Context Menu — Directory Opus 论坛](https://resource.dopus.com/t/missing-onedrive-for-business-context-menu/37759)：OneDrive/OneDrive for Business 处理器 CLSID 佐证。
39. [Right click on Windows folder and open with Visual Studio Code — thisDaveJ](https://thisdavej.com/right-click-on-windows-folder-and-open-with-visual-studio-code)：VS Code 安装器「Open with Code」勾选项与手动 .reg 模式（社区）。
40. [How to open your files with VS Code from the context menu — DEV Community](https://dev.to/matheusgomes062/how-to-open-your-files-with-vs-code-from-the-context-menu-on-windows-5fi9)：`*\shell`、`Directory\shell` 静态动词 + Icon + command 样例（社区）。
41. [The policy EnforceShellExtensionSecurity — Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/4315201/the-policy-enforceshellextensionsecurity)：Approved 名单与该策略在 XP 拦截 7-Zip、Win7+ 失效的实测。
42. [Windows 10 context menu problem for Shell and Shellex — Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/3900877/windows-10-context-menu-problem-for-shell-and-shel)：用 `Shell Extensions\Blocked` 键排查/解除菜单缺失（实例）。
43. [win+E 组合键修复（Manage 动词样本）— Microsoft Q&A（中文）](https://learn.microsoft.com/zh-cn/answers/questions/2454051/win-e-c-windowsexplorer-exe-explorer-exe)：CLSID\{20D04FE0…}\shell\Manage 的 `command`+`SuppressionPolicy`+`HasLUAShield` 实录。
44. [「该文件没有与之关联的程序…」问答（SuppressionPolicy 样例）— Microsoft Q&A](https://learn.microsoft.com/es-es/answers/questions/2496039/this-file-does-not-have-a-program-associated-with)：`SuppressionPolicy=dword:80` 等实例。
45. [6+ Ways to Use Win10 Context Menu in Win11 — ii.com](https://www.ii.com/6plus-ways-to-use-win10-context-menu-in-win11)：`{86ca1aa0-…}\InprocServer32` 经典菜单开关（社区，边界参考）。
46. [How to Get the Old Context Menu Back in Windows 11 — Ascomp 博客](https://www.ascompsoftware.com/blog/en/2024/12/30/how-to-get-the-old-context-menu-back-in-windows-11)：同上开关的另一种社区表述（边界参考）。
47. [右键菜单的注册表位置（中文社区）— 阿里云开发者社区](https://developer.aliyun.com/article/256250)：通用清理路径汇总（`*\shellex`、`Directory(\Background)\shellex` 等），仅作补充性质。
