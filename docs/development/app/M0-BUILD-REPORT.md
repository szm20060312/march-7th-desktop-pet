# M0 双平台测试包交付记录

日期：2026-09-22。状态：测试构建与静态产物校验完成，真实桌面验收待测试者回传。M0 未关闭。

## 精确构建基线

- 源提交：`03af006aa52a9e86f585b5c824e38382c3652433`。
- [Desktop Regression Builds 35633690432](https://github.com/szm20060312/march-7th-desktop-pet/actions/runs/35633690432)：Windows x64 与 macOS ARM64 job 均成功。
- 两平台均执行依赖安装、前端检查（35 项应用测试、5 项打包测试、类型／构建／架构检查）、Rust fmt/test/Clippy，再执行原生 release 构建和打包上传。
- 应用版本仍为 0.2.0，测试者必须以包内完整 sourceCommit 对齐，不能只比较版本号。

## 可下载测试包

| 平台 | Artifact | 内容 |
|---|---|---|
| Windows x64 | [下载测试包](https://github.com/szm20060312/march-7th-desktop-pet/actions/runs/35633690432/artifacts/10655328712) | 便携 exe、构建信息、SHA-256、启动退出说明、清单和结果模板 |
| macOS Apple Silicon | [下载测试包](https://github.com/szm20060312/march-7th-desktop-pet/actions/runs/35633690432/artifacts/10655662433) | 内层 .app.zip、构建信息、SHA-256、同一清单和结果模板 |

GitHub artifact 可能需要登录，默认保留 30 天。此记录只指向本次实际产物；过期后需重新生成并记录新的构建运行与提交标识。

## 已执行的下载校验

- 已下载两平台 artifact；两份 BUILD-INFO.json 的提交一致、目标平台正确。
- 每包的五项 SHA256SUMS 校验通过：平台程序／内层 ZIP、三份测试文档及 BUILD-INFO.json。
- manifest 中的文件大小和 SHA-256 与下载内容一致。
- Windows 文件头确认是 PE x64；Mac 内层 ZIP 包含 Mach-O ARM64 可执行文件，执行权限已保留。
- 以上均为静态产物检查，没有在用户电脑或 Mac 上启动应用，不替代 GUI 或性能验收。

## 构建问题与修复

前两轮 Windows 已编译成功，但构建后 Cargo.toml 被标记为修改；日志中的 diff 无内容差异并提示 LF/CRLF 转换。增加仅针对 Cargo.toml 的 LF 检出规则后，两平台完整打包通过。未修改 Cargo.toml 内容，也未移除脏工作区拒绝打包的溯源保护。

本地 Git 未复现 runner 的状态误报；修复有效性由上述真实 Windows 构建验证。不要把局部换行规则扩大成对所有修改的忽略。

## 尚待完成

- 所有者的 Windows 基础交互、DPI、常驻和性能记录。
- 协作者的 Mac 同提交回归与性能记录。
- 多显示器／混合 DPI 的实测或明确设备缺口。
- 开发热更新清理需开发环境另测，普通包不覆盖。

填写 [共用清单](regression/CHECKLIST.md) 与 [结果模板](regression/RESULT-TEMPLATE.md)。只有回收真实结果并处理问题后才能判断 M0 是否关闭。
