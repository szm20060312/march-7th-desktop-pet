# 三月七 Codex 桌宠 v1.0.0

这是一个可安装到 Codex/ChatGPT 桌面应用中的自定义 v2 动画宠物，包含 9 组基础状态动画和 16 个视线方向。

## 系统要求

- macOS 或 Windows
- 已安装支持自定义 v2 宠物的新版 Codex/ChatGPT 桌面应用
- 宠物图集格式：`spriteVersionNumber: 2`，`1536 × 2288` WebP

如果安装后没有出现“三月七”，请先升级桌面应用并完全退出后重新启动。

## 一键安装

### macOS

1. 解压 ZIP。
2. 打开“终端”。
3. 进入解压后的目录，然后运行：

```bash
chmod +x install-macos.command
./install-macos.command
```

也可以在 Finder 中右键 `install-macos.command`，选择“打开”。

### Windows

1. 解压 ZIP，不要直接在压缩包预览中运行脚本。
2. 右键 `install-windows.ps1`，选择“使用 PowerShell 运行”。
3. 如果执行策略阻止脚本，请使用下面的“手动安装”。

安装脚本只会：

- 校验包内的 `pet.json` 和 `spritesheet.webp`
- 备份已有的同名宠物
- 把两个运行文件复制到 Codex 宠物目录

脚本不会联网，也不会修改 Codex 应用程序本体。

## 手动安装

在宠物目录下创建 `march-7th` 文件夹，然后复制：

```text
pet.json
spritesheet.webp
```

默认位置：

```text
macOS:   ~/.codex/pets/march-7th/
Windows: %USERPROFILE%\.codex\pets\march-7th\
```

如果设置过 `CODEX_HOME`，请改用：

```text
%CODEX_HOME%/pets/march-7th/
```

完成后完全退出并重新打开 Codex/ChatGPT 桌面应用，在宠物或头像选择器中选择“三月七”。

## 更新、备份和卸载

安装脚本如果发现已有版本，会先备份到：

```text
<CODEX_HOME>/pets/_backups/march-7th-<时间戳>/
```

卸载时删除或移走以下目录即可：

```text
<CODEX_HOME>/pets/march-7th/
```

需要回滚时，把 `_backups` 中对应版本的 `pet.json` 和 `spritesheet.webp` 复制回来。

## 文件校验

正式运行文件的 SHA-256：

```text
pet.json         0f61a86adf8051b72600d4b7fad1db70dbf899f5c8e424976df838f2905b6c03
spritesheet.webp 1223cabacfef28dbd40a46387ffff28ecf951460d47bec1defeebbadc2da8622
```

macOS 可以运行：

```bash
shasum -a 256 -c SHA256SUMS.txt
```

Windows 可以运行：

```powershell
Get-FileHash .\pet.json -Algorithm SHA256
Get-FileHash .\spritesheet.webp -Algorithm SHA256
```

## 包含内容

```text
pet.json                  Codex 宠物配置
spritesheet.webp          v2 动画图集
install-macos.command     macOS 安装脚本
install-windows.ps1       Windows 安装脚本
SHA256SUMS.txt            文件校验值
preview/                  动作与方向预览
qa/                       v2 验证和方向检查结果
NOTICE.md                 非官方同人说明
```

## 故障排查

- 看不到宠物：确认应用支持 v2 宠物，然后重启应用。
- 显示成空白或错位：确认两个运行文件位于同一个 `march-7th` 目录中。
- 只有部分动画：确认 `pet.json` 中仍为 `"spriteVersionNumber": 2`。
- Windows 脚本无法运行：改用手动安装，不需要调整系统的全局执行策略。
- 已有同名版本：安装脚本会备份后覆盖；手动安装前请自行复制备份。

## 版本

当前分享版本：`v1.0.0`，冻结于 2026-09-18。后续独立桌宠 App 的开发不会修改这个备份包。

