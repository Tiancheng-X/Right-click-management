---
id: T15
title: "设置页"
labels: [wayfinder:task]
status: closed
assignee: "本会话"
blocked-by: [T11]
---

## Question

聚合分散的配置与状态：

1. 快照保留策略（默认 60 条、删除类与手动点永不清）可调；清空历史（清空前提示）；
2. 数据目录展示（便携优先 / 回退位置）与打开入口；
3. 策略检测状态展示（「异常与边界处理」Q4 的常驻提示细则）；
4. 关于页（版本、wayfinder 地图链接）。

## Resolution

完成（双侧编译全绿）：

- **设置视图**：侧栏 ⚙ 进入（第三视图），四张设置卡片——快照保留 / 数据目录 / 系统策略 / 关于。
- **快照保留**：自动保留条数可调（5–500，持久化到 data\settings.json，保存即触发一次滚动清理）；「清空自动记录」与「清空全部（含手动点与删除备份）」两段式确认。
- **数据目录**：展示实际路径 + 便携模式/回退模式徽章（T6 B2 规则）+ 一键打开目录（tauri-plugin-opener Rust 侧调用，免 capability）。
- **系统策略**：启动与进入设置页时读 NoViewContextMenu / NoTrayContextMenu（HKCU+HKLM），命中则设置页详述来源、**侧栏状态栏常驻 ⚠ 提示**（T9 Q4 落地）。
- **关于**：版本号（CARGO_PKG_VERSION）+ 技术栈 + 指向 wayfinder 地图。
- 新命令：`get_settings` / `set_auto_keep` / `clear_snapshots` / `open_data_dir`；`snapshots.rs` 增加设置持久化与可配置清理上限。
