# M0 / M1 桌面回归测试包

这个包用于验证架构重构与本分支 G1/G2 的真实桌面行为。应用版本号仍为 v0.2.0，但本分支已加入托盘显示/隐藏/重置/退出、交互/点击穿透及位置保存；提醒和角色切换尚未实现。构建成功不代表实机验收通过，M0/M1 仍待验收，必须以 sourceCommit 区分测试基线。

## 先确认版本

1. 从项目 GitHub Actions 的 **Desktop Regression Builds** 同一次成功运行下载对应平台 artifact。GitHub 下载可能要求登录。
2. 解压到一个新的测试目录，不覆盖旧版或旧 Codex 宠物包。
3. 打开 `BUILD-INFO.json`，记录完整 `sourceCommit`、`target` 和 `buildRunUrl`。Windows 与 Mac 必须测试同一 `sourceCommit`；只比较 `0.2.0` 版本号不够。
4. 包内 `validationStatus` 固定为 `build-only-awaiting-human-regression`，不可把它改成测试结果。
5. 按 [CHECKLIST.md](CHECKLIST.md) 测试，将结果填写到 [RESULT-TEMPLATE.md](RESULT-TEMPLATE.md) 的副本。

包包含一个平台程序、三份测试文档、`BUILD-INFO.json` 和 `SHA256SUMS.txt`。SHA-256 用于校验下载内容；从项目对应运行获取文件并核对来源。

## Windows 10/11 x64

- 程序是 `march-7th-app.exe`，不是安装器；无需安装 Node.js、pnpm、Rust 或编译工具。
- 运行前关闭旧的 March 7th 实例，再双击本包 exe。需要 Microsoft Edge WebView2 Runtime；若缺失导致启动失败，记录原始提示及系统版本，按微软官方说明安装运行时后重新测试。
- 正常停止请从系统托盘的 March 7th 菜单选择“退出”，等待进程结束；这会走异步末次位置保存。托盘图标可能在系统的隐藏图标区域。
- 仅在应用无响应时，才在任务管理器“详细信息”中找到**本次启动的** `march-7th-app.exe`，核对进程 ID 后结束该进程。强制结束可能丢失尚未保存的位置，应单独记录，不能作为正常退出或位置持久化验收通过。不要关闭名称不确定的进程。
- 停止程序后即可移走测试目录；这不等同于验证安装包的卸载流程。

校验全部文件（在解压目录，用 PowerShell 执行）：

```powershell
Get-Content -LiteralPath .\SHA256SUMS.txt | ForEach-Object {
    $parts = $_ -split '  ', 2
    $actual = (Get-FileHash -LiteralPath $parts[1] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $parts[0]) { throw "校验失败：$($parts[1])" }
    Write-Host "OK $($parts[1])"
}
```

## macOS Apple Silicon（macOS 13+）

- artifact 解压后还有一份 `March 7th.app.zip`。先校验，再用 Finder 解压该内层 ZIP，得到 `March 7th.app`。内层压缩用于保留应用包的可执行权限。
- 关闭旧实例，再打开本次应用；不支持 Intel Mac，不需要安装开发工具链。
- 这是开发测试构建，没有 Developer ID 签名与公证。如系统阻止启动，先核对来源和校验结果，使用系统提供的针对该应用的允许打开流程；无法允许则记录为“启动受阻”，不要关闭全局系统保护。
- 正常停止请从菜单栏 March 7th 托盘菜单选择“退出”，等待异步末次位置保存并结束进程。
- 仅在应用无响应时使用“活动监视器”强制退出本次 March 7th 进程。强制结束可能丢失尚未保存的位置，应单独记录，不能作为正常退出或位置持久化验收通过。

在测试包目录校验：

```sh
shasum -a 256 -c SHA256SUMS.txt
```

## 测试与反馈

先完成约 10—15 分钟基本交互检查，再按清单记录不同缩放、显示器与常驻表现。没有第二显示器时写“无设备”，不能勾选通过；暂时没时间长测时保留待测。

反馈包括：提交标识、平台/系统、显示器与缩放、具体操作、预期和实际结果。必要时由测试者自愿提供只含桌宠问题区域的截图或录屏；不需要桌面私密内容。

遇到无法启动、明显异常占用或影响正常操作时，先停止测试，记录现象。不要为了“完成验收”把未测项改成通过。

artifact 默认保留 30 天；过期后重新生成并核对代码基线与提交标识。本包不宣称安装器、升级、卸载或 M1 托盘能力已通过。
