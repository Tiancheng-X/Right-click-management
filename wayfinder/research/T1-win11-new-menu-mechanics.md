# T1 调研报告：Win11 新版（折叠）右键菜单机制全景

- 工单：`wayfinder/tickets/T1-win11-new-menu-mechanics.md`
- 日期：2026-08-30
- 方法：全部结论由 web 检索一手资料（Microsoft Learn、Windows 官方博客、DevBlogs）+ 可信技术文章交叉验证；关键注册表事实在本机（Windows 11 24H2，Build 26100.8894，Enterprise LTSC 2024）用只读 `reg query` 实测复核。凡未查到一手依据的点，均在正文明确标注「未确证」。

---

## 0. TL;DR

1. Win11 右键菜单是**双层结构**：新版（折叠）菜单只显示「系统内置命令 + 静态 shell 动词 + 文件关联（打开/打开方式）+ 云文件命令 + **IExplorerCommand + 包标识**的打包扩展」；旧版 `IContextMenu` COM 处理器全部被折叠进「显示更多选项」（即经典菜单）。没有任何条目被彻底移除（官方博客原话）。
2. 枚举某场景全部条目没有公开的单一 API，只能**分来源聚合**：静态动词与 shellex 处理器扫注册表（HKCR 合并视图），打包条目读已安装 MSIX 的 `AppxManifest.xml`（`desktop4:FileExplorerContextMenus`）；系统内置项（剪切/复制/粘贴等）不可枚举也不可屏蔽（官方未提供任何键）。
3. 逐项屏蔽的主流手段是 `Shell Extensions\Blocked` 键（HKCU 免管理员 / HKLM 全机），值为 CLSID、数据留空，重启 Explorer 生效；它对经典 COM 处理器确证有效，对 MSIX/IExplorerCommand 打包条目也实测有效（Winaero「Edit in Notepad」、gist「Open in Terminal」）但微软未文档化。副作用是**按 CLSID 全局生效**，且误屏蔽共享 CLSID 会连带破坏其他 shell 功能。
4. 「一键切回经典菜单」= 在 `HKCU\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32` 建**空字符串默认值**（必须是空串而非「数值未设置」），覆盖 HKLM 中指向 `Windows.UI.FileExplorer.dll` 的注册，使新菜单 COM 激活失败、Explorer 回退经典实现；需重启 explorer。22H2/23H2/24H2/25H2 稳定版均实测可用（本机 24H2 正在生效中），但属**非官方支持**手段，Insider 通道已出现过反复，未来随时可能失效。
5. 第三方把命令加进新菜单的唯一正路：`IExplorerCommand` 原生 COM DLL + 包标识（完整 MSIX 或 **sparse package** 仅授予标识），清单用 `windows.comServer` + `desktop4:FileExplorerContextMenus` 注册；有「每处理器最多 16 项（社区实测、未文档化）」等限制。v1 不做，仅记录。

---

## 1. 新版菜单的条目来源与识别

### 1.1 官方定义的新菜单构成

来源：[Extending the Context Menu and Share Dialog in Windows 11（Windows Developer Blog, 2021-07-19）](https://blogs.windows.com/windowsdeveloper/2021/07/19/extending-the-context-menu-and-share-dialog-in-windows-11/)。自上而下：

| 区块 | 内容 | 来源性质 |
|---|---|---|
| 顶部常用命令 | 剪切、复制、粘贴、删除、重命名等 | 系统内置（无注册表键可查） |
| 打开 / 打开方式 | 成组排列 | 文件类型关联（ProgID + 关联注册） |
| Shell 动词区 | 静态动词（如「在终端中打开」一类注册动词也在这一层显示） | 注册表 `shell\<verb>` |
| 云文件命令 | 释放/ dehydration（OneDrive 等） | Cloud Files 提供程序注册 |
| 应用扩展区 | `IExplorerCommand` + 应用标识；>1 个动词时合并为**带应用署名的二级弹出（flyout）** | MSIX 清单（含 sparse） |
| 底部 | 「显示更多选项」→ 原样加载 Win10 经典菜单；Shift+F10 / 键盘菜单键等效 | 系统内置 |

设计动机（同一博客）：Win10 菜单「20 年无序生长」，`IContextMenu` 自 XP 引入以来命令混排、不可归属、在 Explorer 进程内运行引发性能/稳定性问题——这就是新菜单把旧 COM 扩展折叠起来的原因。

### 1.2 来源分类与识别方式

| 来源 | 注册/实现位置 | 在哪个菜单显示 | 识别方法 |
|---|---|---|---|
| 系统内置命令 | 无公开注册表 | 仅新菜单 | 排除法：不在下述任何注册位置 |
| 静态动词 | `HKCR\<场景>\shell\<verb>`、`HKCR\<ProgID>\shell\<verb>` | 新菜单 + 经典菜单都显示 | 有 `command` 子键 / `MUIVerb` / `DelegateExecute`；官方文档见 [Creating Shortcut Menu Handlers](https://learn.microsoft.com/en-us/windows/win32/shell/context-menu-handlers) |
| `IContextMenu` COM 处理器 | `HKCR\<场景或ProgID>\shellex\ContextMenuHandlers\<名>`，默认值=CLSID；`HKCR\CLSID\{clsid}\InprocServer32` → DLL 路径 | **仅经典菜单**（新菜单不加载） | ShellExView 可列全并显示 DLL；官方注册规则见 [Registering Shell Extension Handlers](https://learn.microsoft.com/en-us/windows/win32/shell/reg-shell-exts)（`ThreadingModel` 必须 `Apartment`） |
| `IExplorerCommand` 打包命令 | MSIX 清单 `windows.comServer`（com:Class CLSID）+ `desktop4:FileExplorerContextMenus` > `desktop5:ItemType`（`Type`=`*`/`Directory`/`Directory\Background`）> `desktop5:Verb` | 仅新菜单为主（见 1.3 注） | 读 `Get-AppxPackage` 各包的 `AppxManifest.xml`；官方文档 [Add a File Explorer context menu command](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/integrate-packaged-app-with-file-explorer) |
| 云文件提供程序命令 | Cloud Files filter/注册 | 新菜单（紧邻 Shell 命令） | 官方博客定位描述；细节键位本次未展开（未深查） |
| 「新建」子菜单 | `ShellNew` 键（`HKCR\.ext\ShellNew` 等） | 两个菜单 | 官方 context-menu-handlers 文档「Extending a New Submenu」一节 |
| 「发送到」 | `%APPDATA%\Microsoft\Windows\SendTo` 目录 | 两个菜单 | 目录枚举 |

官方对打包扩展的原文约束（integrate-packaged-app-with-file-explorer）：「`windows.fileExplorerContextMenus` 注册、`IExplorerCommand` 实现的命令出现在 Win11 新菜单；`IContextMenu` 实现只会出现在旧菜单」「GetTitle/GetIcon/GetState 在 shell UI 路径上执行，必须快，长操作放到 `Invoke`」。

### 1.3 关键行为细节

- **新菜单由 XAML Islands 实现**（[Mouri (NanaZip 作者) 实作记录](https://mouri.moe/en/2021/12/25/Share-my-experience-of-implementing-context-menu-support-for-File-Explorer-in-Windows-11)）。
- **`IExplorerCommand` 条目同时会出现在经典菜单**（多动词时级联）。Mouri 建议开发者只做级联模式以兼容旧菜单；Microsoft Q&A 的复现帖也证实 IExplorerCommand 注册的动词在两个菜单都出现（[示例帖](https://learn.microsoft.com/en-us/answers/questions/5979037/windows-11-classic-context-menu-weird-handling-of)）。
- **每个处理器最多 16 项**：Mouri 实测限制，微软无文档（标为社区实测结论）。分隔符不计数，可注册多个处理器绕过。
- 注册打包扩展后需要**重启 File Explorer（或注销）** 才会加载（官方文档原文）。

---

## 2. 枚举某场景下的全部条目并标明来源

### 2.1 场景 → 注册表位置矩阵

`HKCR` 是 `HKLM\Software\Classes` 与 `HKCU\Software\Classes` 的合并视图（HKCU 优先；官方 context-menu-handlers 文档明确 HKCR 为二者的组合）。官方「预定义 Shell 对象」表（[reg-shell-exts](https://learn.microsoft.com/en-us/windows/win32/shell/reg-shell-exts)）给出可挂动词/处理器的场景：

| 右键场景 | 静态动词 | COM 处理器 |
|---|---|---|
| 任意文件 | `HKCR\*\shell` | `HKCR\*\shellex\ContextMenuHandlers` |
| 所有文件系统对象 | `HKCR\AllFilesystemObjects\shell` | `...\shellex\ContextMenuHandlers` |
| 所有文件夹（文件+文件夹） | `HKCR\Folder\shell` | 同上 |
| 文件夹 | `HKCR\Directory\shell` | 同上 |
| 文件夹内空白处（背景） | `HKCR\Directory\Background\shell` | 同上（**仅快捷菜单处理器**） |
| 桌面背景 | `HKCR\DesktopBackground\shell`（Win7+） | 同上 |
| 驱动器 | `HKCR\Drive\shell` | 同上 |
| 特定文件类型 | `HKCR\<ProgID>\shell`（ProgID 由 `HKCR\.ext` 默认值解析，用户选择在 `HKCU\...\FileExts`） | `HKCR\<ProgID>\shellex\ContextMenuHandlers` |

另有：网络位置（`Network`/`NetShare`/`NetServer`）、打印机、音频 CD 等次要场景，见官方表。

### 2.2 枚举算法（对某场景 x 的全部条目 + 来源标注）

1. **静态动词**：枚举上述矩阵中所有适用键的 `shell` 子键；读取 `MUIVerb`（否则默认值，否则键名）、`Icon`、`Extended`（仅 Shift 显示）、`ProgrammaticAccessOnly`（不显示）、`AppliesTo`（AQS 条件显示）→ 来源标「静态动词 + 所属键路径」。
2. **COM 处理器**：枚举所有适用键的 `shellex\ContextMenuHandlers` 子键；默认值取 CLSID → 查 `HKCR\CLSID\{clsid}` 得名称与 `InprocServer32` DLL 路径；再查该 CLSID 是否出现在 `Blocked` 键 → 来源标「COM 处理器 + DLL + 厂商信息（可用 DLL 版本资源）」。
3. **打包命令**：`Get-AppxPackage`（或 `PackageManager.FindPackages`）遍历已安装包 → 解析 `AppxManifest.xml` 中 `desktop4:FileExplorerContextMenus` 的每个 `desktop5:ItemType/@Type` 与 `desktop5:Verb/@Clsid`，与场景匹配（`*`↔文件、`Directory`↔文件夹、`Directory\Background`↔背景/桌面）→ 来源标「MSIX 包名 + CLSID」。sparse 包同样出现在包枚举里（它就是已注册的包）。
4. **文件关联类**：解析 `HKCR\.ext` → ProgID/用户选择 → 得「打开」「打开方式」候选（官方博客：新菜单把 Open with 移到顶部）。
5. **云文件**：Cloud Files 注册（本次未深查，标未深查）。
6. **系统内置**：以上都查不到的新菜单项 = 系统内置（剪切/复制/粘贴/重命名/显示更多选项等），标记为不可屏蔽。

### 2.3 工具与参考实现

- [NirSoft ShellExView](https://www.nirsoft.net/utils/shexview.html)：枚举全部 shell 扩展（含 Context Menu 类型），可按类型过滤、禁用（其禁用即写 Blocked 键）。
- [NirSoft ShellMenuView](https://www.nirsoft.net/utils/shell_menu_view.html)：枚举静态动词菜单项。
- Sysinternals Autoruns：全量 shell 扩展视图。
- 开源参考：BluePointLilac/ContextMenuManager（社区常用右键菜单管理器，按注册表路径分类枚举；其 MSIX 部分实现细节本次未核验，标未深查）。

### 2.4 局限（重要）

- **不存在公开 API 直接枚举「新菜单」最终渲染结果**（微软只提供了「如何注册」，没有提供「如何查询」）。以上聚合结果与真实菜单之间可能存在顺序/条件（`AppliesTo`、`GetState` 返回 `ECS_HIDDEN` 等）差异，本项目对「展示层」只应做到近似，对「可操作层」以注册表/清单事实为准。（未确证：是否存在未公开内部接口。）
- 动态 `IContextMenu` 处理器返回的**子菜单内容**（如 7-Zip 展开后才有具体命令）静态扫描不可见，需实例化 COM 才能拿到。

---

## 3. 逐项屏蔽与恢复

### 3.1 `Shell Extensions\Blocked` 键（对 COM 处理器，主流手段）

- 位置：`HKCU\Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked`（**每用户，免管理员**）与 `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked`（全机，需管理员）。键不存在时自建（本机实测两处默认都不存在）。
- 用法：在键下新建**字符串值，值名 = CLSID**（如 7-Zip `{23170F69-40C1-278A-1000-000100020000}`），数据留空；重启 Explorer 生效。社区权威实操见 [Winaero：Remove Apps from Right-click Menu](https://winaero.com/remove-apps-right-click-menu)、[gist: remove-nuisance-context-menu-items.reg](https://gist.github.com/Reedbeta/8725a0a3c7ebcf3e68fc030423561aaf)（含 Skype、Windows Terminal、Defender EPP 等大量实例 CLSID）。
- 可行性结论：**对经典 `IContextMenu` 处理器确证有效**（新菜单本来就不加载它们，因此主要影响经典菜单/「显示更多选项」）。
- 对 **MSIX/IExplorerCommand 打包条目同样有效**的实证：Winaero 用该键移除新菜单「Edit in Notepad」，gist 用它移除「Open in Terminal」（二者都是打包 IExplorerCommand 条目）。**注意：微软 Learn 未文档化 Blocked 键对打包 COM 的行为**，此点标「多方实测一致、官方未确证」。
- 恢复：删除对应值 + 重启 Explorer。扩展本体注册不动、应用无感知，是可逆性最好的手段。
- 副作用与风险：
  1. **按 CLSID 全局生效**：该扩展在所有场景、所有菜单（含拖放处理器等若复用同一 CLSID）全部消失，无法按场景细分。
  2. **误伤共享组件**：同一 CLSID 可能被系统其他功能复用（类似地，社区为隐藏 PowerShell 动词改注册表导致 Explorer 菜单栏按钮变灰的案例，见 [MS Q&A](https://learn.microsoft.com/en-us/answers/questions/1340033/disable-default-open-windows-powershell-right-clic)）——屏蔽前必须核对该 CLSID 的归属（DLL 路径、包名）。
  3. 应用更新重装不会自动解除（值仍在），但应用的卸载清理也不感知它。
  4. 旧策略 `EnforceShellExtensionSecurity`（仅加载 Approved 列表）自 Win7 起实际失效（[MS Q&A 验证帖](https://learn.microsoft.com/en-us/answers/questions/4315201/the-policy-enforceshellextensionsecurity)），不要采用。

### 3.2 静态动词的屏蔽（官方注册表语义）

官方文档（[Creating Shortcut Menu Handlers](https://learn.microsoft.com/en-us/windows/win32/shell/context-menu-handlers)）给动词键定义了如下空字符串 `REG_SZ` 值，是**唯一文档化**的逐动词控制手段：

| 值 | 效果 |
|---|---|
| `ProgrammaticAccessOnly` | 菜单**永不显示**，仍可用 `ShellExecuteEx` 编程调用（官方推荐「隐藏不删除」） |
| `Extended` | 仅 Shift+右键时显示 |
| `AppliesTo` / `DefaultAppliesTo` | 按 AQS 条件显示/隐藏/设默认（如 `System.ItemName:"..."`） |
| `Position=Top|Bottom`、`NeverDefault` 等 | 排序与默认行为控制 |
| `LegacyDisable` | 社区常用隐藏值（MS Q&A 帖提及，**官方文档未见定义**，标半确证） |

- 操作层级：动词多在 `HKLM\Software\Classes`（需管理员）；部分应用写 `HKCU\Software\Classes`（免管理员）。删除/改名整个动词键也可，但应用更新会写回——**改值优于删键**。
- 恢复：删除所加的值（如 `ProgrammaticAccessOnly`）即恢复原状；记录原始状态后完全可逆。
- 副作用：同上节第 2 条——隐藏系统动词（如 PowerShell）可能连带影响 Explorer 菜单栏/文件菜单里的等价项。

### 3.3 MSIX/IExplorerCommand 打包条目的屏蔽

1. 向 `Blocked` 键写该命令的 CLSID（3.1，社区实证有效）；
2. 卸载/注销其包（`Remove-AppxPackage`；对 sparse 包即注销身份包）——副作用大：该应用所有依赖标识的功能（通知、共享目标等）一起消失，**不推荐**作为菜单管理手段。

### 3.4 无法屏蔽的项

新菜单顶部的系统内置命令（剪切/复制/粘贴/重命名/复制文件地址等）：**未发现任何官方注册表/策略可逐项移除**（未确证 = 未找到官方手段；社区仅有整层切换经典菜单的做法，见 §4）。

### 3.5 对本项目 v1 的建议

- 屏蔽一律走「写值不删键」：Blocked 值（COM/打包条目）+ 动词键加 `ProgrammaticAccessOnly`（静态动词），全部可逆、可审计（本应用自持一份变更日志即可实现「恢复」）。
- 每次写入后提示「重启 Explorer」或提供一键重启；变更前备份原值。
- 生效判定：`Test-Path`/枚举 Blocked 键 + 重启前后菜单差异由用户确认。

---

## 4. 「一键切回经典菜单」：CLSID {86ca1aa0-34aa-4e8b-a509-50c905bae2a2}

### 4.1 原理（本机实证 + 社区逆向）

本机（Win11 24H2, 26100.8894）实测：

```
HKLM\SOFTWARE\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}
  (默认) = "File Explorer Context Menu"
  InProcServer32\(默认) = C:\Windows\System32\Windows.UI.FileExplorer.dll
```

即该 CLSID 是**新版菜单的实现对象**（XAML Islands 宿主，代码在 `Windows.UI.FileExplorer.dll`，与 Mouri 所述一致）。Explorer 构建右键菜单时 CoCreate 该对象；`HKCR` 是 HKCU/HKLM 合并视图且 **HKCU 优先**，因此在 `HKCU\Software\Classes\CLSID\{...}\InprocServer32` 建一个**空字符串默认值**后，该 CLSID 的进程内激活得到空 DLL 路径 → 激活失败 → Explorer **回退到旧菜单代码路径**。（回退机制本身为社区逆向结论，见 [r/Windows11 HOWTO 帖评论](https://www.reddit.com/r/Windows11/comments/pu5aa3/howto_disable_new_context_menu_explorer_command)、[elevenforum 教程](https://www.elevenforum.com/t/disable-show-more-options-context-menu-in-windows-11.1589/)；HKLM 侧注册为本机一手实证。）

**易错点**：默认值必须是「空字符串」。regedit 里显示「(value not set)」时不生效，必须打开默认值、清空内容后点确定（[andybrownsword, 2025-04](https://andybrownsword.co.uk/2025/04/29/reverting-the-windows-11-context-menu)）。

### 4.2 标准操作（每用户，免管理员）

```bat
:: 启用经典菜单
reg add "HKCU\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32" /ve /f
taskkill /f /im explorer.exe & start explorer.exe

:: 恢复新版菜单
reg delete "HKCU\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}" /f
taskkill /f /im explorer.exe & start explorer.exe
```

（多来源一致：[4sysops（含 GPO 部署法）](https://4sysops.com/archives/restore-classic-context-menu-in-windows-11-explorer-using-group-policy-or-powershell)、[XDA](https://www.xda-developers.com/how-to-open-full-right-click-menu-by-default-windows-11)、[MS Learn Q&A 精华帖](https://learn.microsoft.com/en-us/answers/questions/2287432/article-restore-old-right-click-context-menu-in-wi)。HKLM 变体也存在但无必要且需夺取 TrustedInstaller 权限，不推荐。）

### 4.3 版本兼容性

| 版本 | Build | 状态 | 依据 |
|---|---|---|---|
| 22H2 | 22621 | ✅ 可用 | [ghacks 读者实测 22621.963](https://www.ghacks.net/2022/12/27/windows-11-dev-channel-restore-the-old-context-menu-style-in-file-explorers-left-pane) |
| 23H2 | 22631 | ✅ 可用 | MS Learn Q&A 帖 2025-11 评论实测 |
| 24H2 | 26100 | ✅ 可用 | **本机实证（26100.8894，该键在用）**；[r/sysadmin 指南](https://www.reddit.com/r/sysadmin/comments/1frq94l/guide_restore_old_rightclick_context_menu_in)多用户确认 |
| 25H2 | 26200 | ✅ 可用 | [elevenforum 帖（26200.9168）多用户确认](https://www.elevenforum.com/t/reg-hack-to-get-old-context-menu-no-longer-works.1380/page-2)；个别用户遇「空值无效、改填 `2` 可用」的个例（**未确证**，疑为个体注册表损坏） |
| Insider（Dev/Beta 26220，2025 下半年） | — | ⚠️ 反复 | Insider 曾报道失效/「菜单改进暂停」（[4sysops 报道](https://4sysops.com/archives/windows-11-insider-preview-builds-26300-and-26220-policy-based-removal-of-preinstalled-apps)），后续又有恢复的报告；该通道结论随时变化，不作为兼容性依据 |
| 未来原生开关 | — | 🔭 预兆 | Experimental 26H2 Insider build 26340.9212 起设置里出现「显示更多选项」增删开关（Shift+右键仍可开经典菜单）（[elevenforum 教程](https://www.elevenforum.com/t/disable-show-more-options-context-menu-in-windows-11.1589/)） |

注意：MS Learn Q&A 帖中有评论称「24H2 已弃用此法，只能注入 explorer」（2025-09），与多方报告及本机实证矛盾，判定为不可靠孤例，但**说明风险真实存在**。

### 4.4 风险与副作用

1. **非官方支持**：微软从未文档化，任何更新可能静默失效（Insider 已发生过）；失效表现为「键在、菜单却还是新的」，无损坏后果，删除键即可回原状。
2. **全有或全无**：整层切换，无法保留新菜单；应用署名 flyout、AI 操作等新菜单特性在经典菜单中以级联形式降级出现（IExplorerCommand 条目仍会显示，通常不丢功能）。
3. **仅当前用户**：HKCU 覆盖，不影响其他用户；无需管理员是优点也是「策略管控盲区」（企业 GPO 可用 4sysops 的组策略首选项法统一部署）。
4. 与 Shift+右键的原生关系：不套用时 Shift+右键/Shift+F10 本来就直接打开经典菜单（[Superuser](https://superuser.com/questions/1854126/how-can-i-get-back-old-context-menu-for-windows-11-right-click-tried-4-differen)）；套用后无差异。
5. 对本项目的含义：**管理器必须同时兼容两种模式**。检测方法（零副作用）：`Test-Path 'HKCU:\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}'` —— 本机当前为 True（即开发机正运行经典菜单）。产品内的「一键切回/还原」即 4.2 的两条命令 + 重启 Explorer 封装。

---

## 5. 第三方进入新菜单顶部：sparse MSIX / IExplorerCommand 路线概貌（v1 不做）

结论级要点（依据 [官方 How-to](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/integrate-packaged-app-with-file-explorer)、[Grant identity to non-packaged apps](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-nonpackaged-apps)、[Mouri 实作](https://mouri.moe/en/2021/12/25/Share-my-experience-of-implementing-context-menu-support-for-File-Explorer-in-Windows-11)、[VS Code issue #127365](https://github.com/microsoft/vscode/issues/127365)）：

1. 唯一正路：原生 COM DLL 实现 `IExplorerCommand` + **应用标识**。完整 MSIX（如 NanaZip、终端、记事本、PowerToys）或 **sparse package**（仅含清单的「身份包」，二进制留在外部安装目录，`uap10:AllowExternalContent=true`、`TrustLevel=mediumIL`、`RuntimeBehavior=win32App`、声明 `runFullTrust`+`unvirtualizedResources`；包必须签名，由安装器注册）。
2. 清单注册两件套：`windows.comServer`（`com:Class` 指定 CLSID/DLL/STA）+ `desktop4:FileExplorerContextMenus` > `desktop5:ItemType`（`Type`=`*`/`Directory`/`Directory\Background`，可多场景复用）> `desktop5:Verb`（Id + Clsid）。
3. 能力与限制：标题/图标/状态/子命令（`EnumSubCommands`，实现应用署名 flyout）；构造菜单的回调在 shell UI 路径上必须快；每处理器最多 16 项（社区实测）；不能自绘（无预览图）；DLL 架构须匹配 Explorer；注册后需重启 Explorer 才加载。
4. 定位：这类条目显示在「Shell 动词之下」的**应用署名分组**（即用户所说「顶部菜单」区域）；>1 动词自动折叠为 flyout。
5. 对本项目：v1 只做「识别 + 标注来源 + 可逆屏蔽」，不实现自身进入新菜单；未来若做，sparse 路线是唯一免整体打包的选择，且用户侧可用 Blocked 键/注销包来管理这类条目。

---

## 6. 未确证 / 未深查清单

- Blocked 键对打包（MSIX）COM 的拦截行为：多方实测一致，微软未文档化。
- 25H2 个例中「InprocServer32 默认值填 `2` 代替空值」的变通：孤例，未确证。
- 2025 下半年 Insider 26220 通道该 hack 一度失效的确切 build 范围与结论：通道易变，未确证。
- 「显示更多选项」的原生 Settings 开关：仅在 Experimental 26H2 Insider（26340.9212+）观察到，正式版未知。
- 系统内置新菜单项是否存在隐藏键：未找到官方手段。
- 云文件（Cloud Files）命令的注册细节、ContextMenuManager 对 MSIX 条目的支持细节：本次未深查。
- 动词 `LegacyDisable` 值：社区与 MS Q&A 广泛使用，官方文档未见定义（半确证）。

## 7. 参考资料

**微软一手**
- [Extending the Context Menu and Share Dialog in Windows 11 — Windows Developer Blog](https://blogs.windows.com/windowsdeveloper/2021/07/19/extending-the-context-menu-and-share-dialog-in-windows-11/)
- [Add a File Explorer context menu command to a packaged desktop app — MS Learn](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/integrate-packaged-app-with-file-explorer)
- [Grant package identity by packaging with external location — MS Learn](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-nonpackaged-apps)
- [Registering Shell Extension Handlers — MS Learn](https://learn.microsoft.com/en-us/windows/win32/shell/reg-shell-exts)
- [Creating Shortcut Menu Handlers — MS Learn](https://learn.microsoft.com/en-us/windows/win32/shell/context-menu-handlers)
- [IExplorerCommand — MS Learn](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-iexplorercommand)
- [desktop4:FileExplorerContextMenus 架构 — MS Learn](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-desktop4-fileexplorercontextmenus)
- [MS Learn Q&A：Restore old Right-click Context menu（含各版本实测评论）](https://learn.microsoft.com/en-us/answers/questions/2287432/article-restore-old-right-click-context-menu-in-wi)
- [MS Learn Q&A：EnforceShellExtensionSecurity 已失效](https://learn.microsoft.com/en-us/answers/questions/4315201/the-policy-enforceshellextensionsecurity)
- [MS Learn Q&A：adding an item to Windows 11 Context Menu（sparse 路线答疑）](https://learn.microsoft.com/en-us/answers/questions/832880/adding-an-item-to-windows-11-context-menu)

**可信技术社区**
- [Mouri (NanaZip)：Windows 11 上下文菜单实作经验](https://mouri.moe/en/2021/12/25/Share-my-experience-of-implementing-context-menu-support-for-File-Explorer-in-Windows-11)
- [Winaero：Remove Apps from Right-click Menu（Blocked 键实操）](https://winaero.com/remove-apps-right-click-menu)
- [gist：remove-nuisance-context-menu-items.reg（大量实例 CLSID）](https://gist.github.com/Reedbeta/8725a0a3c7ebcf3e68fc030423561aaf)
- [4sysops：Restore classic context menu（PowerShell/GPO）](https://4sysops.com/archives/restore-classic-context-menu-in-windows-11-explorer-using-group-policy-or-powershell)
- [elevenforum：Disable "Show more options"（含 26H2 原生开关）](https://www.elevenforum.com/t/disable-show-more-options-context-menu-in-windows-11.1589/)
- [elevenforum：Reg hack no longer works？（25H2 实测与个例）](https://www.elevenforum.com/t/reg-hack-to-get-old-context-menu-no-longer-works.1380/page-2)
- [andybrownsword：Reverting the Windows 11 Context Menu（空值细节）](https://andybrownsword.co.uk/2025/04/29/reverting-the-windows-11-context-menu)
- [r/Windows11 HOWTO（机制讨论）](https://www.reddit.com/r/Windows11/comments/pu5aa3/howto_disable_new_context_menu_explorer_command) / [r/sysadmin 指南](https://www.reddit.com/r/sysadmin/comments/1frq94l/guide_restore_old_rightclick_context_menu_in)
- [ghacks：Dev channel 恢复旧菜单（22H2 实测）](https://www.ghacks.net/2022/12/27/windows-11-dev-channel-restore-the-old-context-menu-style-in-file-explorers-left-pane)
- [NirSoft ShellExView](https://www.nirsoft.net/utils/shexview.html) / [ShellMenuView](https://www.nirsoft.net/utils/shell_menu_view.html)
- [VS Code issue #127365：Integrate with the Windows 11 Context Menu](https://github.com/microsoft/vscode/issues/127365)
- [Superuser：Shift+右键直接打开经典菜单](https://superuser.com/questions/1854126/how-can-i-get-back-old-context-menu-for-windows-11-right-click-tried-4-differen)

**本机一手实证（2026-08-30，Win11 24H2 26100.8894）**：`{86ca1aa0-…}` 的 HKLM 注册（名称「File Explorer Context Menu」、DLL 指向 `Windows.UI.FileExplorer.dll`）；HKCU 覆盖键在本机存在（hack 生效中）；`Blocked` 键 HKLM/HKCU 默认均不存在需自建。
