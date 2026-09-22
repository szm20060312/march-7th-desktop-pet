# G7-B1：配置目录与可写实例协调

本阶段只统一现有数据目录并协调写入实例。三个业务文件仍为 `desktop-state.json`、`character-preferences.json`、`reminders.json`，位于原 `app_config_dir`；文件内容、schema、备份和保存语义不变。不包含导入导出、活动数据集指针、迁移、唤醒已有窗口或自动重启。

## 启动与退出

`--build-info` 在调用应用 run/build 前输出身份并返回，不解析数据目录、不创建锁。正常启动使用单个 Tauri context：build → 解析一次配置目录 → 建立原目录并非阻塞 try_lock → managed state 持有目录对象 → run → 原有角色、提醒、桌面 setup 顺序。

这是按锁定依赖 Tauri 2.11.5 源码核对的顺序：Builder::build 不执行 setup，App 的运行循环在 Ready 时才执行 setup。因此第二实例可在 run 前直接正常返回；真实构建/setup 错误仍使用原框架错误处理，无需字符串匹配或吞掉其他错误。当前配置没有 build 时自动创建的默认 tray，业务托盘只在 desktop setup 中创建。

`instance.lock` 以 create/read/write、明确不截断方式打开，不写 PID，不删除，成功后只保留一个句柄。WouldBlock 专指已有遵守协议的持锁实例；其他路径/IO 错误进入 Unavailable。managed state 覆盖现有服务停止、迟到回调围栏与末次保存；退出/崩溃后 OS 释放锁。残留文件本身不代表程序存活，不能靠删除文件解锁。

## 降级与边界

目录定位、创建或锁打开/加锁失败时，原生诊断只输出固定失败码。三个服务均取得 None 路径：角色可在会话内切换但标为临时；提醒默认关闭并显示只读；桌面状态显示位置不可保存。没有替代数据目录，不回读或覆盖原配置。单个业务文件的损坏、未来版本及保存失败继续由原 Store 处理。

该文件锁是参与实例的协作协议，不是访问控制。旧版未使用协议，升级前必须先退出旧实例；任意外部工具仍可能修改配置，不应夸大锁的保护范围。

## 自动验证与待验收

Rust 核心测试覆盖平铺路径、首次建目录、竞争、对象释放、残留锁、路径是文件和锁打开失败。两个业务回归验证真实 Service/Store 的 None 路径下临时选择、提醒默认关闭且只读。Node fixture 用 rustc 直接编译生产 std 核心，使用自有临时目录和真实子进程，验证跨进程竞争、第二实例不进入 fixture 服务初始化、正常退出/强制结束后重获、原配置与锁内容保留；另有静态原生接线护栏。运行 `pnpm test:directory`，无需第三方新依赖；最低 Rust 1.89，当前依赖仍由 CI stable 验证。

fixture 已接入 Windows/macOS 两个平台的 App CI 和 Desktop Regression Builds。本地 Windows 自动测试与静态接线不证明真实桌面第二启动、托盘及降级体验，macOS 锁/退出验证须看同提交 CI，双平台 GUI 仍按 [清单](regression/CHECKLIST.md) 验收；未关闭 M0–M4。前端没有改动，已有前端与浏览器证据只在其原范围内复用。

2026-09-22 本地 Windows 检查：Rust 100 项库测试及 6 项目录集成测试通过；目录进程/接线 2 项与打包脚本 10 项通过；fmt、Clippy（`-D warnings`）、debug build、27 个前端模块架构检查通过。实际 debug 程序 `--build-info` 经 stdout 管道读取，退出码 0，隔离临时树未变化；身份为实现前基线 `7583bbc` 加 `modified`，不是最终提交发布包。输出存在既有工具链路径 canonicalization 提示和 MSVC linker stdout warning；未运行 release 构建、GUI 或用户配置试验。

回退只回退本阶段代码与协议，不删除用户配置或锁文件；回退到不采用锁协议的旧版本前，应先退出当前实例。运行中的旧版与新版不能安全并用。
