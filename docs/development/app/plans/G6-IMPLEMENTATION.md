# G6 日常提醒：界面、原生窗口与角色回应

状态：实施准备。后端基线1251383已通过任务与整合审查、本地77项Rust及79项前端检查，双平台CI/构建仍待核实。用户允许实机待验收时继续独立开发。本计划不关闭M0–M3，不声称真实使用价值成立。

目标：从托盘打开提醒设置，启用所需三项；到期在独立可交互气泡确认/稍后/收起；已收起事项始终可从托盘查看，角色按当前选择回应。具体时间/状态规则以 `../REMINDER-SPEC.md` 和已实现Rust模型为准，不在前端重新实现。

## Global Constraints

- Rust是状态、业务计时和持久化唯一权威。前端只保留表单草稿/呈现状态，禁止自行计算到期、完成重计、安静期或写文件。
- 首启动全部关闭，用户确认设置后才启用。默认间隔只是可调整偏好，不宣称健康建议或真实健康行为。
- 自动气泡不抢焦点，独立于角色穿透；隐藏角色不暂停提醒。原生托盘入口可恢复；不再发送另一套OS通知。
- 两角色共用规则与情境入口，角色切换不重置提醒。完成/稍后反馈只从原生事件播放一次，不在命令回复再播。
- 保留位置/穿透/关闭/退出与单个app_context宏，退出后迟到回调不复活窗口/循环。未保存、只读、加载、时钟错误不能显示成成功。
- 不做专注/任务、导入导出、自动启动、账号、外部活动监测或公开发布。实施者无子代理、push、GUI实机、全局安装、主线合并。

## Task 1: 真实后端接口的前端设置与提示视图

只实施前端与构建入口，原生建窗/托盘属于Task2；在Task2前不宣称用户已能从应用打开这些页面。

1. 建立窄的 `domain/reminder.ts` DTO类型和适配层校验，读取现有Rust model/service/native确认字段，不猜命名。Snapshot为revision/settings/progress/paused/quiet/snoozePending/presentation/persistence/runtimeError/stopped；mode为automatic/manual，save状态loading/default/saved/unsaved/readOnly。固定三ID按ID匹配，不依赖数组位置。检查必需类型、唯一完整ID集合、枚举、安全整数/有限边界，拒绝损坏快照。不要在DTO解析器复制整个业务状态机。runtimeError是业务操作/时钟错误信号，不能只因IPC Promise resolved就提示保存成功。
2. `adapters/tauri-reminders.ts` 连接 get_reminders、reminder_command({command})、reminders-changed、reminder-response。读取源码确认命令tag/字段，allDay只发送{kind:"allDay"}，不夹带daily字段。订阅先建立再读取，事件/get/命令回复统一经过revision围栏；旧回复不得回退界面。错误区分listen未建立需重启与可恢复读取/操作失败，迟到监听/回复/错误在dispose后无副作用。重复响应事件按revision去重，但不要因较新的状态快照而丢掉独立回应事件。
3. 可为现有角色与提醒两个实际消费者提取一个窄快照连接工具，但仅在确实复用相同机制时做；保留角色测试与对外接口，不能建立通用事件总线或插件协议。不改变Rust业务规则以迎合UI。
4. 新增 settings.html、reminder.html 和明确入口模块（建议src/entries/settings.ts、reminder.ts），Vite明确多页输入，架构检查器允许这些真实装配入口并继续检查层级。main.ts仍是角色装配，不用一个庞大URL路由函数承载所有页面。生产构建必须实际含这两页。提醒视图在每次渲染实际presentation后调用宿主ready端口；适配器调用reminder_ui_ready({presentationId,windowToken})，windowToken读取原生注入的__MARCH7_REMINDER_WINDOW_TOKEN__（安全整数）。这是呈现握手，不改变业务状态；重复/迟到ready必须可清理。Task2实现命令与注入，测试使用显式模拟宿主。
5. 设置界面使用清楚的中文标签：喝水/起身活动/休息眼睛分别开关与1–1440分钟间隔；全天或每日活动起止时间（支持跨午夜，起止相同提示改用全天）；稍后1–120分钟；保存设置与关闭。暂停/恢复是明确即时命令，与未提交草稿分开。提示默认关闭、需保存后生效；不自动勾选/提交。不要增加完整设置中心或未来功能占位按钮。
6. 表单是本地草稿，后台快照/命令乱序不能覆盖用户正在编辑的字段。保存期间防重复提交；根据实际快照判定应用与保存状态，runtimeError或settings未匹配提交内容时不能清草稿并报成功。unsaved说明本次应用但未保存，readOnly/loading/stopped禁止修改配置；只读不伪造设置已启用。关闭丢弃未提交草稿；已提交命令仍可能完成。Task2会在用户再次打开时发送 reminder-settings-opened 事件，界面据最新快照重设草稿。
7. 气泡只显示当前presentation；null显示无展示状态，手动空集合显示“暂无待处理提醒”。每项“完成”，底部“稍后提醒”和“收起”。已处理行在本次展示内保留占位并禁用，新增项追加，避免按钮移动使快速点击误处理另一项；新presentationId重建行集合，不能把旧批次遗漏项自动加入。处理状态由快照推导，不把本地按钮点击当成成功。关闭/收起发送dismiss当前ID，保留pending；不自开业务倒计时。
8. main只订阅一次reminder-response，将complete/snoozeAll映射为现有reminderCompleted/reminderSnoozed，对当前presentation控制器回应；未初始化/已销毁时不排队回放，角色隐藏不强制显示。get/命令/快照本身不触发角色动作。窗口内所有文字使用textContent，不插入用户数据HTML。
9. 采用与已有短句一致的暖白/深灰、细边、少量克制强调色，清晰中文系统字体；按钮有hover/active/focus-visible/disabled状态、原生表单标签与合理键盘顺序，支持明暗系统主题。设置默认约460×620并能在较矮视口滚动，底部保存/关闭保持可达；气泡目标320×300，三项以内，内容更新不移动已有按钮。先保证可读与稳定，不用巨型圆角、紫色渐变或卡片网格。错误用短中文说明恢复动作，技术原因仅日志。
10. 条目和表单控制器通过窄视图/宿主端口测试，关键行为先红绿：快照/回复乱序、listen失败与dispose、只读/加载/未保存/时钟错误、草稿不被后台覆盖、保存反馈真实性、完整命令payload、三项完成与行稳定、旧dismissID、响应去重/换角/销毁。浏览器验证由协调者随后做。运行前端完整check，Rust无变化复用证据；更新G6说明和架构的已实现/待原生边界，提交不push。

## Task 2: 原生托盘、非聚焦气泡和设置窗口

依赖Task1审查通过，接口采用实际Task1实现。只修改必要原生接线、窗口配置/能力、文档与受影响测试，不重写提醒模型。

1. 新增专门的reminder UI协调模块，读取Rust权威最新Snapshot并控制两个实际窗口；保持提醒model/store/service不拥有Tauri窗口。settings/reminder窗口按需创建（不在首次全关闭时预建两个WebView），创建时隐藏、对应Task1页面；settings只因用户打开而show+focus，reminder始终focus=false/focusable=false并可接收鼠标，主角色继续现有模式。只建立所需具体窗口职责，不做通用窗口框架。
2. 锁定Tauri2.11.5 webview_window.rs:897–900、tauri-utils2.9.3 config.rs:2068–2071、Wry0.55.1 wry_web_view.rs:59–61已核实acceptFirstMouse默认false会控制Mac非活动窗口首次点击。为main和reminder显式开启acceptFirstMouse，保持非聚焦保护；增加真实配置/策略回归。源码/配置检查不能替代真实Mac首次点击/拖动和不抢焦点实测。
3. 托盘增加提醒设置、查看待处理、暂停/恢复以及必要的简短保存/错误状态；角色隐藏/穿透时均可用。菜单与前端共用已有原生Service命令，不能复制规则或时间。ShowPending由Rust创建manual意图，空集合也可查看。加载/停止/保护状态如实反映；暂停勾选只据实际Snapshot，不把原生自动勾选当业务成功。
4. 将原生服务变化连接到UI协调者；主线程执行窗口/菜单效果且不持服务锁，优先读取最新Snapshot避免队列中的旧快照重新显示已收起视图。可统一发布最新状态快照，回应事件仍按原始动作revision只发一次。展示代次、revision和退出围栏覆盖所有异步建窗、位置稳定与显示回调；禁止旧自动关闭/迟到show关闭或复活新的manual窗口。为每次提醒窗口创建分配windowToken并用初始化脚本注入；reminder_ui_ready只接受本窗口、当前token和当前presentationId。位置已稳定且当前视图ready后才show；销毁/退出废弃旧token/任务，页面重载重新取快照并ready，不用前端ready当真人已看到的证据。
5. 气泡靠角色所在可用显示器放置（优先上方，不够则下方/工作区内），不要求自动识别其他应用。复用/适度抽出已有desktop坐标与观察后稳定的恢复机制，不能重犯Mac每屏physical坐标/当前scale混用。用实际外窗尺寸约束，负原点、不同缩放、屏幕变化均需可达；超小工作区可缩小视口并滚动，设置窗也应适配高DPI工作区，不把默认高度强行伸到屏幕外。位置/大小在同批次尽量稳定。
6. settings关闭只隐藏该窗口，不结束应用或修改提醒；每次用户打开发送reminder-settings-opened，首次加载也有真实快照初始化。reminder关闭/收起只发当前展示dismiss并隐藏，不完成业务项；正常无回应由后端10秒状态推进收起。不要在前端再造业务计时器；UI故障/显示失败保持pending可从托盘恢复并报告，不能宣称用户已经看见。
7. 只授予这些页面需要的Tauri事件/窗口能力，保留主窗口拖动权限，不加文件访问或任意系统执行。新增UI命令限定具体操作/窗口，应用schema与主setup/exit组合显式、单context宏；无硬编码角色ID分支。
8. 自动显示不调用set_focus/应用activate。设置手动打开允许focus，提醒按钮在角色穿透下仍可交互；不发OS toast/声音/另一个重复通知。角色窗口显示隐藏不会因为提醒变化而改变。
9. 控制器/策略 seam覆盖：旧snapshot不重开、同ID更新不重复show、autonull隐藏、manualID不受旧任务影响、关闭不是完成、停止拒绝迟到建窗/回调、负原点/混合DPI及小工作区、settings focus仅手动、main/reminder首次鼠标配置。有针对性的原生故障测试；完整Rustfmt/test/clippy/build和前端check，保留真实GUI缺口。
10. 更新用户README、ARCHITECTURE、TODO、G6记录与随包regression说明/清单/结果模板；教用户从托盘确认设置、查看收起事项、暂停/退出，写清首启全关、全部本地、无账号。区分独立实现、同提交自动构建、浏览器模拟、真正Windows/Mac体验及真实反馈，不关闭M3或称正式日用首版完成。提交不push，协调者随后独立审查、同提交双平台构建和包校验。

## 验收与后续

技术目标是用户路径已接通、相关测试与审查通过、两平台同提交构建可分发。Windows与Mac实际气泡按钮/首次点击/焦点、穿透恢复、睡眠/重启、提醒有用不烦仍要真人验证；不是自动检查能代填的结果。后续G7处理完整试用、导出导入/迁移与安装体验，M5扩展仍服从价值和发布门槛。
