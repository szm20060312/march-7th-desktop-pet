# 未签名安装候选（内部验证）

此包是同一源码提交的安装技术候选，不是稳定版或公开发布许可。Windows 包为当前用户 NSIS 安装器；Mac 包为 Apple Silicon DMG。两者均未配置生产签名，Mac 未公证。现有可直接运行的回归包在同次 CI 中单独保留。

## 先核对来源

1. 只从项目 **Desktop Regression Builds** 同一次成功运行取 `march-7th-unsigned-candidate-windows-x64-<完整提交>` 与 `march-7th-unsigned-candidate-macos-arm64-<完整提交>`；不要只凭版本 `0.2.0` 配对。
2. 各自解压到新目录。记录 `BUILD-INFO.json` 的 `sourceCommit`、`target`、`buildRunUrl`、安装包及包内程序的 `bytes`/`sha256`。`SHA256SUMS.txt` 校验下载后的文件；来源仍需对照 Actions 运行。
3. 在有 Node.js 的检查机运行 `node march-7th-app/scripts/distribution-candidate.mjs verify-pair <Windows包目录> <Mac包目录> <完整提交>`；脚本复算包文件、文档与依赖清单的哈希，并拒绝不同提交或版本。没有 Node.js 时，可用 `Get-FileHash -Algorithm SHA256`（Windows）或 `shasum -a 256 -c SHA256SUMS.txt`（Mac）逐项核对。

生成脚本在原生构建机中读取实际程序的 `--build-info`，将它与完整提交、版本、target 对照；还从 NSIS 解包或只读挂载 DMG，核对包内程序与本次构建程序。Mac 程序须逐字节相同；Windows NSIS 打包只允许 Tauri 将唯一类型标记 `__TAURI_BUNDLE_TYPE_VAR_UNK` 改为 `__TAURI_BUNDLE_TYPE_VAR_NSS`，其余字节及长度必须一致。Windows 核对 x64 PE，Mac 核对 arm64 Mach-O、可执行权限与 `.app` 标识。`BUILD-INFO.json` 记录实际包内程序的哈希及构建时检查结果；下载后哈希验证本身不重新执行包内检查。`DEPENDENCY-LICENSES.json` 是该原生运行器装入的 npm 依赖及 Cargo 解析图的版本和许可证表达式清单，不能替代许可证义务审核。

## 安装边界

- Windows NSIS `currentUser` 默认安装在当前用户范围。WebView2 采用 `downloadBootstrapper`：如果设备没有可用运行时，新装可能需要联网。安装后核心本地功能与“断网新装成功”须分别实测。
- Mac DMG 包含 `.app`，需拖入 Applications。未公证候选可能被 Gatekeeper 阻止；记录原始提示，不能把允许打开流程或关掉全局保护当成公证通过。
- 不在已有日用数据上做升级/回退/卸载实验。使用隔离系统用户、虚拟机或可恢复快照，先保存旧包与数据备份。旧版可能不支持新数据格式；升级失败时按已验证备份恢复，不能假定安装旧程序会自动回滚数据。
- 默认卸载只检查应用、快捷方式与用户配置各自去向；不执行递归删除用户目录的脚本。当前项目没有自动更新。

按 [CHECKLIST.md](CHECKLIST.md) 做真实设备测试，并分别填写 [RESULT-TEMPLATE.md](RESULT-TEMPLATE.md)。打包成功、静态身份一致不表示安装、启动、卸载、数据恢复或长期常驻通过。公开门槛与素材缺口见包内[分发准备合同](DISTRIBUTION-READINESS.md)。
