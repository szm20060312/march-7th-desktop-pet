# March 7th Windows 开发交接文档

更新时间：2026-09-21  
目标平台：Windows 10/11 x64  
当前应用版本：`v0.2.0`  
基线分支：`main`

## 1. 交接目标

在 Windows PC 上完成现有 `v0.2.0` 的功能对齐与实机验收，然后与 macOS Apple Silicon 并行开发 `v0.3.0`。

Windows 第一阶段不要新增托盘、点击穿透或其他 v0.3.0 功能。先确认以下已有能力在 Windows 上与 macOS 行为一致：

- 透明、无边框、始终置顶窗口
- 六帧待机动画
- 真实鼠标驱动的 16 个注视方向
- 向左拖动播放 `running-left`
- 向右拖动播放 `running-right`
- 拖动期间无注视帧抖动
- 停止约 160 ms 后恢复注视

对应任务清单：[TODO.md](TODO.md)

## 2. 仓库与版本

GitHub：<https://github.com/szm20060312/march-7th-desktop-pet>

关键版本：

- `codex-pet-v1.0.0`：原 Codex 内嵌宠物封存
- `app-v0.1.0`：Tauri 初始原型
- `app-v0.1.1`：第一次拖动抖动修复
- `app-v0.2.0`：当前稳定移动基线与 GitHub Release

开始 Windows 工作前执行：

```powershell
git clone https://github.com/szm20060312/march-7th-desktop-pet.git
cd march-7th-desktop-pet
git fetch --all --tags
git switch main
git pull --ff-only
git status
```

预期状态：

```text
On branch main
Your branch is up to date with 'origin/main'.
nothing to commit, working tree clean
```

为 Windows 基线验证创建独立分支：

```powershell
git switch -c windows/v0.2-baseline
```

不要移动、删除或重新创建 `app-v0.2.0` 标签。

## 3. Windows 开发环境

Tauri 2 在 Windows 上需要 Microsoft C++ Build Tools、WebView2 和 Rust MSVC 工具链。官方要求见 [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)。

### 3.1 必需软件

1. Git for Windows
2. Node.js 24 LTS
3. pnpm 11
4. Rust stable MSVC
5. Microsoft Visual Studio 2022 Build Tools
6. Microsoft Edge WebView2 Runtime

安装 Visual Studio Build Tools 时必须选择：

```text
Desktop development with C++
```

Windows 10 1803 及更高版本通常已经带有 WebView2；若 Tauri 启动时报告 WebView2 缺失，再安装 Evergreen Runtime。Tauri 使用 WebView2 渲染 Windows 界面，详情见 [Tauri WebView versions](https://v2.tauri.app/reference/webview-versions/)。

### 3.2 Rust

安装：

```powershell
winget install --id Rustlang.Rustup
```

重新打开 PowerShell，然后确认 MSVC 工具链：

```powershell
rustup default stable-msvc
rustup target add x86_64-pc-windows-msvc
rustc --version
cargo --version
```

不要使用 GNU Rust target；本项目目标是：

```text
x86_64-pc-windows-msvc
```

### 3.3 Node.js 与 pnpm

```powershell
winget install OpenJS.NodeJS.LTS
corepack enable
corepack prepare pnpm@11 --activate
node --version
pnpm --version
```

### 3.4 环境自检

```powershell
git --version
node --version
pnpm --version
rustc --version
cargo --version
rustup show active-toolchain
```

确认输出中包含：

```text
stable-x86_64-pc-windows-msvc
```

## 4. 安装依赖与基线检查

```powershell
cd march-7th-app
pnpm install --frozen-lockfile
pnpm test
pnpm build
cargo test --manifest-path .\src-tauri\Cargo.toml
cargo clippy --manifest-path .\src-tauri\Cargo.toml --target x86_64-pc-windows-msvc -- -D warnings
```

预期：

- TypeScript/Vitest：10 项通过
- Vite 生产构建成功
- Rust 单元测试通过
- Clippy 无 warning

如果基础测试失败，先修复环境或平台编译问题，不要进入 GUI 行为测试。

## 5. 启动开发版本

```powershell
pnpm tauri dev
```

正常行为：

- 出现透明、无边框的 March 7th 窗口
- 窗口位于其他普通窗口上方
- 可以按住角色拖动
- 终端保持运行并显示 Tauri/Vite 日志
- 在终端按 `Ctrl+C` 退出

当前版本还没有托盘菜单和常规退出按钮。如果运行独立 exe 后无法关闭，可以使用任务管理器，或运行：

```powershell
taskkill /IM march-7th-app.exe /F
```

## 6. 当前代码入口

| 文件 | 作用 |
|---|---|
| `march-7th-app/src/main.ts` | 采样鼠标/窗口位置，选择待机、注视或左右移动动画 |
| `march-7th-app/src/animation.ts` | 16 方向映射与水平移动方向判断 |
| `march-7th-app/src/animation.test.ts` | 平台无关动画逻辑测试 |
| `march-7th-app/src-tauri/src/lib.rs` | Tauri 命令，统一返回鼠标相对位置和窗口坐标 |
| `march-7th-app/src-tauri/src/platform/windows.rs` | Win32 `GetCursorPos` 与 DPI 坐标换算 |
| `march-7th-app/src-tauri/src/platform/macos.rs` | macOS CoreGraphics 实现，仅供对照 |
| `march-7th-app/src-tauri/tauri.conf.json` | 窗口、图标、构建与打包配置 |
| `docs/development/app/TODO.md` | 双平台任务和里程碑 Gate |

Windows 当前实现核心：

```rust
#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(point: *mut Point) -> i32;
}
```

`GetCursorPos` 返回屏幕物理坐标；当前代码用 Tauri 的 `scale_factor` 将鼠标与窗口位移换算到前端使用的逻辑坐标。Windows 实机适配的重点就是验证这个换算在不同 DPI 和显示器组合下是否成立。

## 7. Windows v0.2.0 实机验收

### 7.1 透明窗口和图标

- [ ] 背景完全透明，没有黑色、白色或灰色矩形
- [ ] App 图标显示为新的 March 7th 相机图标
- [ ] 窗口无标题栏和系统边框
- [ ] 窗口保持置顶
- [ ] 窗口不会出现在普通任务栏按钮中

### 7.2 注视方向

将鼠标放到角色周围依次验证：

- [ ] 上
- [ ] 右上
- [ ] 右
- [ ] 右下
- [ ] 下
- [ ] 左下
- [ ] 左
- [ ] 左上
- [ ] 鼠标进入角色中心区域时回到待机
- [ ] 鼠标静止时方向不闪烁

### 7.3 拖动移动

- [ ] 向右拖动播放 `running-right`
- [ ] 向左拖动播放 `running-left`
- [ ] 上下拖动沿用最近的水平方向
- [ ] 拖动期间没有注视帧插入
- [ ] 拖动期间眼神不抖动
- [ ] 停止约 160 ms 后恢复鼠标注视

### 7.4 DPI

Windows DPI 官方建议是在运行中修改显示缩放，并把窗口拖到不同 DPI 的显示器之间测试，参见 [Microsoft DPI testing guidance](https://learn.microsoft.com/en-us/windows/apps/develop/win2d/dpi-and-dips#how-to-test-dpi-handling)。

至少验证：

- [ ] 100% 缩放
- [ ] 125% 缩放
- [ ] 150% 缩放
- [ ] 200% 缩放（若设备支持）
- [ ] 修改缩放后重启应用
- [ ] 应用运行时修改缩放

每个缩放比例检查：

- 鼠标方向是否准确
- 拖动方向是否准确
- 窗口尺寸是否约为 240 × 260 逻辑像素
- 图像是否清晰
- 是否出现持续抖动或偏移

### 7.5 多显示器

如果有第二显示器：

- [ ] 主屏到副屏拖动
- [ ] 副屏到主屏拖动
- [ ] 左侧显示器产生负 X 坐标
- [ ] 上方显示器产生负 Y 坐标
- [ ] 两个显示器使用不同缩放比例
- [ ] 跨屏后注视与拖动方向仍正确

Microsoft 建议用不同 DPI 的显示器移动窗口来验证 Per-Monitor DPI 行为，参见 [High DPI desktop development](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows)。

## 8. 常见问题

### 找不到 `link.exe` 或 C++ 工具链

重新打开 Visual Studio Installer，确认安装：

```text
Desktop development with C++
```

然后重启 PowerShell。

### WebView2 缺失

安装 Microsoft Edge WebView2 Evergreen Runtime。不要在当前阶段配置 `fixedRuntime`，它会显著增大安装包。

### 窗口显示黑色矩形

检查：

- `tauri.conf.json` 中 `transparent: true`
- `styles.css` 中 `html`、`body` 背景为 `transparent`
- WebView2 是否最新
- 显卡驱动和硬件加速是否异常

请同时保存截图、控制台日志和 Windows 版本。

### 鼠标方向整体偏移

优先记录：

- Windows 显示缩放比例
- 显示器排列
- 当前显示器是否为主屏
- 窗口是否跨显示器
- 偏移方向和大约像素量

重点检查 `platform/windows.rs` 中物理坐标与 `scale_factor` 的换算；不要在前端单独添加 Windows 特判。

### 拖动时再次出现眼神抖动

记录窗口是否真的在移动，以及 `windowX/windowY` 是否连续变化。拖动动画由窗口位置变化触发，不应该依赖 PointerEvent 的结束时序。

### 图标仍显示旧版本

Windows 可能缓存快捷方式或任务栏图标。先确认构建产物中使用的是当前 `icon.ico`，再删除旧快捷方式并重启 Explorer；不要因此重复生成图标文件。

## 9. Windows 构建产物

开发可执行文件：

```powershell
pnpm tauri build -- --no-bundle
```

NSIS 安装包：

```powershell
pnpm tauri build -- --bundles nsis
```

Tauri 官方建议直接在 Windows 上构建 Windows 安装器；NSIS 输出位于 `src-tauri\target\release\bundle\nsis\`。详细说明见 [Tauri Windows Installer](https://v2.tauri.app/distribute/windows-installer/)。

当前阶段不要优先构建 MSI。MSI 依赖 WiX，并可能需要启用 Windows 可选组件 VBSCRIPT；先用 NSIS 完成开发和验收。

## 10. Git 工作流程

开始前：

```powershell
git switch windows/v0.2-baseline
git status
```

保持提交范围小：

- 环境配置问题不要和坐标逻辑修改放在同一提交。
- Windows 专属代码只改 `platform/windows.rs` 或平台配置。
- 共享坐标或状态逻辑需要同时确认 macOS 不退化。
- 不提交 `node_modules`、`target`、`dist`、`.pnpm-store`、`.env` 或本机 IDE 配置。

完成一个可验证修改后：

```powershell
git add <具体文件>
git commit -m "validate Windows cursor scaling"
git push -u origin windows/v0.2-baseline
```

不要直接在 Windows PC 上重新创建或强制移动版本标签。

## 11. 问题回传模板

发现问题时记录：

```markdown
### 环境
- Windows 版本：
- CPU 架构：x64
- 显卡：
- 显示器数量：
- 每个显示器分辨率与缩放：
- Node / pnpm / rustc 版本：

### 操作步骤
1.
2.
3.

### 预期行为

### 实际行为

### 复现频率
- 必现 / 偶发 / 一次

### 附件
- 截图或录屏：
- 终端日志：
- 涉及提交：
```

## 12. Windows 基线完成条件

只有全部满足才关闭 P0 Gate：

- [ ] `pnpm test`、`pnpm build`、Rust test 和 Clippy 全部通过
- [ ] 透明窗口与图标通过
- [ ] 八个主要注视方向通过
- [ ] 左右拖动和抖动修复通过
- [ ] 100% 与 150% DPI 通过
- [ ] 至少完成一次多显示器测试；若无第二显示器需明确记录
- [ ] 所有 Windows 专属问题已有 issue、修复或已接受说明
- [ ] `TODO.md` 更新
- [ ] 新增 Windows 基线阶段报告
- [ ] 分支推送到 GitHub，并通过双平台 CI

完成后再开始 `v0.3.0` 的托盘、可靠退出和点击穿透开发。
