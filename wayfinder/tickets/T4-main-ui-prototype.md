---
id: T4
title: "主界面原型"
labels: [wayfinder:prototype]
status: closed
assignee: "本会话"
blocked-by: []
---

## Question

Win11 Fluent 风、文案简体中文的主界面长什么样、顺不顺手？做一个 HTML/CSS 可点击原型：菜单条目列表与按场景/来源分组、条目详情（名称/图标/命令/位置）、启用-禁用-删除操作、「一键切回经典菜单」开关、快照历史页。做完请用户直观反应，改到满意为止；原型文件作为资产链接到本工单，作为后续真实开发的视觉基准。

调用 "prototype" 技能构建，用 "frontend-design" 打磨视觉。

## Resolution

用户在三变体中选定 **A · 资源管理器三栏**（左场景/来源导航树 + 中条目列表 + 右常驻详情面板）作为主界面布局与视觉基准。

- 原型资产：[prototype/main-ui.html](../../prototype/main-ui.html)（自包含单文件，`?variant=A|B|C` 切换；变体 B 卡片墙、C 高密度工作台保留在文件内，供后续「偷细节」，如 B 的一键切经典菜单卡片与统计徽章、C 的批量操作条与挂载点列）。
- 设计语言已验证可行：Fluent 令牌（Mica 噪点 + 层级透明卡片、8/4px 圆角、Win11 强调色、Segoe UI Variable、深浅双主题、Fluent toggle/对话框/抽屉）在纯 HTML/CSS/JS 下即可高保真呈现，迁移到 Web 前端无风格损耗。
- 交互语义已用户过目：条目行内开关、删除内联确认、切经典菜单弹「重启资源管理器」横幅、快照时间线还原、可疑残留警示徽章。
