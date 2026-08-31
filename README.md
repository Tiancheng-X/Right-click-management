# Right-click-management · 右键菜单管家

运行在 Windows 11 上的右键菜单管理器：查看 / 禁用 / 删除新版与经典菜单条目、新增自定义菜单项、一键切回经典菜单、改动前自动快照并支持时间点还原。Win11 Fluent 风界面，绿色便携单 exe。

> 本仓库的规划过程由 wayfinder 地图驱动：路线与全部决策见 [`wayfinder/map.md`](wayfinder/map.md)，术语表见 [`CONTEXT.md`](CONTEXT.md)，界面视觉基准见 [`prototype/main-ui.html`](prototype/main-ui.html)。

## 技术栈

- **应用框架**：Tauri v2（Rust 后端 + WebView2）
- **前端**：原生 TypeScript + Vite + 自写 Fluent 组件
- **系统层**：winreg（注册表）、requireAdministrator manifest（始终管理员运行）

## 目录结构

```
├── index.html              # Vite 入口
├── src/                    # 前端（vanilla-ts）
│   ├── main.ts             # 变体 A 三栏布局 + 数据调用（后端不可达时回退 mock）
│   └── styles.css          # Fluent 令牌与组件样式（深浅色跟随系统）
├── src-tauri/              # Tauri 后端
│   ├── app.manifest        # requireAdministrator + Common-Controls v6
│   ├── build.rs            # 注入上述 manifest（始终管理员运行）
│   ├── tauri.conf.json     # 窗口 / 打包配置
│   └── src/
│       ├── lib.rs          # Tauri 命令注册
│       ├── main.rs         # 入口
│       └── registry.rs     # 注册表访问模块（占位：只读枚举）
├── prototype/              # 主界面原型（三变体，变体 A 为视觉基准）
└── wayfinder/              # 路线图与工单
```

## 开发

```powershell
pnpm install
pnpm tauri dev
```

> 首次运行会弹 UAC——这是「始终管理员运行」的既定行为（manifest 注入）。

## 构建

```powershell
pnpm tauri build
```

数据目录：exe 同目录 `data\`（便携优先），不可写时回退 `%APPDATA%`。

## 当前状态

**四大功能全部落地**：

- ✅ 经典菜单条目管理（8 类挂载点全景枚举、禁用/启用/删除，写值不删键可逆）
- ✅ 新版菜单条目（MSIX 打包条目枚举 + Blocked 键屏蔽、层级标注）
- ✅ 自定义菜单项（新增/编辑/删除，自建标记区分，图标提取，写 HKCU 免管理员）
- ✅ 一键切回经典菜单（含失效说明与快照兜底）
- ✅ 自动快照 + 快照历史 + 时间点还原（前像 JSON 存储、撤销闭环）
- ✅ 设置页（保留策略、数据目录、策略检测、关于）

已知限制（详见 wayfinder 工单决议）：打包条目标题显示「包名 · VerbId」（真实标题需 COM 激活）；系统内置项无公开 API 不列出；打包条目不支持删除。
