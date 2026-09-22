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

长期方向已确定为**可更换角色的桌面陪伴工具**，先面向自己和朋友，以日常提醒作为第一个实用场景。本开发分支已实现双角色、情境短句和本地提醒；当前提交的双平台实机体验与真实反馈仍待验收。

开发前请读：[长期计划](docs/development/app/PRODUCT-PLAN.md) · [架构说明](docs/development/app/ARCHITECTURE.md) · [贡献约定](CONTRIBUTING.md)。

本分支 G7-B1 已统一配置目录并协调同目录的可写实例：第二个采用新协议的实例会正常结束；目录或锁不可用时保留临时角色选择、关闭且只读的提醒和位置不可保存提示。升级前先退出旧版；保留的 `instance.lock` 不能用来判断实例仍在运行，也不要删除它来解锁。三份配置的文件名与格式不变，导入导出及双平台 GUI 验收尚未完成。详见 [本地目录与实例协调](docs/development/app/G7-DATA-DIRECTORY.md)。

March 7th 是一个面向 macOS 与 Windows 的独立桌宠应用。项目复用了经过完整 QA 的 Codex v2 动画图集，并逐步将待机、注视、移动和其他角色动作迁移到 Tauri 2。

当前稳定版本为 **v0.2.0**。下方功能与平台实机记录属于该历史基线；本分支另已实现 G1/G2 托盘控制、点击穿透和位置持久化，仍待当前提交的实机验收，不能沿用历史通过结论。实现与验证边界见 [G2 记录](docs/development/app/G2-PLACEMENT.md)。

### v0.2.0 历史基线功能

- 透明、无边框、始终置顶的桌宠窗口
- 六帧待机动画
- 根据真实系统鼠标位置切换 16 个注视方向
- 向左拖动时播放 `running-left`
- 向右拖动时播放 `running-right`
- 拖动期间屏蔽注视帧，解决眼神抖动
- 停止移动约 160 ms 后恢复鼠标注视
- 可拖动窗口

### 本分支的本地提醒（实机待验收）

首次启动喝水、起身活动、休息眼睛全部关闭；从系统托盘的“提醒 → 提醒设置…”查看间隔和活动时段，确认并保存后才启用。默认分钟数只是可调整偏好，不是健康建议。设置窗关闭只隐藏窗口，未保存草稿在下次打开时丢弃。

提醒气泡有“完成”“全部稍后”“收起”；收起及约 10 秒无回应只结束这次自动展示，事项仍待处理，可从“提醒 → 查看待处理”恢复，空集合也能查看。从托盘“暂停提醒／恢复提醒”控制自动提示；隐藏角色或开启角色穿透不会暂停提醒，气泡独立接收鼠标。两角色共用同一套提醒规则，切换角色不重计时。

全部数据保存在本机，无账号、云同步或另一套系统通知；保存失败、只读保护和界面失败会如实提示。关闭设置或气泡不退出应用；正常结束使用托盘“退出”。本地代码、自动测试、浏览器模拟、同提交双平台构建和真实桌面体验是不同证据，当前不宣称 M3 或完整日用首版完成。详见 [G6 记录](docs/development/app/G6-REMINDERS.md) 和 [实机清单](docs/development/app/regression/CHECKLIST.md)。

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

### 平台支持（历史基线实机记录）

| 平台 | 状态 |
|---|---|
| macOS Apple Silicon | 已开发并实机验证 |
| macOS Intel | 不支持 |
| Windows 10/11 x64 | 已完成单显示器实机基线验证（100% / 150% / 200% DPI） |

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
- [双平台 TODO](docs/development/app/TODO.md)
- [Windows 开发交接](docs/development/app/WINDOWS-HANDOFF.md)
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

The long-term goal is a switchable-character desktop companion with gentle daily reminders, initially for the owner and friends. This development branch implements two built-in characters, contextual lines and local reminders; current-commit Windows/macOS hands-on acceptance and user feedback remain pending. See the [product plan](docs/development/app/PRODUCT-PLAN.md), [architecture](docs/development/app/ARCHITECTURE.md) and [contributing guide](CONTRIBUTING.md).

March 7th is a standalone animated desktop companion for macOS and Windows. It reuses the fully validated Codex v2 sprite atlas and progressively ports idle, gaze, movement, and character interactions to Tauri 2.

The current stable version is **v0.2.0**. The features and real-device platform results below describe that historical baseline. This branch also implements G1/G2 tray controls, click-through and placement persistence; human acceptance for the current commit is still pending. See the [G2 record](docs/development/app/G2-PLACEMENT.md) for implementation and evidence boundaries.

### v0.2.0 Baseline Features

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

### Platform Support (Historical Baseline)

| Platform | Status |
|---|---|
| macOS Apple Silicon | Implemented and tested on real hardware |
| macOS Intel | Not supported |
| Windows 10/11 x64 | Single-display real-device baseline validated at 100% / 150% / 200% DPI |

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
- [Cross-platform TODO](docs/development/app/TODO.md)
- [Windows development handoff](docs/development/app/WINDOWS-HANDOFF.md)
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
