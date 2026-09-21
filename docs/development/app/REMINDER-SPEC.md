# G5/G6 提醒实施规格

来源：已批准长期计划。G5 Task 1 已实现原生后端，具体自动验证见本节后的实现合同及 G5 实现报告；G6 窗口、设置及双平台人工验收仍待完成。提醒独立存储，不覆盖角色/桌面配置。

## 用户可观察行为

- 三类固定提醒：water喝水、move起身活动、eyes休息眼睛。首次启动全部停用，直到用户在设置中确认。设置初始建议为60/60/30分钟、活动时段09:00–22:00、稍后10分钟；明确只是可改的个人偏好起点，不是健康建议。
- 每项开关与间隔独立，范围1–1440分钟、整数。全局稍后1–120分钟、整数。活动时段支持全天或单个每日区间，起止不同，支持跨午夜，按当前本地时间解释。
- 首次启用、关闭再启用、修改间隔：该项从成功应用设置时重新计时，清除该项旧待处理；其它项不变。关闭立即撤销该项待处理。
- 完成单项：只清除该项并从完成时重新计时。重复完成非待处理项不重置截止时间。完成不能视为实际健康行为证据。
- 到期成为pending，一项最多一条，不按漏过的周期累计；在暂停/活动时段外/安静期内继续判断到期但不主动展示。
- 自动收起或手动收起只结束此次展示并保留pending。已提示过的pending不会因轮询、睡眠恢复、活动时段切换或重启反复弹出。
- 新自动批次只包含尚未展示的pending事项，不把已收起的旧事项重新弹出。正在展示时加入新到期项可更新同一个窗口，不创建堆叠窗口，也不延长已有自动收起截止时间。全局稍后到期与用户主动查看才允许合并全部pending。
- 全局稍后是明确再次提醒的请求：收起当前展示并设quietUntil，期间所有新到期保持安静；期满且允许展示时，把所有pending合并再提示一次。期满仍在暂停/活动时段外时，保留一次合并提示意图，到允许时再消费。无pending也可稍后，使后续到期安静。
- 手动暂停只阻止主动提示，不隐藏角色、不清除pending、不重置各项截止时间；恢复仅合并尚未提示的事项或尚未消费的稍后意图，不重复提示已收起项目。隐藏角色不自动暂停提醒。
- 固定托盘入口“查看待处理”始终可用，用户主动查看可显示所有pending，不受活动时段/暂停/稍后限制；查看不修改这些全局控制，不算完成。无pending显示简短空状态。
- 提示窗口独立于角色窗口，主角色穿透不影响按钮；自动展示不抢焦点。设置窗口只因用户主动打开获取焦点。按钮为逐项“完成”、全局“稍后提醒”、收起；自动提示10秒收起，手动打开保持直到操作/关闭。消息文本不含内部实现词。
- 点击完成与稍后调用当前角色情境回应。切换角色不更改提醒状态；完成和稍后动作不用前端动画回调确认业务成功。

## 时间与数据合同

- Rust纯引擎接收显式时间样本（UTC毫秒、本地分钟、进程单调时间），无系统调用。短暂展示的收起不能因墙上时钟回拨无限延长；睡眠后也不保持已过期气泡。运行适配器低频调度并处理命令，UI只读快照/发命令，不开第二个业务计时器。
- 持久化截止时间，重启评估当前状态，只合并当前到期事项，不重播过去事件。非有限/越界时间与间隔拒绝，计算用checked/saturating边界并明确错误。
- 向前时钟跳变视为当前到期，最多一个pending；向后跳变若剩余超过原间隔，将未来截止限制为now+interval；quietUntil同样最多剩余已选snoozeDuration。这样不会无限延期；不声称能识别用户真实生理作息。暂停不冻结墙上时间。
- 记录已提示状态与一次性snooze意图，跨重启防重复。开始展示时先推进逻辑状态，再尽力持久化并向UI推快照；失败明确标记未保存，保留会话内状态，不能伪称下次启动仍会保持。
- 单独versioned reminders.json，唯一Rust写入者。迁移/备份/原子写复用实际公共IO工具，具体schema仍各自验证。损坏/未来版本保留原件、关闭自动提醒并报告待恢复，不擅自覆盖或重置。
- 没有后台统计服务、应用活动采集或任务文本采集。导出导入安排在G7，不能直接让页面写文件。
- 快照带revision；订阅与首次读取防乱序，迟到IPC/销毁不重新启动展示；未回应记录由Rust权威维护，页面reload不能重复弹出。

## 验收重点与停止边界

G5交付纯引擎、文件存储、原生调度和命令状态转换测试，G6接独立气泡、必要设置及端到端体验。测试必须覆盖同时到期、稍后中新增、未回应重看、重启/睡眠/跨午夜/正反时钟跳变、设置变更、非法输入、损坏/未来配置、IO失败和并发命令顺序。

G5自动通过不代表G6体验通过；G6需要两平台穿透下操作、无焦点抢夺、睡眠/重启实测和真实使用反馈。未取得这些证据不关闭M3，不通过填造用户反馈进入M4/M5。用户允许的“先完成独立部分”可准备后续实验开关/文档，但不能默认证明需求。

## G5 Task 1 实际后端合同（2026-09-22）

实现位于 `src-tauri/src/reminders/`：`model.rs` 是注入时间的纯规则，`store.rs` 校验 v1 并复用 `atomic_file::replace`，`service.rs` 独占引擎、文件与命令队列，`native.rs` 适配当地时间、IPC 与事件。`lib.rs` 在现有角色、桌面 setup 后启动提醒，并在首次 ExitRequested/Exit 停止；仍只有一个 app_context/generate_context。chrono 0.4.45 已在锁文件中，本轮仅加直接依赖及 `clock` feature，不升级任何包。

**本阶段没有气泡、设置窗口或查看待处理托盘按钮。** 初始 water/move/eyes 均关闭。引擎中的 presentation/autoHandled 只表示软件处理了展示意图，不证明窗口显示、真人看到或执行健康行为。G6 必须接独立不抢焦点气泡、设置、固定查看入口，并将情境回应事件接到当时选中的角色。自动测试不关闭 M3。

### 手动查看的补充裁定

手动查看不完成事项、不修改暂停或 quiet，期满前不取消明确的 snoozePending。手动视图当前显示的 pending 及打开期间新加入的 pending 会标记普通 autoHandled；因此“暂停→到期→手动查看→收起→恢复”不立即重弹刚收起的项目。“稍后→手动查看→收起→期满”仍按明确稍后请求合并一次。手动视图在暂停/活动时段外也持续更新，没有自动倒计时。quiet 到期且允许普通展示时，已开的手动视图就地消费该次 snoozePending，保持同一 ID、manual 模式与无截止时间，关闭后不再立即重弹自动视图。若 dismiss 恰是首个到期 step，按关闭前的手动视图处理已到期请求与新增 pending，然后关闭。期满仍在暂停/活动时段外时保留意图，直到允许时再处理；同一步新的 snoozeAll 建立新的未来 quiet/request，不被旧请求的消费删除。这里的处理标记仍不是真人看到/完成证据。

### 文件格式与时间界限

`app_config_dir()/reminders.json` 首个版本为 v1，无虚构 v0 迁移。所有对象拒绝未知字段，数组固定三个元素并按 water/move/eyes 顺序排列，拒绝缺失、重复、未知或错序 ID。

```json
{
  "version": 1,
  "settings": {
    "items": [
      { "id": "water", "enabled": false, "intervalMinutes": 60 },
      { "id": "move", "enabled": false, "intervalMinutes": 60 },
      { "id": "eyes", "enabled": false, "intervalMinutes": 30 }
    ],
    "activeHours": { "kind": "daily", "start": 540, "end": 1320 },
    "snoozeMinutes": 10
  },
  "progress": [
    { "id": "water", "nextDueAt": null, "pending": false, "autoHandled": false },
    { "id": "move", "nextDueAt": null, "pending": false, "autoHandled": false },
    { "id": "eyes", "nextDueAt": null, "pending": false, "autoHandled": false }
  ],
  "paused": false,
  "quiet": null,
  "snoozePending": false
}
```

- `activeHours` 也可为 `{ "kind": "allDay" }`。daily start/end 为 0–1439 且不同，起点包含/终点不含；跨午夜为 start>end。
- `nextDueAt` 与 `quiet.until` 是整数 UTC 毫秒，范围 0–253402300799999（Unix epoch 至 9999 年末）。本地分钟必须在 0–1439；单调采样为 0–9007199254740991 毫秒。越界拒绝，所有新增截止均检查加法；不以溢出制造到期。暂停不冻结截止。
- 开启且非 pending 必须恰有 nextDueAt；pending 不得有 nextDueAt；关闭不得有 pending/截止；autoHandled 只能用于 pending。quiet 为 `{ "until": ..., "durationMinutes": 10 }` 且须有 snoozePending。quiet 清空后允许保留 snoozePending，表示过期但尚在暂停或活动时段外的明确意图；允许展示时，已开的手动视图可就地消费该意图。
- 回拨/重启将未来截止剩余上限限制为当前间隔；quiet 上限使用建立该安静期时保存的 durationMinutes，不因之后更改默认稍后时长而缩短。前跳/睡眠只处理当前到期集合。
- presentation、revision、单调截止和保存状态是会话数据，不进文件。自动展示 10 秒，墙钟或单调截止任一到达即收起；加入新项保持原 presentation ID 和截止。重启不恢复旧 presentation，已处理 pending 仍待处理而不重复自动提示。

首次无文件仅使用未保存默认值；有实际业务变化才保存。替换前重新读取并校验已有内容，有效旧版本备份到 `reminders.json.bak`，调用公共原子替换保留完整旧/新文件。损坏、未知版本、读失败保留原件，当次只读并禁自动展示；配置更新返回 readOnly，手动暂停/查看/收起仍可用。运行中发现外部文件变坏也切换保护状态，不覆盖。真实升级迁移、恢复 UI、导出导入安排在 G7。

### IPC 与事件

只有两个入口：

- `get_reminders()` 返回当前快照（启动读取期间 persistence=loading，退出后 stopped=true 仍可只读快照）。
- `reminder_command({ command })`：固定联合 `{type:"updateSettings",settings}`、`{type:"complete",id}`、`{type:"snoozeAll"}`、`{type:"setPaused",paused}`、`{type:"showPending"}`、`{type:"dismiss",presentationId}`。没有传时间、截止或完整状态的入口。CommandInput 仅把 wire JSON 转为这个 Rust 联合，把 malformed payload 统一转为 `{code:"invalidCommand"}`；不存在任意 JSON 状态操作。

语义错误返回 `{code}`，包括 invalidSettings、invalidTime、invalidPresentationId、readOnly、stopped、queueFull、workerUnavailable、sequenceExhausted。非法形状/未知额外字段/未知 ID 返回 invalidCommand。Tauri 自身无法解析的 IPC 信封仍属宿主传输错误。

成功命令回复是应用后快照：`revision, settings, progress, paused, quiet, snoozePending, presentation, persistence, runtimeError, stopped`。presentation 为 null 或 `{id,mode,items,closesAt}`；mode=automatic/manual，手动 closesAt=null，空 items 是合法手动空状态。revision 和展示 ID 都是 JS 安全整数；旧 dismiss ID 不能关闭新视图。订阅先于首次读取，G6 应按 revision 丢弃迟到快照，销毁后丢弃所有回调。

persistence 为 `{status,code}`：loading、default（尚未落盘）、saved、unsaved（本次已应用但保存失败）、readOnly（受保护，仅会话状态）。普通写失败不撤销已应用业务；返回/事件必须保留 unsaved + writeFailed，不能以命令 resolve 视为已持久化。之后合法命令（包括相同设置）可重试；无变化 tick 不重试、不重复写盘。读取错误码为 directoryUnavailable/readFailed/invalidFile/unsupportedVersion。时钟错误显示 runtimeError，重复同一故障不刷事件/日志，合法样本恢复后清除。

原生只在状态变化时发送 `reminders-changed`。实际应用 complete/snoozeAll 另发送一次 `reminder-response`，形如 `{revision,type:"complete",id:"water"}` 或 `{revision,type:"snoozeAll"}`，不含角色 ID。重复完成非 pending 不发回应。**G6 以 reminder-response 事件触发角色反馈，命令回复只同步状态；不得两边各播一次。**

### 调度、失败与退出

一条专属 reminders 工作线程串行启动读取、显式命令、时间推进和文件 IO。最多 128 条排队命令；无业务时等命令，有开启提醒/quiet/自动展示时最多每秒唤醒。快照/队列锁只涵盖内存访问，不覆盖磁盘 IO 或原生调用；每 tick 不新建线程。IPC 等待回复在 Tauri blocking pool，主线程不等待也不 join。

首次退出立即置 stopped、清展示、拒绝新业务命令、取消排队请求并唤醒线程。启动读取在途时退出，读取返回后先检查 stopped，不开始初次时钟采样/pump/保存/通知。已开始的原子 IO 可完成，但退出后不发布其状态/成功回复/事件；退出前已提交的 IPC 回复仍可能迟到，由 G6 销毁保护丢弃。未确认排队动作不保证保存，进程先结束时磁盘只能保证完整旧/新版本，不保证全部点击保存。后台排队的原生回调在主线程再检查 stopped，不能复活服务或展示。desktop 原有末次位置保存退出流程不变。

自动验证使用生产纯模型、手动 pump、临时文件与通道屏障；没有真实睡眠猜时序。原生窗口/休眠/重启操作和双平台 CI 由后续同提交验收记录提供，本地 Rust/前端检查不代替这些证据。
