# 开发文档

本目录集中保存 March 7th 项目的开发资料。运行源码、构建配置和资源文件不放在这里。

## 目录

```text
docs/
├── README.md
├── assets/
│   ├── march-7th-front.png
│   └── demos/
│       ├── idle.gif
│       ├── running-left.gif
│       └── running-right.gif
└── development/
    ├── app/
    │   ├── ROADMAP.md
    │   ├── STAGE-0.2.0.md
    │   └── TODO.md
    └── codex-pet/
        ├── BASELINE.md
        ├── INTERACTIONS.md
        └── OPTIMIZATION.md
```

## 独立 App

- [开发计划](development/app/ROADMAP.md)
- [双平台 TODO](development/app/TODO.md)
- [v0.2.0 阶段报告](development/app/STAGE-0.2.0.md)
- [App 开发入口](../march-7th-app/README.md)

## Codex 内嵌宠物

- [视觉与动画基线](development/codex-pet/BASELINE.md)
- [交互状态映射](development/codex-pet/INTERACTIONS.md)
- [优化与验收流程](development/codex-pet/OPTIMIZATION.md)

## 非开发文件

- 可分享的 Codex 宠物包位于 `releases/`。
- Tauri 独立应用源码位于 `march-7th-app/`。
- 项目级变更记录位于根目录 `CHANGELOG.md`。
