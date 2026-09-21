<p align="center">
  <img src="docs/assets/march-7th-front.png" width="218" alt="March 7th" />
</p>

<h1 align="center">March 7th</h1>

<p align="center">
  A lightweight animated desktop companion built with Tauri 2.<br />
  基于 Tauri 2 的轻量级三月七动态桌宠。
</p>

<p align="center">
  <a href="#中文">中文</a> · <a href="#english">English</a>
</p>

---

## 中文

### 项目简介

March 7th 是一个面向 macOS 与 Windows 的独立桌宠应用。项目复用了经过完整 QA 的 Codex v2 动画图集，并逐步将待机、注视、移动和其他角色动作迁移到 Tauri 2。

当前稳定版本为 **v0.2.0**。

### 当前功能

- 透明、无边框、始终置顶的桌宠窗口
- 六帧待机动画
- 根据真实系统鼠标位置切换 16 个注视方向
- 向左拖动时播放 `running-left`
- 向右拖动时播放 `running-right`
- 拖动期间屏蔽注视帧，解决眼神抖动
- 停止移动约 160 ms 后恢复鼠标注视
- 可拖动窗口

### 动画预览

<table>
  <tr>
    <th>待机</th>
    <th>向左移动</th>
    <th>向右移动</th>
  </tr>
  <tr>
    <td><img src="docs/assets/demos/idle.gif" width="192" alt="Idle animation" /></td>
    <td><img src="docs/assets/demos/running-left.gif" width="192" alt="Running left animation" /></td>
    <td><img src="docs/assets/demos/running-right.gif" width="192" alt="Running right animation" /></td>
  </tr>
</table>

### 平台支持

| 平台 | 状态 |
|---|---|
| macOS Apple Silicon | 已开发并实机验证 |
| macOS Intel | 不支持 |
| Windows 10/11 x64 | 平台代码与 CI 已建立，等待 Windows 实机验证 |

### 从源码运行

依赖：Node.js 24、pnpm 11、Rust stable，以及对应平台的 Tauri 2 系统依赖。

```bash
cd march-7th-app
pnpm install
pnpm test
pnpm tauri dev
```

构建 Apple Silicon macOS 应用：

```bash
pnpm tauri build --target aarch64-apple-darwin --bundles app
```

预构建安装包将在 GitHub Releases 中发布。

### 项目结构

```text
.
├── march-7th-app/          # Tauri 2 独立应用源码
├── docs/                   # 文档、阶段报告和演示资源
├── design/                 # 图标主稿与设计草稿
├── releases/v1.0.0/       # 原 Codex 内嵌宠物封存包
├── .github/workflows/      # macOS ARM64 与 Windows x64 CI
├── CHANGELOG.md
└── README.md
```

### 文档

- [开发文档索引](docs/README.md)
- [开发计划](docs/development/app/ROADMAP.md)
- [v0.2.0 阶段报告](docs/development/app/STAGE-0.2.0.md)
- [动画与视觉基线](docs/development/codex-pet/BASELINE.md)
- [交互状态映射](docs/development/codex-pet/INTERACTIONS.md)
- [优化与验收流程](docs/development/codex-pet/OPTIMIZATION.md)
- [变更记录](CHANGELOG.md)

### 路线图

- 点击穿透与交互模式切换
- 单击挥手、双击跳跃
- 托盘菜单与退出入口
- 窗口位置持久化
- Windows 实机、混合 DPI 和安装包验证
- macOS 签名与公证

### Codex 内嵌版本

最初的 Codex v2 内嵌宠物保存在 `releases/v1.0.0/`，Git 标签为 `codex-pet-v1.0.0`。独立 App 的开发不会覆盖该版本。

---

## English

### Overview

March 7th is a standalone animated desktop companion for macOS and Windows. It reuses the fully validated Codex v2 sprite atlas and progressively ports idle, gaze, movement, and character interactions to Tauri 2.

The current stable version is **v0.2.0**.

### Features

- Transparent, frameless, always-on-top pet window
- Six-frame idle animation
- Sixteen gaze directions driven by the real system cursor
- `running-left` while dragging the window left
- `running-right` while dragging the window right
- Gaze suppression during window movement to prevent eye jitter
- Gaze resumes about 160 ms after the window settles
- Draggable desktop window

### Animation Preview

<table>
  <tr>
    <th>Idle</th>
    <th>Move left</th>
    <th>Move right</th>
  </tr>
  <tr>
    <td><img src="docs/assets/demos/idle.gif" width="192" alt="Idle animation" /></td>
    <td><img src="docs/assets/demos/running-left.gif" width="192" alt="Running left animation" /></td>
    <td><img src="docs/assets/demos/running-right.gif" width="192" alt="Running right animation" /></td>
  </tr>
</table>

### Platform Support

| Platform | Status |
|---|---|
| macOS Apple Silicon | Implemented and tested on real hardware |
| macOS Intel | Not supported |
| Windows 10/11 x64 | Platform implementation and CI are ready; real-device validation is pending |

### Run from Source

Requirements: Node.js 24, pnpm 11, Rust stable, and the Tauri 2 system prerequisites for your platform.

```bash
cd march-7th-app
pnpm install
pnpm test
pnpm tauri dev
```

Build for Apple Silicon macOS:

```bash
pnpm tauri build --target aarch64-apple-darwin --bundles app
```

Prebuilt packages will be published through GitHub Releases.

### Repository Layout

```text
.
├── march-7th-app/          # Tauri 2 application source
├── docs/                   # Documentation, milestone reports, and demos
├── design/                 # Icon master artwork and design drafts
├── releases/v1.0.0/       # Archived Codex embedded-pet package
├── .github/workflows/      # macOS ARM64 and Windows x64 CI
├── CHANGELOG.md
└── README.md
```

### Documentation

- [Documentation index](docs/README.md)
- [Development roadmap](docs/development/app/ROADMAP.md)
- [v0.2.0 milestone report](docs/development/app/STAGE-0.2.0.md)
- [Animation and visual baseline](docs/development/codex-pet/BASELINE.md)
- [Interaction state mapping](docs/development/codex-pet/INTERACTIONS.md)
- [Optimization and acceptance workflow](docs/development/codex-pet/OPTIMIZATION.md)
- [Changelog](CHANGELOG.md)

### Roadmap

- Click-through and interaction-mode switching
- Single-click wave and double-click jump
- Tray menu and a normal quit action
- Persistent window position
- Windows hardware, mixed-DPI, and installer validation
- macOS signing and notarization

### Archived Codex Version

The original Codex v2 embedded pet is preserved under `releases/v1.0.0/` and tagged as `codex-pet-v1.0.0`. Development of the standalone app does not overwrite that package.

---

## Disclaimer / 免责声明

This is an unofficial, non-commercial fan project. March 7th and related Honkai: Star Rail character elements belong to their respective rights holders. This project is not affiliated with or endorsed by OpenAI, HoYoverse, or miHoYo.

本项目为非官方、非商业同人作品。“三月七”及《崩坏：星穹铁道》相关角色元素的权利归各自权利人所有。本项目与 OpenAI、HoYoverse 或米哈游不存在官方关联或背书关系。
