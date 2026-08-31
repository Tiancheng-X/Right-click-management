---
id: T11
title: "快照与时间点还原落地"
labels: [wayfinder:task]
status: closed
assignee: "本会话"
blocked-by: [T10]
---

## Question

按「备份/恢复设计」B1–B4 落地：

1. JSON 主存储的操作级自动快照（每操作一录全带前像），删除类额外导出 .reg；
2. 手动全量快照点；
3. 存储 exe 同目录 `data\snapshots\`，不可写回退 %APPDATA%；滚动保留 60 条，删除类与手动点永不自动清理；
4. 快照历史 UI + 还原流程：还原前自动留档 → 逐键精确写回 → 结果报告（含外部改动过的键）→「撤销本次还原」；失败走分类 toast + 撤销闭环（T9 Q5）。

## Resolution

完成（双侧编译全绿零警告）：

- **快照存储**（`snapshots.rs`）：每操作一录全带前像的 JSON 单文件存储（`data\snapshots\`，便携优先回退 %APPDATA%）；前像两种粒度统一为 `SnapshotEntry`——值级（禁用/启用的单值状态）与子树级（删除/手动快照的递归键镜像，含全部值的 vtype+原始字节）；删除类附 .reg 且 `protected`（永不自动清理），自动记录滚动保留 60 条。
- **自动留档接入**：`set_entry_enabled` / `delete_entry` 在变更前捕获前像、成功后落盘（失败不产生快照）；删除 = 子树前像 + .reg 双保险。
- **还原流程**：还原前自动拍「还原前留档」（protected，可撤销）→ 逆序逐键精确写回（原生 `RegQueryValueExW/RegSetValueExW`，绕开 winreg 的 RegType 包装；重建缺失子树）→ 结果报告 `written/notes/undo_id`——外部改动过的键如实列入 notes。
- **前端**：侧栏新增「🕘 快照历史」视图（时间、动作、键数、永久/附 .reg 徽章），两段式还原确认，结果横幅含「撤销本次还原」；「📷 手动快照」按钮（全量受管键，protected）。
- 新命令：`list_snapshots` / `restore_snapshot` / `create_manual_snapshot`；`set_entry_enabled` / `delete_entry` 增加 `name` 参数用于快照描述。
