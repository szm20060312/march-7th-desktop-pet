# March 7th

March 7th 是从现有 Codex v2 桌宠迁移而来的独立 Tauri 2 桌面应用。

## 当前里程碑：0.1.0

- 透明、无边框、始终置顶的宠物窗口
- 复用已封存的 `1536 × 2288` v2 精灵图
- 六帧待机循环
- 根据真实系统鼠标位置切换 16 个视线方向
- macOS Apple Silicon 原生光标读取
- Windows x64 `GetCursorPos` 平台实现
- macOS 与 Windows CI 编译矩阵

尚未实现：点击穿透、托盘菜单、自动启动、设置界面、位置持久化和安装包签名。

## 形象预览

![March 7th](docs/march-7th-front.png)

透明窗口、v2 图集裁切和真实鼠标方向跟随均已在 Apple Silicon macOS 上验证运行。

## 开发环境

- Tauri 2
- Rust stable
- TypeScript + Vite
- pnpm

```bash
pnpm install
pnpm test
pnpm tauri dev
```

## 构建目标

- macOS Apple Silicon：`aarch64-apple-darwin`
- Windows 10/11 x64：`x86_64-pc-windows-msvc`

不支持 macOS Intel。

## 项目边界

旧版 Codex 内嵌宠物已在仓库标签 `codex-pet-v1.0.0` 中封存。独立应用开发只发生在本目录，不修改旧版资源包。
