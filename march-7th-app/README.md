# March 7th

March 7th 是从现有 Codex v2 桌宠迁移而来的独立 Tauri 2 桌面应用。

开发说明：[长期计划](../docs/development/app/PRODUCT-PLAN.md) · [架构与扩展](../docs/development/app/ARCHITECTURE.md) · [贡献约定](../CONTRIBUTING.md)。

当前构建可在托盘“提醒设置”底部展开“版本信息”，核对版本、平台、完整提交和本地修改／来源未知状态。它应与测试包 `BUILD-INFO.json` 一致；完整 SHA 不代表正式发布或桌面验收。开发和 CI 可对本次构建的程序使用 `--build-info`，在创建窗口、worker 或读取用户配置前输出同一 JSON 并退出。范围与打包规则见 [G7-A 构建身份](../docs/development/app/G7-BUILD-IDENTITY.md)。

当前 M5-A2a 候选启动时把旧三项配置迁移至含 `focus.json` 的 v2 活动数据集（专注入口尚未启用），新备份导出采用 v2、仍可导入旧 v1。迁移和恢复旧版的边界见 [四项数据集说明](../docs/development/app/M5-DATASET-V2.md)。应用在启动业务服务前持有 `instance.lock` 的操作系统排他锁。同目录已有采用此协议的实例时，第二次启动直接正常结束，不唤醒或结束已有进程。旧版没有这个协议，升级前仍须先退出旧实例。锁文件会保留，不能凭文件存在判断程序仍运行，也不要删除它来“解锁”。目录或锁不可用时，角色选择只在本次会话有效、提醒默认关闭且只读、位置不可保存，不改用另一数据目录；原有单个文件的损坏/未来格式保护继续有效。实现和验收边界见 [G7-B1 说明](../docs/development/app/G7-DATA-DIRECTORY.md)。

`src/main.ts` 只负责装配；动画状态在 `domain/`，角色参数在 `characters/`，运行调度在 `application/`，Tauri/DOM/浏览器时钟在 `adapters/`。当前包含三月七与雷电将军两个内置角色，可从托盘“角色”菜单切换；Rust 保存选择，前端验证快照并安全切换图集。已接通默认关闭的本地提醒、提醒设置与待处理气泡，实际桌面体验仍需分平台验收。

## 历史基线：0.2.0（以下实机结果仅属于原记录提交）

- 透明、无边框、始终置顶的宠物窗口
- 复用已封存的 `1536 × 2288` v2 精灵图
- 六帧待机循环
- 根据真实系统鼠标位置切换 16 个视线方向
- macOS Apple Silicon 原生光标读取
- Windows x64 `GetCursorPos` 平台实现
- macOS 与 Windows CI 编译矩阵
- Windows 10/11 x64 单显示器实机基线已通过（100%、150%、200% DPI）
- 拖动期间暂停注视追踪，修复眼神在方向帧之间抖动的问题
- 根据窗口实际移动方向播放向左或向右奔跑动画
- macOS 状态栏显示 March 7th 图标，并提供显示、隐藏和退出菜单
- macOS 状态栏提供“切换到当前桌面”，通过隐藏后重新显示处理 Spaces 切换后的窗口跟随问题（旧版 PR #12）

当前集成候选另已实现双平台托盘显示/隐藏/退出、点击穿透与恢复、位置保存、双角色单击/双击互动及选择保存，以及 Mac 专属“切换到当前桌面”。后者复用同一控制器，延迟显示可被后续隐藏、重置、退出及窗口销毁取消；Mac Spaces 实机结果仍待记录。设置页已接入本地备份导出、预览和确认下次启动导入，真实迁移仍待两平台验收。尚未实现自动启动及安装包签名。

阶段验收记录见 [v0.2.0 阶段文档](../docs/development/app/STAGE-0.2.0.md)。

## 形象预览

![March 7th](../docs/assets/march-7th-front.png)

透明窗口、v2 图集裁切和真实鼠标方向跟随均已在 Apple Silicon macOS 上验证运行。

## 开发环境

- Tauri 2
- Rust stable，最低 1.89（标准库文件锁）；当前依赖以 CI stable 工具链验证
- TypeScript + Vite
- pnpm

```bash
pnpm install
pnpm test
pnpm check
pnpm test:directory
pnpm tauri dev
```

## 构建目标

- macOS Apple Silicon：`aarch64-apple-darwin`
- Windows 10/11 x64：`x86_64-pc-windows-msvc`

不支持 macOS Intel。

## 项目边界

旧版 Codex 内嵌宠物已在仓库标签 `codex-pet-v1.0.0` 中封存。独立应用开发只发生在本目录，不修改旧版资源包。
