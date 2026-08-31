---
id: T13
title: "一键切回经典菜单"
labels: [wayfinder:task]
status: closed
assignee: "本会话"
blocked-by: []
---

## Question

按「Win11 新版右键菜单机制调研」与「异常与边界处理」Q2 落地全局开关：

1. 开 = `HKCU\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32` 建**空字符串**默认值；关 = 清除；写前自动快照；
2. 状态以注册表键为准（启动自检），开关旁注明「系统更新可能使其失效，重新开启即可」；
3. 切换后 toast + 待生效徽章（一键重启 explorer）；
4. 入口位置参考原型 B 的醒目卡片样式（偷细节）。

## Resolution

完成（双侧编译全绿）：

- **状态**：`classic_menu_state()` 以注册表键为准——`InprocServer32` 存在且默认值为空串 = 开，否则 = 关；启动自检同步开关位置，系统更新清键后自然显示为关（T9 Q2，不做深度探测）。
- **切换**：开 = 建**空字符串**默认值；关 = 仅删该默认值（保守不删键）。写前自动留值级前像快照（kind = classic），可从快照历史还原。
- **前端**：侧栏玻璃卡片（偷原型 B hero 卡样式：🍃 标题 + 弹簧开关 + 说明「系统更新可能使其失效，重新开启即可」）；乐观更新失败回滚；toast + 待生效徽章一键重启。
- 新命令：`classic_menu_state` / `set_classic_menu`。
