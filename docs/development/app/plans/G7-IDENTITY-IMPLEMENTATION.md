# G7-A：运行构建身份与测试包核对

状态：执行计划；属于 G7 安装与迁移准备的一个独立技术结果，不代表 M4/G7/M5 已验收。依据 PRODUCT-PLAN.md 的构建可追溯、手工分发与回退要求。用户已授权持续完成可独立执行的部分；本计划不创建正式 Goal。

已核实：测试包 manifest 有完整提交号/目标/哈希，运行中的应用没有对应入口；build.rs 只调用 tauri_build::build。现有设置页可承载可折叠的版本信息，不新建 WebView。G6 源码已审查通过 d014935；双平台 CI 仍在运行，若暴露基础问题先处理基础问题，不伪造上游验收。

## 结果与非目标

用户可在现有设置页查看版本、目标平台、完整源提交和本地修改/来源未知状态；CI 可从实际构建的程序读取同一身份，并拒绝给身份不匹配的程序制作可追溯测试包。无账户、网络服务、遥测、升级器、导入导出、专注/任务功能、签名或发布，不修改应用版本号来代替验收。

## Global Constraints

- 同一 Rust 值同时供只读 IPC 与 `--build-info` 使用。命令行读取必须在 Tauri/window/worker/config 初始化之前结束；正常无参启动行为保持。
- JSON 契约固定为 `{schemaVersion:1,appVersion:string,target:string,sourceCommit:string|null,sourceState:"clean"|"modified"|"unknown"}`。version 来自当前 Cargo package，target 来自编译 TARGET；commit 是该应用所属 Git 仓库的完整40位小写SHA。来源无法可靠读取时如实 unknown，不用环境变量伪造 clean。
- clean/modified 明确指文档定义的应用构建输入范围，不是正式发布、签名或安全认证。生成构建身份发生在构建时，不在程序启动时依赖当前工作目录或 Git。
- 不返回本机路径、用户名、环境变量集合、分支名或日志。完整SHA用文本展示并可选取，未知/本地修改清楚标记。
- 增量重建必须处理源文件修改和 HEAD/ref 改变：不能复用旧提交或错误的 clean 标记。观察实际输入和 Git 控制文件，避免扫描 node_modules/target/dist，也不要用不存在路径强制无穷重建。无 Git 或处于不相关父仓库中的未跟踪导出包不得借用父仓库SHA。
- 提醒草稿、计时、进度、窗口模式和持久化不受版本信息读取影响。版本信息错误只出现在自己的区域。
- 只在隔离工作树内实施。无子代理、push、主线合并、发布、全局安装或真实用户数据测试；协调者负责复审和远端验证。

## Task 1：贯通运行身份、界面与打包验证

### 构建与 Rust

1. 检查实际 build.rs/lib.rs/main.rs 和打包脚本，再添加窄的构建身份生成/运行模块；建议 `src-tauri/build_identity_support.rs`（构建期 Git/观察路径）与 `src-tauri/src/build_info.rs`（运行常量/序列化/只读命令）。不提前建设通用构建框架。
2. build.rs 保留唯一 tauri_build::build 调用，嵌入上述元数据。检查 manifest 是当前 Git 仓库实际跟踪的应用文件，捕获 Git 不可用/失败，限定输出字段。明确应用输入/dirty检测与观察路径的一致性，记录有意不纳入的缓存和非应用文档。HEAD、symbolic ref、packed refs、worktree 路径需要按实际 Git 查询处理，不假定 `.git` 一定是目录。
3. `get_build_info` 返回同一结构；main 对唯一 `--build-info` 参数输出一行 JSON 并正常退出，在该路径不调用 run、app_context 或配置服务。不增加通用调试命令或路径读取命令。
4. 关键回归先红绿：正常仓库、应用修改、无 Git/不相关父仓库、带空格路径/工作树；增量源码变更与仅 HEAD 变化不沿用旧身份。可用临时 Git/Cargo fixture 做必要整合探针，不改用户仓库历史/全局 Git 配置，不读取认证信息。关注真实构建缓存失效，不只测试字符串格式。

### 前端

5. 在现有 settings.html 表单之外加默认折叠的“版本信息”；清楚展示版本、平台、完整提交或未知、本地修改状态。现有保存/关闭仍可达，不把技术信息放入日常提醒气泡。不建新设置中心。
6. 窄的类型/解析/IPC 适配和真实入口装配；`schemaVersion`、枚举、SHA、合理字符串边界有校验。textContent 输出，dispose 后迟到读取没有副作用；读取失败不污染提醒设置的状态/草稿。无需业务控制器或轮询。

### 打包与说明

7. 扩展 scripts/prepare-regression.mjs 与现有纯函数测试：实际构建程序的身份必须与欲写 manifest 的 version/target/commit 一致，且 sourceState=clean，否则拒绝标记分发包。保留当前干净检出/哈希规则。命令行探针只用于明确给定的本次构建程序，设有界超时；不能执行任意导入包或自动寻找未知程序。
8. 更新 desktop-regression.yml 的两个平台步骤，使用本次已构建 Windows exe / Mac .app 内实际可执行文件读取身份，核对后再生成包。不能用再次读取 Git 的结果冒充“程序里嵌入了正确身份”。保留 Mac 打包权限及现有构建路径语义，核实可执行文件位置再写。
9. 更新用户说明、ARCHITECTURE/TODO 和此阶段记录，区分本地自动验证、同提交 CI、CLI 身份读取与 GUI 验收。完整 SHA 不证明素材授权或稳定发布。
10. 跑相关测试迭代，结束时一次相关 Rust/frontend/packaging/full build 检查；提交后因 HEAD 改变再做有目的的增量构建与 `--build-info` 读取，确认最新提交/clean，而不是把提交前 dirty 二进制当作最终产物。实际 Mac CLI 与包核对由双平台 CI 补证，未做实机验收不得关闭 M0/M4。

## 接口与顺序自查

| 生产者→消费者 | 约束 | 自查 |
|---|---|---|
| 构建期→Rust运行结构 | schemaVersion/版本/目标/SHA/sourceState | 唯一值，不由运行时重新查询 Git |
| Rust IPC→设置页 | 同一JSON契约、独立错误区域 | 不改变提醒业务或表单 |
| Rust CLI→打包器 | 真二进制输出与期望身份一致 | CI先构建再读取，再写包manifest |
| Git变化→Cargo缓存 | 输入范围与watch一致 | 专门验证未提交修改和仅HEAD变化 |
| 本任务→上游G6 | 只读身份加必要打包接线 | G6源代码/用户验收结论不被覆盖 |

风险与停止边界：来源查询失败应 unknown，本地仍能正常启动；只有制作可追溯分发包才拒绝不完整身份。身份探针失败不启动桌宠或自动修复用户数据。若基础 G6 CI 失败，记录并优先修复基础，不用身份功能掩盖它。回退仅恢复本任务代码，不迁移用户数据。
## 实施中确认的构建约定

源码审查发现 Tauri 自动读取的平台配置也必须进入身份范围。为避免 Cargo 为可缺省文件递归扫描 target，或因观察缺失文件而重复构建，本项目将 `src-tauri/tauri.windows.conf.json` 与 `src-tauri/tauri.macos.conf.json` 以 `{}` 纳入仓库并设为必需输入；修改、删除失败、恢复和无变化缓存复用均有真实 Cargo 回归。删除任一文件会明确构建失败。这是本项目约定，不代表 Tauri 普遍要求它们存在。

Mac 归档在制作测试包前，使用有界、只读的系统 tar 精确读取程序成员，并与已探测的显式程序逐字节比较；匹配、同长度旧内容及成员缺失均有真实 ZIP 回归。不会执行归档内容，也不建立通用 ZIP 框架。Mac 宿主行为继续由同提交 CI 补证。
