# 开发约定

先读 [产品计划](docs/development/app/PRODUCT-PLAN.md)、[架构说明](docs/development/app/ARCHITECTURE.md) 和 [TODO](docs/development/app/TODO.md)。产品计划是目标，TODO 是阶段状态，阶段报告是已有验证证据，三者不可互相替代。

## 每次修改

- 一个变更围绕一个可验证行为；不要顺带修改封存的 `releases/v1.0.0`。
- 前端动画纯规则留在 domain；前端运行调度留在 application；Tauri/DOM/浏览器 API 留在 adapters。新增路径必须通过结构检查，不通过时先解释依赖，而不是放宽规则。
- 未来提醒状态／计时、桌面控制和持久化由 Rust 管理；前端使用有类型的命令与快照。Rust 是唯一存储写入者，UI 和托盘不能各自维护业务进度。这是待实施约束，不代表这些功能已经存在。
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

共享行为变化需要 Windows 与 Mac 两份人工验收记录。Windows 由所有者测试，Mac 由朋友/协作者测试；同一份清单、同一提交，缺失项明确保留。任何本地工具缺失或 CI 未完成都要如实说明。

不要在工作区提交 node_modules、target、dist、环境文件或机器配置。新功能通过独立分支/PR交付；不要强推默认分支或重建已有发布标签。
