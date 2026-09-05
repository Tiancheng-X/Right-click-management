---
labels: [wayfinder:map]
---

# 地图：Windows 11 右键菜单管理器

## Destination

一个能在 Windows 11 上实际运行、界面美观（Win11 Fluent 风、文案简体中文）的右键菜单管理器：绿色便携单 exe、始终管理员运行；可查看/禁用/删除 Win11 新版菜单与经典菜单条目、新增自定义菜单项（进经典菜单）、一键切回经典菜单、改动前自动快照并支持时间点还原。程序跑起来、上述功能可用，地图即完成（本 effort 已把执行纳入地图）。

## Notes

- 域：Windows Shell + 注册表。右键菜单条目的事实来源是注册表（及 MSIX 包清单），程序本质是这些数据的可视化编辑器。
- 技术栈已定：Tauri v2（Rust 后端 + Web 前端），不再开放选型。
- 绘图期已定前提（第一、二轮 grilling，Q1–Q10）：
  - 终点 = 可用的程序（执行纳入地图，工单可产出真实代码与工件）；
  - 用户 = 自己和身边人（无签名/更新/多语言负担）；
  - 功能 = 新版菜单 + 经典菜单 + 自定义项 + 备份恢复，四项全做；
  - 权限 = 始终管理员运行（用户明确选择，未采纳按需提权）；
  - 提供「一键切回经典菜单」全局开关；v1 自定义项只进经典菜单；
  - UI = Win11 Fluent 风（Mica/圆角/深浅色跟随系统）、文案简体中文；
  - 备份 = 改动前自动快照 + 时间点还原；交付 = 绿色便携单 exe。
- 进行期修订（骨架验收反馈，2026-08-30）：**视觉语言由 Win11 Fluent 改为 Apple 式设计语言**（按 apple-design-zh 规范：材质半透明分层、按下即反馈、可打断过渡、排印纪律、reduced-motion/transparency 适配）；**布局不变**（仍是变体 A 三栏）；去掉应用内仿制标题栏（原生窗口框自带），工具栏合并为列表头上方轻材质层。前端架构改为静态骨架 + 局部更新，禁止全量重绘（用户反馈：点击整页刷新很难受）。
- 进行期修订（视觉定版，同日）：采纳用户提供的 `design.html` 视觉（玻璃卡片流）——渐变背景上悬浮毛玻璃主容器、**两栏 + 底部状态栏**（详情面板取消，点卡片内联展开注册表位置）、iOS 弹簧开关与卡片悬停浮起；深色模式按同一语言适配（design.html 仅浅色）；开关与「+ 新增」先渲染为禁用态（操作后续工单接入，不放假控件）；覆盖式滚动条（隐形→滚动/悬停浮现细条）。布局血统仍源自变体 A（侧栏筛选 + 列表主区）。
- 进行期修订（分类与来源，同日）：导航分类定为两个用户场景——**文件右键 / 桌面右键**（由挂载点推导，取代挂载点直译）；卡片直标**来源应用**（后端推导：动词 = 命令行 exe 名，shellex = CLSID→DLL 所在目录名，失败回退挂接键名），展开详情补命令行字段。CONTEXT.md 的「适用场景」「来源」术语已同步。
- 进行期修订（免重启刷新，同日）：写操作成功后自动广播 `SHChangeNotify(SHCNE_ASSOCCHANGED)` 刷新菜单缓存——**动词与 Blocked 类改动一般免重启即时生效**，重启资源管理器降级为兜底手段（徽章文案改为「菜单没变化？」）；经典菜单开关是进程级 DLL 切换，仍必须重启。
- 技能约定：research 工单调用 "research"；prototype 工单调用 "prototype" 与 "frontend-design"；grilling 工单调用 "grilling" 与 "domain-modeling"。
- Tracker 为本地 markdown：地图即本文件；工单在 `wayfinder/tickets/`；认领/阻塞/状态看工单 front-matter（`status: open|closed`、`assignee`、`blocked-by`）。叙述时用工单名，不用裸 id。

## Decisions so far

<!-- 每关闭一张工单，在这里加一行：[工单名](相对链接): 一句话结论 -->

- [设置页](tickets/T15-settings.md): ⚙ 第三视图（快照保留可调+两段式清空 / 数据目录+便携徽章+一键打开 / 策略检测常驻提示 / 关于页）；设置持久化 data\settings.json；保留上限 5–500 可调即时生效。

<!-- 🏁 全部工单已关闭（T1–T15），地图主干与执行路线走完：Destination 达成。 -->

- [新版菜单条目（MSIX 枚举与屏蔽）](tickets/T14-msix-entries.md): PackageManager 枚举主包 + 解析 AppxManifest 的 FileExplorerContextMenus（desktop4/5）；打包条目入域模型（📦、仅新版菜单标签），屏蔽复用 Blocked 键；标题需 COM 激活故 v1 显示「包名 · VerbId」；删除不支持（不卸载包）；系统内置无公开 API 不列出。

- [一键切回经典菜单](tickets/T13-classic-toggle.md): 侧栏玻璃卡片开关；开 = CLSID InprocServer32 空串默认值、关 = 仅删该值；状态以键为准（启动自检、失效自然为关）+ 注明可重开；写前留值级前像快照，toast + 待生效徽章。

- [自定义菜单项新增与编辑](tickets/T12-custom-entries.md): 玻璃风模态表单新增/编辑（名称/命令/图标/场景），HKCU 写 shell\verb + MenuManager 标记值区分自建与他人（D6）；图标 = rfd 文件选择或手填路径即时预览提取；场景变化 = 旧删新建双侧前像快照，完全可逆。

- [快照与时间点还原落地](tickets/T11-snapshot-restore.md): JSON 单文件前像存储（值级+子树级统一，删除类/手动点 protected 永不清理，滚动 60 条）；禁用/启用/删除全接入自动留档；还原 = 还原前留档可撤销 + 原生逐键精确写回 + 外部改动如实报告；前端新增快照历史视图（还原两段确认、撤销横幅、手动快照点）。

- [条目操作接入](tickets/T10-entry-operations.md): 禁用/启用 = LegacyDisable（动词）与 Blocked 键（shellex）写值不删键，HKCU 优先回退 HKLM；开关接真状态推导；删除 = 递归断开挂接 + 强制 reg.exe 导出 .reg（失败中止）；两段式确认、待生效徽章一键重启、分类 toast 全部落地。

- [异常与边界处理](tickets/T9-edge-cases-and-resilience.md): 失败 = 三档 toast 不打断；经典开关以注册表键为准 + 注明可重开；重启引导 = toast + 侧栏「待生效」徽章一键重启；启动检测策略键并常驻提示；还原失败如实报告 + 撤销闭环。
- [枚举与刷新语义](tickets/T8-enum-and-refresh-semantics.md): 启动即扫 + 手动兜底；首扫骨架屏、重扫保留旧内容；窗口聚焦时静默重扫感知外部变化；首屏目标 ≤ 1s、实测说话不提前优化。

- [初始化 Tauri 项目骨架](tickets/T7-scaffold-tauri-project.md): Tauri v2 + 原生 TS/Vite 工程落仓库根，变体 A 视觉迁移完成；requireAdministrator manifest 经 build.rs 注入生效（dev/build 均弹 UAC）；winreg 只读枚举占位模块 + `list_menu_entries` 命令打通前端⇄注册表数据通路；pnpm build 与 cargo check 全绿。
- [备份/恢复设计](tickets/T6-backup-restore-design.md): 每操作一录全带前像（JSON 主存储，删除类附 .reg，另有手动全量快照点）；存储 exe 同目录 `data\snapshots\` 回退 %APPDATA%，滚动 60 条且删除类/手动点永不清；还原 = 权威逐键写回 + 还原前自动留档可撤销 + 外部改动如实报告；v1 只整条还原。
- [核心域模型与操作语义](tickets/T5-domain-model-and-semantics.md): 一个逻辑条目多挂载点、操作作用全部挂载点；禁用逐类可逆（动词/自定义=LegacyDisable，shellex/打包=Blocked 键，系统内置只读）；删除=断开挂接不碰 CLSID 本体+强制先快照；可疑残留只做两条零误报确定性判定；新版/经典分层详情标注不加筛选；自定义条目自己的可编辑。术语见 [CONTEXT.md](../../CONTEXT.md)。
- [主界面原型](tickets/T4-main-ui-prototype.md): 用户选定 **变体 A · 资源管理器三栏**（场景/来源导航树 + 条目列表 + 常驻详情面板）为主界面视觉基准；Fluent 风在纯 Web 技术下高保真可行；原型资产 [prototype/main-ui.html](../../prototype/main-ui.html) 留作基准，B/C 变体保留供偷细节。
- [经典菜单注册表全景调研](tickets/T2-classic-menu-registry.md): 经典菜单只有两种挂载机制（`shell\<verb>\command` 静态动词、`shellex\ContextMenuHandlers` COM 处理器），铺满 `*`/AllFilesystemObjects/Folder/Directory/Directory\Background/**DesktopBackground**（并非 `Desktop`，本机实测）/Drive/lnkfile/ProgID 等挂载点；HKCR 是 HKCU 优先的合并视图、写入应显式走 HKCU/HKLM；禁用手法按可逆性排列（LegacyDisable/ProgrammaticAccessOnly/Extended → 导出后删键 → Blocked 键/无效 CLSID）；第三方分两派（压缩网盘类走 shellex、编辑器类走静态动词）；可见名称/图标/命令/场景可从注册表完全推导；`SkipExplorerPage` 查无出处、弃用。
- [Win11 新版右键菜单机制调研](tickets/T1-win11-new-menu-mechanics.md): 新菜单双层结构、旧 COM 项折叠不丢失；枚举无公开 API 须分来源聚合（HKCR 扫描 + MSIX 清单）；逐项屏蔽 = Blocked 键写 CLSID 空串值（HKCU 免管理员）或静态动词加 `ProgrammaticAccessOnly`/`Extended`；切经典菜单 = `{86ca1aa0-…}` InprocServer32 建**空字符串**默认值 + 重启 explorer（22H2–25H2 稳定版可用、非官方需兜底）；进新菜单顶部唯一正路是 sparse MSIX/IExplorerCommand（v1 不做）。
- [Tauri 栈系统层能力调研](tickets/T3-tauri-stack-research.md): 注册表用 winreg 0.56（WOW64 双视图一等支持）；提权走 build.rs `app_manifest` 注入 requireAdministrator（裸单 exe 同样弹 UAC）；快照 = reg.exe 导出 .reg + API 枚举/JSON 主存储、恢复走 API 精确写回；前端推荐原生 TS + Vite 自写 Fluent 组件；便携单 exe 可行（唯一硬依赖 WebView2），数据目录 exe 同目录 `data\`、不可写回退 %APPDATA%。

## Not yet specified

（雾区已清空——「快照存储布局」随「备份/恢复设计」定案消散。剩余前沿均为已明确的开放工单。）

## Out of scope

- 公开发布工程（代码签名、自动更新、安装器、多语言）——目标用户是自己和身边人。
- Windows 10 及更早系统适配——目标只有 Windows 11。
- 菜单之外的 Shell 定制（任务栏、开始菜单、默认程序关联）——不是本图要找的路。
- 自定义项进入 Win11 新版顶部菜单——v1 明确不做；技术结论留在「Win11 新版右键菜单机制调研」，将来若要做单独立图。
