# G7-A：运行构建身份

G7 的一个技术切片，版本仍为 0.2.0；不关闭 M0、M4/G7 或 M5。上游 G6 源码基线为 d014935。本地自动检查、同提交双平台 CI、实际 GUI 验收分别记录，不互相代替。

## 使用与契约

托盘打开现有提醒设置，在表单下方展开默认折叠的“版本信息”，可选择并复制完整提交号。读取失败仅显示在该区域，不改变提醒设置、草稿或运行状态。

已知的本次构建程序接受唯一参数 `--build-info`，直接输出一行 JSON 并退出；它在 Tauri context、窗口、worker 和配置服务初始化前返回。无参数启动行为保持。

固定契约为 `{schemaVersion:1,appVersion:string,target:string,sourceCommit:string|null,sourceState:"clean"|"modified"|"unknown"}`。真实提交只接受 40 位小写十六进制 SHA 或 null。版本来自 Cargo package，平台来自编译 TARGET。Rust 的同一只读常量同时用于 CLI 与 `get_build_info` IPC，不在运行时依赖 Git、当前目录、分支或环境。输出不含路径、用户名、分支名或日志。

`clean` / `modified` 只表示下面定义的应用构建输入是否有 Git 记录之外的修改；`unknown` 表示来源无法可靠读取，提交为 null。它们都不是发布、签名、安全或素材授权证明，也不证明运行稳定。

## 构建输入与缓存

构建脚本的 `INPUTS` 同时用于 Git 状态比较与 Cargo 输入观察：

- 前端 `src/`、`public/`、`index.html`、`settings.html`、`reminder.html`。
- `package.json`、`pnpm-lock.yaml`、`tsconfig.json`、`vite.config.ts`。
- Rust `src-tauri/src/`、`icons/`、`capabilities/`、`Cargo.toml`、`Cargo.lock`、`tauri.conf.json`、`build.rs`、`build_identity_support.rs`。
- 本项目将自动合并的 `src-tauri/tauri.windows.conf.json` 与 `tauri.macos.conf.json` 作为必需构建文件跟踪，初始内容都是 `{}`。修改会标记 modified，删除任一文件会明确构建失败，恢复后可继续。这是本项目的约定，并非 Tauri 通用要求；避免为可缺省配置扫描整个含缓存的目录，或依赖缺失文件强制反复重建。
- 应用与 Rust 的 `.gitignore`，以及仓库根的 `.gitignore`、`.gitattributes`（影响来源比较）。

源码目录递归观察，可发现新文件；在此范围内被 Git 忽略的本地文件也视为修改。已存在的明确配置文件逐个观察；不递归观察应用根目录。`node_modules/`、`target/`、`dist/` 等生成物和缓存不在范围内；外部工具链、依赖缓存、构建参数和系统 SDK 不由此 SHA 证明。仓库文档、设计资料、测试辅助文件和打包脚本不作为编译应用输入；`src/` 内的测试仍属于该目录范围。新增构建输入位置时必须同步修改列表与此说明。

Git 必须确认该应用的 `src-tauri/Cargo.toml` 受当前仓库跟踪；没有 Git、查询失败、无提交的仓库或嵌入不相关父仓库的未跟踪导出包均为 unknown。Git 控制路径通过 `rev-parse --git-path` 取得，观察实际 HEAD、index、symbolic ref、packed-refs，以及存在的配置和排除文件，支持 `.git` 文件与 linked worktree。观察现有 ref 目录以捕获 packed ref 转为新 loose ref；不观察整个 Git 对象库。不存在的控制路径不被用作强制重建机制。无 Git 的导出包之后新建仓库时，应重新构建（清理此前 Cargo 缓存或修改已观察输入）；本机制不轮询新出现的仓库。

来源在构建时采样；构建期间应冻结输入，不允许并发修改后再把程序当作该提交产物。正常测试包流程要求干净检出、先构建再探针。

排查 CI 中的 modified 来源时，可显式设置 `MARCH_BUILD_INPUT_DIAGNOSTICS=1`。仅在身份已经判为 modified 时额外只读查询固定范围，按固定公开标签和 tracked/untracked/ignored 类别输出每范围一条去重汇总；查询失败只输出该固定标签与 unavailable。默认关闭，其他值也关闭，开关变化会使 Cargo 重跑身份脚本。诊断不输出实际文件名、目录、Git 原始输出或环境／配置内容，也不改变身份分类和打包拒绝。范围诊断在两平台原生 CI 构建步骤开启，阶段探针见下文。

第二轮探针仅针对 `frontend-source`，使用同一个无外部依赖的 Node 分类器读取 NUL 分隔状态和实际匹配规则。固定来源标签为 root-rules、app-rules、nested-rules、internal-external-rules；固定规则类别为 dependency-directory、build-output、logs、local-config、editor-metadata、other，条目仅分 file/directory/link/other。数量用 zero/one/few/many（0、1、2–9、10 以上），每次最多展示 8 组，超出用 overflow=yes；单行不超过 1024 字符。查询有 5 秒和 2 MiB 边界，超过 512 个项或查询／解析失败会显示 unavailable/unknown，不能当成空范围。未命中的规则只归 other，不输出原始规则。

同一显式开关在安装前、安装后、前端检查后、Cargo 测试后、Clippy 后、identity fixture 后及身份采样时输出阶段摘要。第三轮将 Rust 阶段拆开，以缩小首次出现或首次查询失败的区间；fixture 后即原生构建前，不再重复。三个测试后探针在对应步骤成功或失败后均可运行，但不继续原本失败的构建／打包。身份采样的 Node 直接子进程限时 20 秒，输出最多读取 1025 字节以判断是否超过 1024 字节，stderr 丢弃；清理或输出收尾最多再等 0.5 秒。仅允许固定词表、单行和数量桶通过，失败回退固定 unavailable。只有此诊断开启才额外需要 Node；缺少 Node 不改变身份判断。阶段摘要是时间点证据，不能直接证明 ignored 项不参与应用构建，也不能据此放宽拒包规则。

unavailable 摘要另带固定 `failureStage`：phase、root、status-query、status-format、limit、ignore-query、ignore-format、path、metadata；Rust 无法取得或验证子进程摘要时为 helper。标签只说明失败的操作位置，不含异常原文或敏感输入，也不表示已经确定根因。路径范围校验在忽略规则查询前执行；任何不确定结果仍保持 unknown，成功摘要不添加失败字段。

第四轮仅修复诊断路径解析：status 和 check-ignore 显式使用同一个已验证仓库根，status 使用 repo-relative literal pathspec，NUL 输入／返回保持 repo-relative，不尝试 app-relative 猜测回退。声明范围仍是本 app 的 `src` 目录，包括经验证的根目录自身。候选路径必须拒绝绝对、盘符相对、`.`／`..` 遍历、空组件及相似前缀；规范化后还比较真实范围目录的设备／文件标识，不能仅凭转小写接受大小写敏感文件系统中的另一个目录。链接解析后越出范围、源根本身是链接、无法取得可靠元数据或规则查询失败均保守 unavailable。该修复不扩大应用构建输入、不改变 sourceState 或拒包，并未证明已解释 CI 中实际的 ignored 项。

## 分发包核对

`prepare-regression.mjs` 新增必填的本次构建可执行文件参数。Windows 必须与 exe payload 是同一路径；Mac 必须来自 zip 对应 `.app/Contents/MacOS/`。CI 用 Info.plist 的 CFBundleExecutable 确认实际文件，并保留 `ditto` 权限打包流程。探针有 10 秒超时和 16 KiB 输出上限，失败不生成测试包，不自动寻找程序或运行任意导入包。

Mac 分发前还使用宿主系统的 bsdtar（macOS `/usr/bin/tar`、Windows 系统 `tar.exe`）将 ZIP 中的确切可执行文件成员只读输出到有界管道，与刚刚探针过的程序逐字节比较；10 秒超时，输出上限为程序大小加 1。内容不同、成员缺失或读取失败都会在创建输出目录前拒绝打包，不执行归档内容，也不解压到磁盘。可执行文件名限定为字母、数字、点、下划线和连字符，防止成员选择变成通配模式。

打包器要求实际程序身份的 schema/version/target/commit 与欲写 manifest 完全一致，且 sourceState=clean。仍检查整个仓库的已跟踪修改并计算包内文件 SHA-256；manifest 新增 `binaryBuildInfo` 保存核对过的字段。Mac zip 与其可执行文件必须在同一次受控构建中生成；忘记更新 ZIP 的旧包会因字节不同被拒绝。

## 验证边界

`scripts/build-identity.node-test.mjs` 使用独立临时 Git/Cargo fixture，覆盖正常仓库、源码/HTML/新增文件/暂存修改、只改 HEAD、detached HEAD、含空格工作树、packed refs、无 Git 和不相关父仓库，检验实际增量构建。前端检查校验非法身份、真实入口装配、错误隔离与 dispose 后迟到结果。打包纯函数检查拒绝不匹配身份。

实现提交后的 Windows 增量构建与实际 CLI 探针由任务报告记录。Mac CLI 与对应测试包须由该提交的双平台 CI 补证；设置页真实浏览器检查由协调者记录，原生 GUI、安装迁移与长期使用仍待用户实测。
