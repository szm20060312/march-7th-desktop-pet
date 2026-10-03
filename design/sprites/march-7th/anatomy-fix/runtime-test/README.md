# PR #29 本地测试记录

测试日期：2026-10-03。被测代码提交：`78448d914e20d41f021efa53185edae923e0dd8c`。平台：本地 macOS。本记录是代理执行的自动检查和视觉检查，不替代贡献约定中的双平台人工验收。

## 项目检查

| 检查 | 结果 |
| --- | --- |
| `pnpm install --frozen-lockfile` | 通过，未改动锁文件 |
| `pnpm check:architecture` | 通过，10 个生产模块 |
| `pnpm test` | 6 个测试文件、35 项测试通过 |
| `pnpm build` | TypeScript 检查及 Vite 生产构建通过 |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | 通过 |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml` | 1 项 macOS 单元测试通过，二进制和文档测试无测试用例 |
| `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | 通过 |

源图集与生产构建内图集的 SHA-256 均为：

```text
5f0879f921ac9995427281597b916c4e37b89cdb041ba335b43c0f89f5285ce1
```

## 浏览器图集播放

使用临时测试页导入项目现有 `createDomPetView` 和角色定义，图集 URL 指向生产构建的 WebP。测试页直接向渲染器传入挥手、相机动画的行列位置，没有修改应用动作配置，也没有模拟 Tauri IPC。

- 图集 HTTP 200，浏览器成功解码为 1536 × 2288，浏览器计算的 SHA-256 与源图集一致。
- 192 × 208 原始格子尺寸播放。挥手全部 4 帧、相机全部 6 帧均显示；首次暂停时分别记录 95 次、77 次切换。
- 暂停后使用“下一帧”，两组动画切换计数各增加 1，帧位置正确。
- 每次绘制均检查实际 CSS `background-position` 与对应行列一致，没有触发位置不匹配错误。
- 检查四个修正帧在深色和浅色背景下的实际显示，未见第三只手、重复袖口或明显局部接缝。
- 浏览器错误和警告日志查询结果为空。

截图：[深色背景](browser-dark.jpg)、[浅色背景](browser-light.jpg)。浅色截图时浏览器面板较窄，卡片自动换行，不代表图集格子尺寸变化。

## macOS 原生启动

使用当前分支启动 Tauri 开发程序，连接本地 Vite 开发服务器。程序成功启动，系统窗口列表确认 `March 7th` 窗口逻辑尺寸为 240 × 260，截图确认角色实际可见：[原生窗口](native-window.png)。测试后已关闭本次启动的程序和临时服务器。

原生日志出现两条 `Warning: Window move completed without beginning`。观察到窗口仍可正常显示；这不是 Rust/Clippy 失败。本轮未完成系统拖动路径的复现和定位，因此不将其写成拖拽验收通过。

## 覆盖边界

- 当前正式角色配置尚未启用挥手和相机动作，因此这两组动作在浏览器中通过真实 DOM 渲染器检查，未宣称在原生交互中触发成功。
- 未完成托盘操作、系统拖动、多显示器、跨桌面等完整 macOS 人工验收。
- 未进行 Windows 实机操作；此前 GitHub 的 macOS、Windows CI 均通过，CI 不替代 Windows 实机验收。
- 本轮测试没有改动应用代码或图集，不执行 PR 合并。
