# March 7th

March 7th 是从现有 Codex v2 桌宠迁移而来的独立 Tauri 2 桌面应用。

开发说明：[长期计划](../docs/development/app/PRODUCT-PLAN.md) · [架构与扩展](../docs/development/app/ARCHITECTURE.md) · [贡献约定](../CONTRIBUTING.md)。

`src/main.ts` 只负责装配；动画状态在 `domain/`，角色参数在 `characters/`，运行调度在 `application/`，Tauri/DOM/浏览器时钟在 `adapters/`。当前仍只有一个内置角色；本次重构没有提前实现提醒或切换界面。

## 当前里程碑：0.2.0

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
- macOS 状态栏提供“切换到当前桌面”，通过隐藏后重新显示解决 Spaces 切换后的窗口跟随问题

尚未实现：Windows 托盘菜单、点击穿透、自动启动、设置界面、位置持久化和安装包签名。

阶段验收记录见 [v0.2.0 阶段文档](../docs/development/app/STAGE-0.2.0.md)。

## 形象预览

![March 7th](../docs/assets/march-7th-front.png)

透明窗口、v2 图集裁切和真实鼠标方向跟随均已在 Apple Silicon macOS 上验证运行。

## 开发环境

- Tauri 2
- Rust stable
- TypeScript + Vite
- pnpm

```bash
pnpm install
pnpm test
pnpm check
pnpm tauri dev
```

## 构建目标

- macOS Apple Silicon：`aarch64-apple-darwin`
- Windows 10/11 x64：`x86_64-pc-windows-msvc`

## 项目边界

旧版 Codex 内嵌宠物已在仓库标签 `codex-pet-v1.0.0` 中封存。独立应用开发只发生在本目录，不修改旧版资源包。
