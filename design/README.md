# Design Assets / 设计资源

## App Icon / 应用图标

- `icons/app-icon-master.png`：当前图标主稿，1024 × 1024。
- `icons/drafts/`：历史设计草稿，仅用于追溯，不参与构建。
- `../march-7th-app/src-tauri/icons/`：由 Tauri 生成并直接用于平台打包的图标文件。

从主稿重新生成桌面图标：

```bash
cd march-7th-app
pnpm tauri icon ../design/icons/app-icon-master.png
```

当前项目只支持 macOS Apple Silicon 与 Windows x64，因此生成的 Android/iOS 图标目录被 Git 忽略。

---

- `icons/app-icon-master.png`: current 1024 × 1024 icon master.
- `icons/drafts/`: historical drafts; they are not used by the build.
- `../march-7th-app/src-tauri/icons/`: generated platform assets consumed by Tauri.

The project currently targets macOS Apple Silicon and Windows x64 only, so generated Android/iOS icon directories are ignored by Git.
