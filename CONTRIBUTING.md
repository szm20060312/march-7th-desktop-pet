# 开发约定

先读 [产品计划](docs/development/app/PRODUCT-PLAN.md)、[架构说明](docs/development/app/ARCHITECTURE.md) 和 [公开协作规则](docs/COLLABORATION.md)。产品计划是目标，[TODO](docs/development/app/TODO.md) 是带日期的阶段记录，阶段报告是特定提交的验证证据，三者不可互相替代；当前工程进展需核对相关 Issue、PR 与提交。

## 每次修改

- 一个变更围绕一个可验证行为；不要顺带修改封存的 `releases/v1.0.0`。
- 前端动画纯规则留在 domain；前端运行调度留在 application；Tauri/DOM/浏览器 API 留在 adapters。新增路径必须通过结构检查，不通过时先解释依赖，而不是放宽规则。
- 提醒状态／计时、桌面控制和持久化遵循由 Rust 管理的职责边界；前端使用有类型的命令与快照。Rust 是唯一存储写入者，UI 和托盘不能各自维护业务进度。职责约定不表示所有能力已经实现，具体功能与验收状态须核对目标提交。
- 主入口只装配对象。角色资源和表现放在 characters；角色不能拥有提醒进度。
- 计时使用可注入时钟，订阅/定时器必须有清理路径，异步结果必须检查生命周期。
- 先用回归锁住现有行为再重构；新行为覆盖边界和失败场景。测试替身不等于系统实测。
- 说明“改了什么、为什么、怎么验证、哪些未验证”；不要把计划功能写成已实现。
- 长期计划决定方向，短期 Goal 每次只完成一个结果。启动前明确范围、前置条件、证据与停止边界；不把全年路线当成一个 Goal。M2/M3 可并行，单角色提醒原型不等于双角色日用版完成。

## 提交前

```sh
cd march-7th-app
pnpm install --frozen-lockfile
pnpm check
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

共享行为变化必须同时保留 Windows 与 macOS 两份人工验收记录，并使用同一份验收清单、同一个提交 SHA。

- macOS 由 [Zheming_Song](https://github.com/szm20060312) 完成人工验收。
- Windows 由 [zthagyamin](https://github.com/zthagyamin) 完成人工验收。
- 两个平台的验收记录必须明确写出测试平台、提交 SHA、测试结果、未覆盖项目和阻塞原因。
- 任一平台记录缺失，不能默认视为通过；缺失项必须明确保留。
- 任何本地工具缺失、实机不可用、CI 未完成或仅完成静态检查，都必须如实说明。
- 共享行为只能在两份记录都对应同一提交并完成必要测试后合并到默认分支。

不要在工作区提交 node_modules、target、dist、环境文件或机器配置。新功能通过独立分支/PR交付；不要强推默认分支或重建已有发布标签。

## 公开贡献与内部记录

- 非敏感缺陷、可参与的工程任务和实质性代码审查留在 GitHub。一个任务围绕一个可复现、可验收的结果，PR 关联相应 Issue（如有）。
- 公开任务和 PR 必须独立写明目标、边界、设计理由、验证证据及未覆盖场景；不得只写“按飞书文档完成”。外部贡献者不需要加入内部飞书才能参与。
- 飞书承载内部排期、分工、原始开发流水、临时提示词与 Agent 接力过程；长期架构约束、通用 Agent 规范及必要测试不随流水迁走。
- 原始日志和人工截图需脱敏后公开。可以在飞书保留受限原件，但公开验收摘要仍需包含平台、提交 SHA、结果、未覆盖项及复现方式。
- 开发完成、PR 合并、目标环境验证和验收通过分别记录。关闭但未合并的 PR 不等于交付；自动检查不代替既有双平台实机门禁。
- 文档导航变更不改变功能状态、素材权利、发布权限或验收结论；不把候选分支写成稳定版本。
- 旧记录应先分类，受限原件在目标空间归档并读回核对、公开必要知识补全、引用修复之后，才能另开清理 PR。移动或删除文件不代表其 Git 历史已被清除。

通用实机记录格式见 [跨平台验证指南](docs/development/app/PLATFORM-VALIDATION.md)，自动化协作者先读 [AGENTS.md](AGENTS.md)。
