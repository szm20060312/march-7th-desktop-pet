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
    │   ├── PRODUCT-PLAN.md
    │   ├── ARCHITECTURE.md
    │   ├── ARCHITECTURE-VALIDATION.md
    │   ├── IMPLEMENTATION-ARCHITECTURE.md
    │   ├── ROADMAP.md
    │   ├── STAGE-0.2.0.md
    │   ├── TODO.md
    │   └── WINDOWS-HANDOFF.md
    └── codex-pet/
        ├── BASELINE.md
        ├── INTERACTIONS.md
        └── OPTIMIZATION.md
```

## 独立 App

- [已确认的长期产品计划](development/app/PRODUCT-PLAN.md)
- [架构与扩展说明](development/app/ARCHITECTURE.md)
- [本次架构验证记录](development/app/ARCHITECTURE-VALIDATION.md)
- [M0 测试包说明](development/app/regression/README.md)
- [M0 实际构建与产物校验记录](development/app/M0-BUILD-REPORT.md)
- [双角色实现与同提交测试包记录](development/app/G4-BUILD-REPORT.md)
- [提醒后端实现与同提交自动验证记录](development/app/G5-BUILD-REPORT.md)
- [提醒界面与双平台测试包记录](development/app/G6-BUILD-REPORT.md)
- [M5 本地技术候选、证据与验收缺口](development/app/M5-TECHNICAL-CANDIDATE.md)
- [四项数据集与旧版备份迁移](development/app/M5-DATASET-V2.md)
- [当前一件事的范围与隐私](M5-CURRENT-TASK.md)
- [公开分发准备与未签名安装候选](development/app/DISTRIBUTION-READINESS.md)
- [双平台回归清单](development/app/regression/CHECKLIST.md)
- [回归结果模板](development/app/regression/RESULT-TEMPLATE.md)
- [本次架构整理范围](development/app/IMPLEMENTATION-ARCHITECTURE.md)
- [贡献约定](../CONTRIBUTING.md)
- [开发计划](development/app/ROADMAP.md)
- [双平台 TODO](development/app/TODO.md)
- [Windows 开发交接](development/app/WINDOWS-HANDOFF.md)
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
