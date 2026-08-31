---
id: T10
title: "条目操作接入（禁用/启用/删除）"
labels: [wayfinder:task]
status: closed
assignee: "本会话"
blocked-by: []
---

## Question

按「核心域模型与操作语义」D2/D3 与「异常与边界处理」落地真实操作：

1. 禁用：动词/自定义加 `LegacyDisable`；shellex/打包写 Blocked 键（CLSID 空串值）；系统内置只读；启用 = 反向移除；
2. 删除：断开挂接（不碰 CLSID 本体），执行前强制自动快照；
3. 卡片开关接真数据：当前状态从注册表推导（动词有无 `LegacyDisable` 值、shellex 的 CLSID 是否在 Blocked 键）；
4. 操作后走分类 toast + 「待生效」徽章（一键重启 explorer）。

## Resolution

完成（双侧编译全绿）：

- **禁用/启用**：动词/自定义 = `LegacyDisable` 值写入/删除；shellex = HKCU `Shell Extensions Blocked` 键写/删 CLSID 空串值。全部「写值不删键」可逆；写入位置 HKCU 优先、回退 HKLM（始终管理员运行，合并视图优先级与 T2 一致）。
- **状态推导**：`MenuEntry` 新增 `clsid` 与 `enabled` 字段——动词查有无 `LegacyDisable`，shellex 查 CLSID 是否在 Blocked 集合；开关初始态即真实状态。
- **删除**：递归删除挂载子键（先清子键再删自身，不碰 CLSID 本体）；删除前用 `reg.exe export` 强制导出 .reg 到 `data\snapshots\`（便携优先、回退 %APPDATA%），**导出失败即中止删除**。
- **前端**：弹簧开关接真数据（乐观更新、失败回滚）；两段式删除确认（悬停显 🗑 → 「删除？已自动备份」→ 确认/取消）；「待生效」徽章常驻侧栏 + 一键重启 explorer；分类 toast（ok/warn/err）。
- 新命令：`set_entry_enabled` / `delete_entry` / `restart_explorer`。
