# T3 · Tauri 栈系统层能力调研

> 调研日期：2026-08-30 ｜ 调研方式：web_search + 一手文档逐页核对（官方文档 / docs.rs / Microsoft Learn / Tauri GitHub）
> 版本现状：**tauri 2.11.5**（2026-07-01）、tauri-build 2.6.3、winreg 0.56.0、windows-registry 0.6.1（详见文末「版本现状」）

## TL;DR

1. **注册表**：[winreg](https://crates.io/crates/winreg) 能力完全覆盖本项目需求（枚举/读写/删除/WOW64 双视图/事务/serde）；[windows-registry](https://crates.io/crates/windows-registry)（微软 windows-rs 官方系）更现代但截至 0.6.1 没有一等公民的 32/64 位视图切换 API。
2. **始终管理员**：Tauri v2 **无内置配置项**（[feature request #7173](https://github.com/tauri-apps/tauri/issues/7173) 至今 open）；官方路径是在 `src-tauri/build.rs` 用 [`WindowsAttributes::app_manifest`](https://docs.rs/tauri-build/2.6.3/tauri_build/struct.WindowsAttributes.html) 注入 `requireAdministrator` manifest——官方文档直接给了示例。manifest 嵌在 exe 内，**裸单 exe 直跑同样弹 UAC**。
3. **.reg 导出/导入**：可靠做法是调用 `reg.exe export/import`（静默、退出码 0/1）；自动快照的主存储建议用注册表 API 枚举 + 自有 JSON 格式，恢复走 API 精确写回，`.reg` 只作为交付/人工恢复的产物。
4. **前端**：Tauri 框架无关，[create-tauri-app 官方模板](https://v2.tauri.app/start/create-project)覆盖 vanilla/Vue/Svelte/React/Solid 等；本项目自定义 Fluent UI 推荐原生 TS + Vite。
5. **绿色便携**：`target/release/<app>.exe` 可直接分发，唯一硬依赖 WebView2（[Win11 预装](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/evergreen-vs-fixed-version)）；官方 updater 仅支持安装器形态（本项目自动更新本就 out of scope）；数据目录推荐 **exe 同目录 `data\` 子目录**（自实现，Tauri 的 `app_data_dir` 恒指 `%APPDATA%`）。

---

## 1. Rust 注册表访问：winreg vs windows-registry

### 结论要点

- **winreg 0.56.0**（gentoo90 维护，社区事实标准，基于 `windows-sys`）能力边界，见 [crate 文档](https://docs.rs/winreg/0.56.0/winreg/) 与 [RegKey API](https://docs.rs/winreg/0.56.0/winreg/reg_key/struct.RegKey.html)：
  - 打开/创建/删除/重命名子键：`open_subkey`（KEY_READ）、`open_subkey_with_flags(path, perms: REG_SAM_FLAGS)`、`create_subkey`（KEY_ALL_ACCESS，返回 `REG_CREATED_NEW_KEY`/`REG_OPENED_EXISTING_KEY` disposition）、`delete_subkey`（不能删含子键的键）、`delete_subkey_all`（递归删）、`rename_subkey`、`copy_tree`；
  - **32/64 位视图一等支持**：`*_with_flags` 系列直接接受 `KEY_WOW64_64KEY` / `KEY_WOW64_32KEY`，文档示例原话即「delete the key from the 32-bit registry view」；
  - 读写值：`get_value`/`set_value` 强类型转换（`String`/`u32`/`u64` ↔ `REG_SZ`/`DWORD`/`QWORD`），`get_raw_value`/`set_raw_value` 原始字节 + 显式 `vtype`（覆盖 `REG_EXPAND_SZ`/`REG_MULTI_SZ`/`REG_BINARY` 等全部类型）；
  - 枚举：`enum_keys()` / `enum_values()`（另有 `_os_string` 变体）迭代器；
  - 附加：`query_info()`（含最后写入时间，可选 chrono）、`Transaction` 事务（feature）、serde `encode/decode`（feature，写整键可带事务回滚）、`load_app_key` 加载 hive 文件。
- **windows-registry 0.6.1**（[microsoft/windows-rs](https://github.com/microsoft/windows-rs) 官方，kennykerr 维护，100% documented）见 [crate 文档](https://docs.rs/windows-registry/0.6.1/windows_registry/)：`Key`/`KeyIterator`/`ValueIterator`/`Value`/`Type`（全部值类型枚举）+ 预定义根键常量（`CLASSES_ROOT`、`CURRENT_USER`、`LOCAL_MACHINE`、`USERS`、`CURRENT_CONFIG`）；`OpenOptions` 支持 `read/write/create/transaction/volatile/access(u32)`（[OpenOptions API](https://docs.rs/windows-registry/0.6.1/windows_registry/struct.OpenOptions.html)）；强类型 `set_u32`/`set_string`/`get_string` 等；注册表事务 `Transaction` + commit。
- **两者对本项目都够用**。推荐 **winreg**：需要遍历/清理 `HKCR\*\shell`、`HKCU\Software\Classes\*\shell` 等大量键值，且很可能要同时看 64 位与 32 位（WOW6432Node）两个视图（大量 32 位老程序的菜单项注册在 32 位视图）——winreg 的 `KEY_WOW64_32KEY/64KEY` 是文档明确的一等 API；windows-registry 截至文档未见对应的专门方法（见「坑与边界」）。若不关心 32 位视图、且更看重官方维护背书，windows-registry 也是合理选择。

### 坑与边界

- **WOW64 重定向**：32 位进程访问 `HKLM\Software`、`HKCR` 会被重定向到 `WOW6432Node`（[Microsoft Learn：View registry keys with 64-bit versions of Windows](https://learn.microsoft.com/en-us/troubleshoot/windows-client/performance/view-system-registry-with-64-bit-windows)）。本程序是 x64 原生进程，默认落 64 位视图；要管理 32 位程序的菜单条目必须**显式**用 `KEY_WOW64_32KEY` 再开一次，两视图内容不互通。
- **HKCR 是合并视图**：读写语义（对 HKCR 的写落到哪里、per-user 覆盖 per-machine）属于经典菜单机制范畴，留待工单「经典菜单注册表机制」详细展开；在本工单语境下的结论是：**以管理员身份运行后写 HKCR 可行**（`reg add` 等命令的合法根键都包含 HKCR，见 [reg 命令文档](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/reg)）。
- **`KEY_WOW64_64KEY`/32KEY 与 windows-registry**：0.6.1 的 `OpenOptions` 只有 `access(u32)` 一个底层入口，未见 wow64 相关一等方法；能否安全传原始 `KEY_WOW64_*` 位实现跨视图——**未确证**（文档未提及）。
- **`REG_EXPAND_SZ` 展开语义**：winreg 用 `RegQueryValueEx` 原始读取，`get_value::<String>` 返回的是原始串还是环境变量展开后的串，文档未明示——**未确证，接入前需实测**（对本项目：菜单命令行里 `%SystemRoot%` 一类值应按原样保留展示，倾向拿到原始值反而合适）。
- 错误处理：winreg 返回 `io::Error`（`ErrorKind::NotFound`/`PermissionDenied`），windows-registry 返回自有的 `windows_registry::Result`（携带 Windows 错误码）；错误面不同，封装层二选一即可。

---

## 2. 「始终管理员运行」：manifest requireAdministrator + Tauri v2

### 结论要点

- **机制**：应用 manifest 中 `<requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>` 会让系统把该程序标记为管理类应用并在启动时执行提权（弹 UAC），见 [Microsoft Learn：Application manifests](https://learn.microsoft.com/en-us/windows/win32/sbscs/application-manifests)。
- **Tauri v2 官方实现路径**：`src-tauri/build.rs` 中：

  ```rust
  fn main() {
    let mut windows = tauri_build::WindowsAttributes::new();
    windows = windows.app_manifest(r#"
      <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
        <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
          <security>
            <requestedPrivileges>
              <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
            </requestedPrivileges>
          </security>
        </trustInfo>
      </assembly>"#);
    tauri_build::try_build(
      tauri_build::Attributes::new().windows_attributes(windows)
    ).expect("failed to run build script");
  }
  ```

  这是 [tauri-build `WindowsAttributes::app_manifest` 官方文档](https://docs.rs/tauri-build/2.6.3/tauri_build/struct.WindowsAttributes.html)（2.6.3，2026-06 发布）里**逐字给出的示例**，原文明确说「every time it is executed, a Windows UAC dialog will appear」。manifest 内容也可放独立文件用 `include_str!("manifest.xml")`。
- **无内置配置**：`tauri.conf.json` v2 没有让应用 exe 提权启动的开关；[feature request #7173](https://github.com/tauri-apps/tauri/issues/7173)（2023-06 提出）至今仍是 **open** 的 feature request，即官方口径就是「用 app_manifest 自己注入」。
- **NSIS 的 `installMode` 与此无关**：`bundle > windows > nsis > installMode` 的 `currentUser`（默认，装到 `%LOCALAPPDATA%`、安装器不需要管理员、元数据写 HKCU）/ `perMachine`（装 Program Files、**安装器**需要管理员、元数据写 HKLM）/ `both` 影响的是**安装/卸载过程**的提权与位置，见 [Tauri 配置参考（NSISInstallerMode）](https://v2.tauri.app/reference/config) 与 [Windows Installer 文档](https://v2.tauri.app/distribute/windows-installer)。它不改变应用 exe 启动时是否弹 UAC。
- **非安装单 exe 同样生效**：manifest 由 tauri-build（经 tauri-winres）编译为 exe 的 Win32 资源，属 PE 文件的一部分，与分发方式无关；`target/release` 下的裸 exe 直接运行也会请求 UAC。官方文档无一句专门针对「未安装分发」的表述——此点由机制 + `app_manifest` 文档语义推出，**表述层面未确证但机制成立**。

### 坑与边界

- **自定义 manifest 必须保留默认内容**：Tauri 默认 manifest 含 Common-Controls v6 依赖；[官方 Warning](https://docs.rs/tauri-build/2.6.3/tauri_build/struct.WindowsAttributes.html)：使用 dialog API 时若丢了该依赖会出问题。正确做法 = 默认 `<dependency>` 块 + 追加 `<trustInfo>` 块合并成一份。
- **dev 模式也会提权**：build script 对 debug 构建同样生效，`tauri dev` 每次运行都弹 UAC。社区通用做法是在 build.rs 里按 cargo 注入的 `PROFILE` 环境变量只在 release 注入提权 manifest（[cargo 构建脚本环境变量](https://doc.rust-lang.org/cargo/reference/environment-variables.html)）；这是实践建议而非官方条目。
- **提权与 Tauri 功能的已知摩擦**：管理员运行时 file-drop 事件失效（[#9271](https://github.com/tauri-apps/tauri/issues/9271)，修复状态**未确证**）；updater 遇到需管理员权限的 NSIS 安装器会失败（[#7184](https://github.com/tauri-apps/tauri/issues/7184)）——本项目不依赖 drag-drop、自动更新 out of scope，影响可控。
- 历史：Tauri v1.x 曾有自定义 manifest 报「duplicate resource」的 bug（[#10154](https://github.com/tauri-apps/tauri/issues/10154)，v1.6.2 时代）；v2 现状未再见同类报告。

---

## 3. 编程式导出/导入 .reg：reg.exe vs 自实现

### 结论要点

- **reg.exe export**：`reg export <KeyName> <FileName> [/y]`，仅限本地计算机，根键可为 HKLM/HKCU/HKCR/HKU/HKCC，输出文件必须 `.reg` 扩展名，`/y` 覆盖不询问；退出码 0 成功 / 1 失败（[Microsoft Learn：reg export](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/reg-export)）。程序内用 `std::process::Command` 调用即可静默完成。
- **reg.exe import**：`reg import <FileName>` 把 .reg 内容合并进注册表，支持 `/reg:32` / `/reg:64` 指定视图，退出码 0/1（[Microsoft Learn：reg import](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/reg-import)）。注意：**成功消息打到 stderr**，在 PowerShell 里会伪装成 terminating error，但 `$LASTEXITCODE`/进程 exit code 仍正确（[Stack Overflow 案例](https://stackoverflow.com/questions/61483349/powershell-throws-terminating-error-on-reg-import-but-operation-completes-succes)）——Rust 里读 exit code 不受影响。`regedit /s file.reg` 是另一条静默导入途径（[社区资料](https://community.spiceworks.com/t/import-registry-via-command-prompt-silently/806260)）。
- **reg save / reg restore**：`reg save <KeyName> <FileName>` 保存为 **`.hiv` 二进制 hive 文件**（不是 .reg 文本），`reg restore` 写回（[reg save](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/reg-save)、[reg restore](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/reg-restore)）。粒度是整个键子树的二进制快照，适合「整键原样备份」，不适合 diff 与选择性恢复。
- **自实现 .reg v5 序列化：可行但细节多**。格式要点（多来源交叉印证）：UTF-16LE 带 BOM 编码、首行 `Windows Registry Editor Version 5.00`、键路径行 `[HKEY_...\subkey]`、值行 `"name"="value"` / `dword:` / `hex(xx):`、删除键 `[-HKEY_...]`、删除值 `"name"=-`；编码/转义细节见 [NinjaOne：Export and Import Keys](https://www.ninjaone.com/blog/export-and-import-keys-in-registry-editor-in-windows)（明确 .reg 为 UTF-16 LE BOM、行尾格式错误会导致合并失败）与 [Google Groups 讨论](https://groups.google.com/g/vim_use/c/8btxOMzYfOk)（.reg 为 UTF-16le with BOM）。微软经典 KB（KB310516「add, modify, or delete registry subkeys and values by using a .reg file」）现并入 Learn troubleshoot 库，本轮两次检索均 404 未能打开确认最终 URL——**官方格式规范链接未确证**，实现时以 reg.exe 实际导出的文件做 golden 测试最稳。
- **注意 .reg import 的合并语义**：导入是 merge（新增/覆盖），**不会删除**文件里不存在的值；「精确还原到某时间点」必须配合显式删除逻辑。

### 自动快照场景推荐

1. **主存储走 API**：改动前用 winreg 递归枚举将被修改的键子树 → 序列化为自有结构化格式（JSON/ron）存入应用数据目录。优点：可 diff、可选择性恢复、可做时间点列表，完全掌控语义。
2. **恢复走 API**：按快照精确写回（先删后写/逐值覆盖），避免 reg import 的 merge 语义带来的残留。
3. **.reg 作为交付物**：需要「双击就能还原/可在注册表编辑器查看」的人工恢复产物时，优先 **调用 `reg.exe export` 生成**（改动前自动导出被改键子树到快照目录），把格式正确性外包给系统工具；自实现序列化仅在需要「从 JSON 快照反向生成 .reg」时做，并配 golden 测试。

---

## 4. Tauri v2 与前端方案：集成现状与推荐

### 结论要点

- **框架无关是官方立场**：Tauri 的前端就是被 WebView 加载的静态资产（`frontendDist`）+ `invoke` IPC，官方原话「ability to work with virtually any frontend framework」，见 [Create a Project](https://v2.tauri.app/start/create-project)。
- **官方模板齐全**：[create-tauri-app](https://github.com/tauri-apps/create-tauri-app) 预设含 `vanilla / vanilla-ts / vue / vue-ts / svelte / svelte-ts / react / react-ts / preact / preact-ts / solid / solid-ts / angular`，另有 Rust 系（Yew/Leptos/Sycamore）与 .NET（Blazor）；社区模板走 Awesome Tauri（[同页](https://v2.tauri.app/start/create-project)）。各方案均为成熟集成，无官方偏好。
- **Tauri v2 IPC 对框架透明**：`@tauri-apps/api` 的 invoke/事件在任何框架（或无框架）下用法一致；权限模型（capabilities/ACL）在 Rust/配置侧，与前端框架无关。

### 对本项目的推荐

- **推荐：原生 TS + Vite（`vanilla-ts` 模板）**。理由：
  - 终点定义 UI 为自定义 Win11 Fluent 风（Mica/圆角/深浅色跟随系统）+ 简体中文文案，明确不依赖组件库——框架最大的价值（组件生态）用不上；
  - 交互形态是「侧栏 + 列表 + 详情 + 全局开关」的中等复杂度表单流，原生 TS + 少量工具函数足够，省掉框架运行时与抽象层，产物更贴近「绿色便携单 exe」的定位；
  - 深浅色跟随系统 = CSS `prefers-color-scheme` + 窗口主题 API，与框架无关。
- **次选：Svelte（编译期框架，运行时几乎零开销）或 Solid**——若实现中发现手写响应式代码量失控，可平滑换用，Tauri 侧零改动。
- React 生态最大但运行时最重，对自绘 Fluent 组件无增益；不推荐作为默认。
- 可选参考：微软 [@fluentui/web-components](https://learn.microsoft.com/en-us/windows/apps/web/fluent-ui)（Fluent UI Web Components）可作视觉/交互基准参考；本项目自写组件，不必引入依赖（本条为参考建议，未在官方 Tauri 文档中出现）。

---

## 5. 绿色便携单 exe：构建、限制与数据目录

### 结论要点

- **产物**：`tauri build` 的核心产物是 `target/release/<app>.exe`；不采用安装器（忽略 `target/release/bundle/` 下的 NSIS/MSI 产物）直接分发该 exe 即可。Tauri 官方讨论确认：裸 exe 在现代 Tauri 上**只依赖 WebView2**，CRT 已静态链接、无额外 dll（[Discussion #3048: Is it possible to create a standalone binary?](https://github.com/orgs/tauri-apps/discussions/3048)，含维护者侧回复「A plain tauri app shouldn't require anything… all it requires is WebView2」）。
- **唯一硬依赖：WebView2 Evergreen Runtime**。微软官方文档明确：**Evergreen Runtime 作为 Windows 11 操作系统的一部分预装**（[Evergreen vs. fixed version of the WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/evergreen-vs-fixed-version)、[Distribute your app and the WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)）。本项目目标平台只有 Win11 → 无需处理运行时分发。
- **不用安装器的限制**：
  - 无开始菜单快捷方式/卸载注册/文件关联（本项目均不需要，属可接受代价）；
  - **官方 updater 不能用于裸 exe**：updater 的 Windows 更新包就是 MSI/NSIS **安装器**的 zip + 签名，更新 = 下载并运行安装器（[Tauri v2 Updater 文档](https://v2.tauri.app/plugin/updater)）；官方没有一句直接明说「未安装应用不支持自更新」——由文档产物形态 + [Discussion #8963](https://github.com/orgs/tauri-apps/discussions/8963)（NSIS/MSI 更新路径讨论）推出，**该推论标注为非官方明示**。本项目自动更新 out of scope，不受影响；
  - 无签名 exe 会遇到 SmartScreen/未知发布者警告（Windows 机制；目标用户为自己与身边人，可接受。具体触发条件**未确证**）。
- **数据目录**：Tauri 的 `appDataDir()` 解析为 `${dataDir}/${bundleIdentifier}`，Windows 上 `dataDir` = `FOLDERID_RoamingAppData`（即 `%APPDATA%`），`appLocalDataDir` = `FOLDERID_LocalAppData`（`%LOCALAPPDATA%`）——**完全由 tauri.conf.json 的 identifier 决定，与 exe 所在位置无关**（[Tauri v2 path API 参考](https://v2.tauri.app/reference/javascript/api/namespacepath/)）。Tauri 没有内置「便携模式」开关。
- **便携场景的两个选项**：
  - **A. `%APPDATA%\<identifier>`**（Tauri 原生路径 API 直用）：稳定、多份 exe 拷贝共享同一份数据；代价是「删目录即卸载」的绿色语义不成立，快照不随 exe 走。
  - **B. exe 同目录 `data\` 子目录**（`std::env::current_exe()` 自行解析）：数据随 exe 走、拷走即备份、删目录即卸载，最贴合「绿色便携」终点定义。程序始终以管理员运行，写入用户目录下的自身目录无权限问题（只有放进 `Program Files` 这类受保护位置才有写入问题）。
- **推荐**：**B 为主、A 兜底**——默认解析 `<exe目录>\data\`，若目录不可写（被放进受保护位置）则回退 `%APPDATA%\<identifier>` 并在 UI 提示实际使用的目录。具体目录结构与保留策略留给「备份/恢复设计」工单定案。

---

## 对本项目的建议汇总

1. **注册表层**：选 winreg；所有打开/删除操作显式指定 `KEY_WOW64_64KEY`/`KEY_WOW64_32KEY` 双视图策略；管理员权限下写 HKCR 可行，per-user 数据写 `HKCU\Software\Classes`（语义细节归「经典菜单机制」工单）。
2. **提权**：`build.rs` 用 `WindowsAttributes::app_manifest` 注入含 `requireAdministrator` 的完整 manifest（保留默认 Common-Controls v6 依赖）；用 `PROFILE` 环境变量让 debug 构建不提权；NSIS `installMode` 不用动（本项目交付裸 exe，连安装器都不需要）。
3. **快照/恢复**：API 枚举 → JSON 快照为主存储；`reg.exe export` 生成 `.reg` 作人工恢复交付物；恢复用 API 精确写回，不依赖 reg import 的 merge 语义。
4. **前端**：`vanilla-ts` + Vite，自写 Fluent 风组件；备选 Svelte/Solid。
5. **交付**：直接分发 `target/release` 单 exe；数据目录 `<exe目录>\data\`（不可写时回退 `%APPDATA%`）；WebView2 依赖在 Win11 上天然满足。

## 版本现状（截至 2026-08-30）

| 组件 | 版本 | 说明 |
| --- | --- | --- |
| tauri | **2.11.5**（2026-07-01） | v2 线持续维护中（[全版本 changelog](https://v2.tauri.app/release/tauri/all-versions)） |
| tauri-build | 2.6.3（2026-06-17） | `app_manifest` 现行 API（[docs.rs](https://docs.rs/tauri-build/2.6.3/tauri_build/struct.WindowsAttributes.html)） |
| winreg | 0.56.0（2026-07-01） | 本调研所据版本（[docs.rs](https://docs.rs/winreg/0.56.0/winreg/)） |
| windows-registry | 0.6.1（2026-08-06） | windows-rs 官方（[docs.rs](https://docs.rs/windows-registry/0.6.1/windows_registry/)） |

各结论对应现状：`app_manifest` 提权 = **官方支持**（文档自带示例）；tauri.conf.json 提权开关 = **不存在**（#7173 open）；create-tauri-app 模板 = **官方维护齐全**；裸 exe 分发 = **可行且官方讨论认可**（updater 除外）。

## 未确证清单

1. windows-registry 能否经 `OpenOptions::access(u32)` 传原始 `KEY_WOW64_64KEY` 实现跨视图（文档未见一等支持，未确证）。
2. winreg `get_value::<String>` 对 `REG_EXPAND_SZ` 返回原始值还是环境变量展开值（文档未明示，需实测）。
3. 「裸 exe + requireAdministrator 同样弹 UAC」无官方直接表述，为机制推论（机制层面成立）。
4. 提权后 drag-drop 失效（#9271）在当前版本的修复状态未确证。
5. .reg 官方格式规范（经典 KB310516）的现行 URL 未打开确证；格式细节由多来源交叉印证，落地时以 reg.exe 导出文件做 golden 测试。
6. SmartScreen 对无签名 exe 的拦截/警告具体条件未确证。
7. 「官方 updater 不支持未安装应用」为文档产物形态推论，官方无一句明示（对本项目无影响）。
