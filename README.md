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

长期方向已确定为**可更换角色的桌面陪伴工具**，先面向自己和朋友，以日常提醒作为第一个实用场景。[PR #27](https://github.com/szm20060312/march-7th-desktop-pet/pull/27)（HEAD `fc7713c`）已合入 `main`，集成双角色切换、本地提醒、单会话专注与“当前专注”、本地备份导出/导入、宠物右键菜单、顶级“设置”入口及卡片式设置页。该提交已通过双平台 CI 与 macOS / Windows 实机验收；**合入 `main` 仅代表主干阶段集成完成，不代表公开发布（`M5-C`）**。

开发前请读：[长期计划](docs/development/app/PRODUCT-PLAN.md) · [架构说明](docs/development/app/ARCHITECTURE.md) · [贡献约定](CONTRIBUTING.md)。

应用已统一配置目录并协调同目录的可写实例：第二个采用新协议的实例会正常结束；目录或锁不可用时保留临时角色选择、关闭且只读的提醒和位置不可保存提示。升级前先退出旧版；保留的 `instance.lock` 不能用来判断实例仍在运行，也不要删除它来解锁。设置页已接入本地备份导出和确认下次启动导入；完整安装/升级/回退验证仍待 P3 实测。详见 [本地目录与实例协调](docs/development/app/G7-DATA-DIRECTORY.md)和[导入入口](docs/development/app/G7-LOCAL-BACKUP-IMPORT.md)。

March 7th 是一个面向 macOS 与 Windows 的独立桌宠应用。项目复用了经过完整 QA 的 Codex v2 动画图集，并逐步将待机、注视、移动和其他角色动作迁移到 Tauri 2。

**v0.2.0** 为早期历史基线；当前 `main` 已包含 G1–G7 与 M1–M5 阶段功能（托盘与宠物右键菜单、点击穿透、位置持久化、双角色互动、本地提醒与专注管理）。实现与验证边界见 [G2 记录](docs/development/app/G2-PLACEMENT.md) 与 [M5 技术候选与缺口](docs/development/app/M5-TECHNICAL-CANDIDATE.md)。

### v0.2.0 历史基线功能

- 透明、无边框、始终置顶的桌宠窗口
- 六帧待机动画
- 根据真实系统鼠标位置切换 16 个注视方向
- 向左拖动时播放 `running-left`
- 向右拖动时播放 `running-right`
- 拖动期间屏蔽注视帧，解决眼神抖动
- 停止移动约 160 ms 后恢复鼠标注视
- 可拖动窗口

### 本地提醒与安静期控制

首次启动喝水、起身活动、休息眼睛全部关闭；从系统托盘或宠物右键菜单的“设置”打开卡片式设置页，查看间隔和活动时段，确认并保存后才启用（连续点击设置页版本号 8 次可开启隐藏的开发者调试控制台）。默认分钟数只是可调整偏好，不是健康建议。设置窗关闭只隐藏窗口，未保存草稿在下次打开时丢弃。

提醒气泡有“完成”“全部稍后”“收起”；收起及约 10 秒无回应只结束这次自动展示，事项仍待处理，可从“提醒 → 查看待处理”恢复，空集合也能查看。从托盘/右键菜单或设置页可操作“暂停提醒／恢复提醒”以及“结束安静期”；隐藏角色或开启角色穿透不会暂停提醒，气泡独立接收鼠标。两角色共用同一套提醒规则，切换角色不重计时。

自动提醒由当前角色先做动作并说一句话，贴在角色旁的透明对白显示角色形象及“完成／稍后”操作。三月七偏活泼亲近，雷电将军采用克制正式的将军口吻；手动查看待处理不会重新触发到期动作。

全部数据保存在本机，无账号、云同步或另一套系统通知；保存失败、只读保护和界面失败会如实提示。关闭设置或气泡不退出应用；正常结束使用托盘/右键菜单“退出”。详见 [G6 记录](docs/development/app/G6-REMINDERS.md) 和 [实机清单](docs/development/app/regression/CHECKLIST.md)。

### 专注与当前专注

设置页与托盘/右键菜单支持开始、暂停、继续或结束一场专注；过程中日常提醒仍保持待处理，但不主动弹出。专注自然完成只在允许时借同一个气泡提示一次，结果仍可从固定入口查看。开始时可选填当前任务名，明确标记完成或放弃；任务结果不自动结束计时，计时到时也不代替你完成任务。旧本地配置和备份有版本迁移，导入预览只显示任务状态，不显示名称。CI 已生成 Windows NSIS 与 Mac DMG 未签名安装候选，正式安装、升级、回退及两周试用仍待后续验证。详见 [M5 技术候选与缺口](docs/development/app/M5-TECHNICAL-CANDIDATE.md)。

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
| macOS Apple Silicon | 已通过 CI 构建及 `fc7713c` 实机验收 |
| Windows 10/11 x64 | 已通过 CI 构建及 `fc7713c` 实机验收（含单/混合 DPI 基线） |

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

是否公开发布尚待试用、素材范围、签名和维护决策；当前只准备未签名测试候选，不承诺 GitHub Release。

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

公开仓库保留参与开发所需的规范、问题与验证依据；团队内部排期和执行流水不作为贡献前置条件。旧 TODO 与 Windows 交接参考仍可从文档索引访问，不代表最新分支状态。

- [开发文档索引](docs/README.md)
- [开发计划](docs/development/app/ROADMAP.md)
- [公开协作与资料边界](docs/COLLABORATION.md)
- [跨平台验证指南](docs/development/app/PLATFORM-VALIDATION.md)
- [v0.2.0 阶段报告](docs/development/app/STAGE-0.2.0.md)
- [动画与视觉基线](docs/development/codex-pet/BASELINE.md)
- [交互状态映射](docs/development/codex-pet/INTERACTIONS.md)
- [优化与验收流程](docs/development/codex-pet/OPTIMIZATION.md)
- [变更记录](CHANGELOG.md)

### 路线图

- [x] 点击穿透、托盘与宠物右键菜单、窗口位置持久化
- [x] 双角色切换、本地提醒、专注计时与“当前专注”主干集成（PR #27）
- [ ] 未签名 NSIS / DMG 安装、升级、回退与两周朋友试用验证（P3）
- [ ] `codex/raiden-frame-rebuild-20261006` 与未回应自动收起重调度策略等后续独立 PR
- [ ] macOS 签名公证、素材范围确认与公开发布决策（M5-C）

### Codex 内嵌版本

最初的 Codex v2 内嵌宠物保存在 `releases/v1.0.0/`，Git 标签为 `codex-pet-v1.0.0`。独立 App 的开发不会覆盖该版本。

---

## English

### Overview

The long-term goal is a switchable-character desktop companion with gentle daily reminders, initially for the owner and friends. [PR #27](https://github.com/szm20060312/march-7th-desktop-pet/pull/27) (`fc7713c`) has been merged into `main`, integrating dual-character switching, local reminders with quiet-period controls, a single focus session with an optional current task, local backup export/import, a pet right-click context menu, and a card-based Settings window. Both CI and macOS/Windows hands-on verification passed for `fc7713c`; merging into `main` is an internal milestone integration and **not** a public release (`M5-C`). See the [product plan](docs/development/app/PRODUCT-PLAN.md), [M5 candidate and evidence gaps](docs/development/app/M5-TECHNICAL-CANDIDATE.md), [architecture](docs/development/app/ARCHITECTURE.md) and [contributing guide](CONTRIBUTING.md).

March 7th is a standalone animated desktop companion for macOS and Windows. It reuses the fully validated Codex v2 sprite atlas and progressively ports idle, gaze, movement, and character interactions to Tauri 2.

**v0.2.0** was the earlier baseline; `main` now includes G1–G7 and M1–M5 milestone capabilities (tray & context menu controls, click-through, placement persistence, dual characters, local reminders, and focus/current-task tracking). See the [G2 record](docs/development/app/G2-PLACEMENT.md) for implementation and evidence boundaries.

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

### Platform Support

| Platform | Status |
|---|---|
| macOS Apple Silicon | CI-built and hands-on verified on `fc7713c` |
| Windows 10/11 x64 | CI-built and hands-on verified on `fc7713c` (including DPI baseline) |

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

Public distribution has not been decided. Unsigned test candidates are being prepared; a GitHub Release requires later trial, asset-rights, signing and maintenance decisions.

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

Public documentation must be sufficient to contribute without access to the team workspace. Dated TODO and Windows handoff references remain available through the documentation index; they do not establish the state of another branch.

- [Documentation index](docs/README.md)
- [Development roadmap](docs/development/app/ROADMAP.md)
- [Public collaboration and documentation boundaries](docs/COLLABORATION.md)
- [Cross-platform validation guide](docs/development/app/PLATFORM-VALIDATION.md)
- [v0.2.0 milestone report](docs/development/app/STAGE-0.2.0.md)
- [Animation and visual baseline](docs/development/codex-pet/BASELINE.md)
- [Interaction state mapping](docs/development/codex-pet/INTERACTIONS.md)
- [Optimization and acceptance workflow](docs/development/codex-pet/OPTIMIZATION.md)
- [Changelog](CHANGELOG.md)

### Roadmap

- [x] Click-through, tray & pet context menus, and window placement persistence
- [x] Dual-character switching, local reminders, focus session, and current-task integration on `main` (PR #27)
- [ ] Unsigned NSIS / DMG clean installation, upgrade, rollback, and a two-week friends trial (P3)
- [ ] Follow-up PRs for `codex/raiden-frame-rebuild-20261006` and pending-reminder reschedule policy
- [ ] Asset-rights, macOS signing/notarization, and maintenance decisions before any public release (`M5-C`)

### Archived Codex Version

The original Codex v2 embedded pet is preserved under `releases/v1.0.0/` and tagged as `codex-pet-v1.0.0`. Development of the standalone app does not overwrite that package.

---

## Disclaimer / 免责声明

This is an unofficial, non-commercial fan project. March 7th and related Honkai: Star Rail character elements belong to their respective rights holders. This project is not affiliated with or endorsed by OpenAI, HoYoverse, or miHoYo.

本项目为非官方、非商业同人作品。“三月七”及《崩坏：星穹铁道》相关角色元素的权利归各自权利人所有。本项目与 OpenAI、HoYoverse 或米哈游不存在官方关联或背书关系。
