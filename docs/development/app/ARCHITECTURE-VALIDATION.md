# 架构整理验证记录

日期：2026-09-21。代码基线：`windows/v0.2-baseline@a11d0d1`。本文件与本次架构改动一起提交；结果仅适用于该版本。

## 已实际执行

| 检查 | 结果 |
|---|---|
| 切换入口前：原方向测试 + 新入口行为回归 | 2 文件、17 项通过 |
| 切换入口后：全部前端测试 | 6 文件、35 项通过 |
| TypeScript `tsc --noEmit` | 通过 |
| Vite 生产构建 | 通过，18 个模块转换 |
| 依赖边界检查 | 10 个生产模块通过 |
| 边界反向验证 | 临时 domain → adapter 引用被拒绝，清理后通过 |
| `git diff --check` | 通过 |
| 独立只读代码审查 | 无可报告缺陷，复核 35 测试及结构检查通过 |

入口行为回归覆盖：注视帧、六帧待机、左右移动与反向、竖直移动、0.5 px 阈值、160 ms 恢复、采样失败和主鼠标键。

新增模块回归覆盖：多实例隔离、16 方向及角度回绕、不同角色布局/节奏、采样不重叠、停止后迟到 resolve/reject、RAF/计时器/事件清理、拖动失败、IPC 非有限坐标、图集重设后刷新。

## 本地环境限制

Node.js 24.19.0；锁文件中的 Vitest 4.1.11、TypeScript 6.0.3、Vite 8.3.0。依赖包已下载，但本地 pnpm 的额外元数据验证长时间未返回，安装命令已停止，未声称完整安装流程通过；未修改依赖版本或锁文件。实际检查直接运行已安装的对应程序入口：

```sh
node node_modules/.pnpm/vitest@4.1.11_vite@8.3.0/node_modules/vitest/vitest.mjs run
node node_modules/.pnpm/typescript@6.0.3/node_modules/typescript/bin/tsc --noEmit
node node_modules/.pnpm/vite@8.3.0/node_modules/vite/bin/vite.js build
node scripts/check-architecture.mjs
```

正常开发/CI 仍使用 `pnpm install --frozen-lockfile` 与 `pnpm check`，不跳过依赖校验。上面的直接入口只说明本次实际执行的证据，不能把安装阶段未完成改写成通过。

当时本机未检测到可用 Rust 工具链，未在本机运行 Rust fmt/test/Clippy 或 Tauri 原生构建。该本地限制保持原记录；后续远端验证结果单独补充如下。

## 2026-09-22 远端证据补充

- [PR #1](https://github.com/szm20060312/march-7th-desktop-pet/pull/1) 已合并，主分支基线为 `77d12419125382e2d3441175c9a49ab8791e3f28`，与被测架构提交 `da56984e04d872d176222de393f6538c6ad6e0d2` 文件内容一致。
- [CI 运行 35618545447](https://github.com/szm20060312/march-7th-desktop-pet/actions/runs/35618545447) 的 Windows x64 与 macOS ARM64 job 均为 `success`，包含锁文件依赖安装、`pnpm check`、Rust fmt/test/Clippy。
- 这证明远端自动检查通过，不代表已经生成可分享原生测试包，更不代表桌面实测通过。
- 所有者确认尚未测试本次新架构；M0 保持未关闭。后续 GUI 记录必须注明真实测试的提交和产物标识。

## M0 关闭前仍需完成

- [x] 架构提交 da56984 的 Windows 与 macOS CI 均通过（见上述运行）。
- [ ] 同一提交的双平台可运行测试构建、标识、校验文件与共用清单。
- [ ] Windows：透明置顶、16 方向注视、拖动/恢复、100%/150%/200% DPI 回归。
- [ ] Mac：透明置顶、注视、拖动/恢复回归。
- [ ] 多显示器和混合 DPI：有设备则实测，无设备则明确保留缺口。
- [ ] 热更新及退出后没有重复循环或异常；长时间常驻无明显退化。
- [ ] 两台实际设备的 CPU、内存和响应基准。

既有 Windows 单屏报告、旧 Mac 报告、浏览器测试替身及本次单元测试，都不替代上述 GUI 实测。第二角色、提醒和托盘仍未实现。
