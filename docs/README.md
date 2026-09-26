# March 7th 文档索引

这里保存外部贡献者理解、构建、修改和验证项目需要的公开工程资料。内部排期、个人开发流水和临时 Agent 接力过程在团队受限空间维护，不作为公开贡献的前置条件。

## 开始贡献

- [贡献约定](../CONTRIBUTING.md)
- [公开协作与资料边界](COLLABORATION.md)
- [自动化协作者规范](../AGENTS.md)
- [App 开发入口](../march-7th-app/README.md)

## 产品与技术规范

- [产品目标与阶段定义](development/app/PRODUCT-PLAN.md)
- [架构与扩展约束](development/app/ARCHITECTURE.md)
- [路线图](development/app/ROADMAP.md)
- [架构整理范围](development/app/IMPLEMENTATION-ARCHITECTURE.md)

目标不等于已经实现的功能；读取规范时同时核对所处的分支、提交和相关 PR。

## 验证与发布依据

- [跨平台验证指南与记录格式](development/app/PLATFORM-VALIDATION.md)
- [架构验证记录](development/app/ARCHITECTURE-VALIDATION.md)
- [v0.2.0 阶段报告](development/app/STAGE-0.2.0.md)
- [项目变更记录](../CHANGELOG.md)

历史报告只证明其明确列出的提交和测试范围，不替代新提交的回归、真实设备验收或发布授权。

## 带日期的阶段与交接参考

- [阶段 TODO（以文内日期和基线为准）](development/app/TODO.md)
- [Windows v0.2.0 原始交接与基线参考](development/app/WINDOWS-HANDOFF.md)

这两份材料混合了阶段安排和可复用工程知识。本次只调整导航与后续记录规则，原文件暂时完整保留；并未宣称已在飞书归档。后续应逐段分类，保留公开必要知识，核验受限归档后再提交清理变更。新内部流水不再追加到公开参考文档。

## Codex 内嵌宠物的公开规范

- [视觉与动画基线](development/codex-pet/BASELINE.md)
- [交互状态映射](development/codex-pet/INTERACTIONS.md)
- [优化与验收流程](development/codex-pet/OPTIMIZATION.md)

这些文件描述资源和行为约束，不因文件夹名称含 Codex 就自动视为内部会话记录。

## 代码、素材与封存包

- `march-7th-app/`：Tauri 独立应用源码。
- `docs/assets/`：公开文档和演示使用的资源。
- `design/`：设计资料；素材的使用范围需逐项核实，不由记录迁移改变。
- `releases/v1.0.0/`：原 Codex 内嵌宠物封存包，保持不变。
