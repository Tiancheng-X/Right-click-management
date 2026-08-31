---
id: T12
title: "自定义菜单项新增与编辑"
labels: [wayfinder:task]
status: closed
assignee: "本会话"
blocked-by: []
---

## Question

按「核心域模型与操作语义」D6 落地自定义条目：

1. 新增：向 HKCU 写 `shell\<verb>`（名称、图标、命令、挂载点=场景选择），v1 只进经典菜单；
2. 自己创建的条目可随时编辑（名称/图标/命令/场景）与删除；他人条目仅禁用/删除；
3. 图标选择：从用户选定的 exe 提取（复用 icons.rs）或选用 .ico 文件；
4. 写入前自动快照；完成后 toast + 待生效徽章。

## Resolution

完成（双侧编译全绿）：

- **自建标记**：创建时在条目键写 `MenuManager = "RightClickManager"` 标记值；枚举据此把 kind 判为 `custom`，前端「我创建的」条目显示 ✎ 编辑按钮（D6：他人条目仅禁用/删除）。
- **新增**：玻璃风模态表单（名称/命令行/图标/场景段选择 📄文件 vs 🖥️桌面）→ 写 `HKCU\Software\Classes\<挂载点>\shell\<RCM_唯一键名>`（MUIVerb + Icon + command + 标记值）；场景映射：file → `*`，desktop → `Directory\Background`。
- **编辑**：✎ 打开预填表单；场景变化 = 旧位置删 + 新位置建，快照同时记录两侧（新键前像 None + 旧键子树镜像），还原完全可逆。
- **图标**：原生文件选择器（rfd，exe/dll/ico）或手填路径，即时预览提取结果；写入 `Icon` 值后枚举时优先提取（回退命令行 exe）。
- **安全网**：写入前自动快照（create = 键不存在前像；update = 双侧前像）；toast + 待生效徽章。
- 新命令：`create_custom_entry` / `update_custom_entry` / `pick_icon_file` / `extract_icon_preview`。
