# G6 提醒呈现接入记录

2026-09-22：Task1 前端已实现；Task2 原生窗口与托盘仍待实施。此页不是实机验收或公开发布记录。

## 已实现

- `settings.html` / `src/entries/settings.ts`：喝水、起身活动、休息眼睛的开关与分钟间隔；全天/每日时段、全局稍后；明确保存与关闭，以及独立即时暂停/恢复。首次载入禁用表单，等待真实快照，无自动启用或自动提交。
- `reminder.html` / `src/entries/reminder.ts`：当前 presentation 的完成、稍后、收起。三行以内，同一批次已处理行保留禁用占位，新项追加；新 presentation ID 只使用新 items，不引入其他 pending 项。无前端业务倒计时。
- `adapters/reminder-dto.ts`：必需字段、枚举、有限范围、安全整数及固定完整三 ID 校验；输入按 ID 归一化。只校验 DTO，业务迁移仍由 Rust 负责。
- `adapters/tauri-reminders.ts`：先订阅再读；事件、get 与命令回复共用 revision 接受围栏。命令 Promise 返回本次回复供操作结果判断，但不会绕过控制器围栏更新界面。回应事件有独立的 revision 去重集合，较新快照不会吞掉较早回应。
- `main.ts` 仅一次订阅 `reminder-response`，complete/snoozeAll 对应现有 reminderCompleted/reminderSnoozed；只转发当前控制器，无排队和强制显示角色。

## Task2 需要接通的具体原生端口

| 页面/事件 | Task1 契约 | 待原生实现 |
|---|---|---|
| 设置窗口 | 页面 `settings.html`，建议 460×620；按钮调用当前窗口 `hide()` | 按需建窗、权限、关闭只隐藏；手动打开时允许 focus |
| 设置重新打开 | 监听无载荷事件 `reminder-settings-opened`，丢弃此前草稿、清操作提示并刷新 get；刷新期间新编辑不被覆盖 | 每次用户重新打开时发送该事件 |
| 提醒窗口 | 页面 `reminder.html`，目标 320×300；UI 仅发 Rust complete/snoozeAll/dismiss 命令 | 按需隐藏建窗、位置恢复、不抢焦点/首次点击/独立穿透策略 |
| 渲染握手 | DOM 渲染同步完成后调用 `reminder_ui_ready({ presentationId, windowToken })` | 处理该命令；初始化脚本注入 `window.__MARCH7_REMINDER_WINDOW_TOKEN__`，正安全整数；校验当前窗口/代次/当前 presentation |
| 收起/关闭 | 按钮捕获当前 presentation ID 发 dismiss，等待真实快照；不自行 hide 或完成事项 | 原生标题栏关闭也必须按当前 ID dismiss，并隐藏对应窗口 |

相同展示内容更新可以重复 ready。前端销毁后不再发起 ready，旧 ready 拒绝不会写回/记录错误；已发送 IPC 无法撤回，原生必须按 token/presentation/退出状态拒绝迟到握手。测试通过显式 fake transport 与 ready port 完成，不伪装已存在原生处理器。

## 草稿、反馈与生命周期

保存期间防重复提交；仅当本次回复与最新快照的 settings 都匹配提交值，runtimeError 为空，且实际持久化为 saved 时清草稿。unsaved 显示本次应用但未保存，并保留重试草稿。loading/readOnly/stopped 禁止修改配置；暂停与草稿无关。关闭丢弃未提交草稿，但已发送命令仍可能完成；重开会废弃上一轮的本地保存反馈。命令拒绝仍记录适配器日志，但界面提示只由拥有提交会话的控制器处理，避免旧拒绝覆盖重开后的新草稿或保存提示。

提醒页的头部与底部操作固定在各自区域，中间提醒/空状态/错误内容独立滚动；追加行和错误消息不推动底部按钮。较矮视口保留底部按钮与可滚动内容。

监听失败提示重启；已建立监听后的读取/操作错误提示重试或托盘恢复。部分订阅失败会释放已建立监听并挡住其排队回调。dispose 后迟到监听、读取、命令、响应、ready 错误均不更新 UI。用户可见文本使用 textContent，未使用用户数据 HTML。

## 自动验证与待验收

Task1 本地验证：114 项前端测试、5 项打包测试、25 个生产模块架构检查、TypeScript、Vite 多页生产构建通过。详细命令、红绿历史与环境限制见 `.superpowers/sdd/G6-IMPLEMENTATION/task-1-report.md`（工作交接记录）。Rust 未修改，沿用协调者已提供的 G5 证据，未重跑 Rust。

首轮审查发现的问题修复后仍待独立复审；浏览器模拟结果见下文。Task2 后才可开展 Windows/macOS 真正窗口焦点、按钮首次点击、穿透、关闭/恢复、睡眠/重启及真实提醒体验验收。当前不能称 G6 或 M3 已完成。

Task1 第一轮修复验证：真实入口、适配器与控制器的 4 项组合回归（旧保存拒绝、新会话草稿、新保存提示、当前失败和读取错误）通过；受影响 6 文件共 37 项测试、类型/架构检查及多页构建通过。协调者另在实际浏览器确认：320×300 下同批次 1→2→3 项再出现运行错误，footer y=239.302 / bottom=300 保持不变；320×200 下 footer y=139.302 / bottom=200，内容可滚动，点击收起得到无展示状态。此为模拟宿主浏览器证据，不是原生实机验收。
