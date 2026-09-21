# 开发约定

先读 [产品计划](docs/development/app/PRODUCT-PLAN.md)、[架构说明](docs/development/app/ARCHITECTURE.md) 和 [TODO](docs/development/app/TODO.md)。产品计划是目标，TODO 是阶段状态，阶段报告是已有验证证据，三者不可互相替代。

## 每次修改

- 一个变更围绕一个可验证行为；不要顺带修改封存的 `releases/v1.0.0`。
- 纯规则留在 domain；运行调度留在 application；Tauri/DOM/浏览器 API 留在 adapters。新增路径必须通过结构检查，不通过时先解释依赖，而不是放宽规则。
- 主入口只装配对象。角色资源和表现放在 characters；角色不能拥有提醒进度。
- 计时使用可注入时钟，订阅/定时器必须有清理路径，异步结果必须检查生命周期。
- 先用回归锁住现有行为再重构；新行为覆盖边界和失败场景。测试替身不等于系统实测。
- 说明“改了什么、为什么、怎么验证、哪些未验证”；不要把计划功能写成已实现。

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
