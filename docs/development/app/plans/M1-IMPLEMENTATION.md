# M1 implementation: native desktop controls

Status: implementation branch preparation. M0 GUI acceptance is still pending. The owner has asked to continue through M5-A/B/C and finish independently actionable work. This branch may prepare implementation, but cannot close milestones or publish releases without their evidence.

## Global Constraints

- Preserve idle, gaze, movement and the archived Codex package.
- Rust owns native desktop controls. Frontend only retains role presentation.
- No reminders, new character integration, persistence, click-through, auto-start or settings UI in Task 1.
- Never label code completion or CI as GUI acceptance.
- No global toolchain installs, force pushes, merges or releases. Report verification gaps.

## Task 1: Native tray and reliable desktop controls

Deliver G1's independently implementable code on this branch. Read this brief as the complete task scope.

1. Add a focused Rust desktop module under src-tauri/src/desktop/; keep lib.rs as wiring. Enable the Tauri tray-icon feature. Use the configured default window icon.
2. Provide one Chinese tray menu with stable command IDs: show / 显示, hide / 隐藏, reset-position / 重置位置, quit / 退出. Native menu handling must remain available when the WebView is hidden.
3. Show/hide should not forcibly focus another window. Reset moves the pet to the center of the primary monitor, falling back to the current monitor if needed, and shows the pet. Use physical monitor origin/size and actual window outer size; handle negative coordinates and smaller-than-window screens without integer overflow. No persistence in this task.
4. Closing the pet window hides it only after the tray has initialized successfully. Quit from the tray exits the app. Do not add an ExitRequested veto that prevents system or menu quitting.
5. If tray creation fails, fail startup visibly through the existing error path rather than hiding the only window. On menu operation failure report the operation and error through stderr; do not silently pretend success.
6. Keep native IO separate from a small pure geometry function. Add Rust unit tests for negative-origin monitors, nonzero origins, oversized windows and normal centering. Add behavior tests for the command mapping only if they catch meaningful errors, not tests copying a list.
7. Update a short G1 implementation note and TODO to distinguish implemented vs unverified; no M0 or M1 closure.
8. Read Tauri's official current API/source as needed; build against the locked dependency version. Do not hand-edit Cargo.lock. If enabling a feature requires lockfile regeneration, report that need for the controller's isolated toolchain.
9. Run available focused checks, inspect your diff, and commit on the given branch with a local command-scoped identity if none is configured. Do not push. No subagents. Report actual commands/results and all unavailable checks.

Acceptance: code compiles and unit tests pass on both target platforms before technical approval; Windows/Mac GUI acceptance remains a later owner/collaborator action. Do not run installer commands or interact with the user's desktop.

## Task 2: Recoverable click-through and persisted placement

Deliver the independently implementable G2 code on this branch. G1 source at 76f9499 has passed both platform CI and native builds; its GUI acceptance and M0 remain pending. No milestone closure.

Global constraints for this task: preserve all G1 controls, non-activating pet display, existing animations and cursor API. Rust owns native state and persistence. Do not add reminders, character integration, auto-start, public release or a settings window.

1. Extend the native tray with two checked mode items: interaction / 交互模式 and click-through / 点击穿透. They are mutually exclusive. Every launch starts visible and in interaction mode; do not persist hidden/click-through state. A successful host set_ignore_cursor_events call precedes changing the state/menu indication. On failure retain the last effective mode and report an operation-specific error. Tray always remains the recovery path, independent of WebView events.
2. Save only placement in version-1 desktop configuration at app_config_dir()/desktop-state.json. Rust provides the sole writer for this file. Use serde/serde_json with typed configuration; update Cargo.lock via cargo only if needed. No frontend file access. Schema stores monitor name (optional), monitor-relative logical x/y offsets and version. Source and destination scale conversion belongs in a pure geometry module using f64/i64 checked operations, never unchecked narrowing.
3. On save, derive logical offsets from physical window position minus that monitor's physical work-area origin, divided by its scale. On restore, match a unique saved monitor name; otherwise use primary then first available monitor. Convert offsets with the current scale and clamp the entire actual outer window rectangle to that monitor's work area. If the window is larger than the work area, anchor to work-area origin on the oversized axis. Invalid/nonfinite/out-of-range data falls back to a visible centered placement; never leave a window unreachable. Tauri 2.11.5 Monitor::work_area() is available in the locked sources.
4. Startup restoration must complete before the pet is exposed at its restored position. Show revalidates visibility; Reset uses the primary/current work-area center, shows the pet, and schedules/saves the new placement. Reapply geometry after scale changes. A low-frequency native monitor check (every 2 seconds) compares topology/work areas and corrects an unreachable window when monitors disconnect; no other app contents/titles are inspected.
5. Debounce movement writes by 500 ms with one reschedulable pending save, not an OS thread per move. Flush latest placement on explicit tray Quit and orderly app exit, cancel the pending task, and stop the monitor check. Avoid holding locks while invoking native window operations or awaiting; avoid synchronous waits between the UI thread and a background task. Late callbacks after disposal must do nothing. If using Tauri async runtime timers, add the minimum direct tokio time feature as needed rather than relying on undeclared transitive imports.
6. Persistence must not partially overwrite a good file. Write a same-directory temporary file then atomically replace; retain the previous valid configuration as a backup before replacement. Test replacement with an existing destination on Windows. A missing file is normal. Corrupt JSON or an unsupported version uses safe defaults for this session, preserves the original, and disables automatic persistence so a future-version file is not silently downgraded. A failed write retains the old file and reports failure, never pretends saved.
7. Expose persistence availability with one disabled tray status item: saved / pending / unavailable text in Chinese. Log detailed errors to stderr without dumping user configuration. A transient write failure may recover on a later legitimate save; corrupt/unsupported configuration stays read-only until repaired outside this task. No modal dialogs or notification spam.
8. Suggested focused file boundaries: desktop/mod.rs wiring and native effects, desktop/geometry.rs pure placement math, desktop/state.rs modes/lifecycle scheduling, desktop/store.rs versioned IO. Do not introduce an event bus, generic plugin framework, or empty future modules. A different small split is allowed only if reported with rationale.
9. Tests must cover negative monitor origins, mixed scales, removed/missing/duplicate monitor identity, oversized windows, nonfinite/overflow configuration, mode-host failure (through a narrow seam, not duplicated assertions), debounced save/cancellation behavior, missing/corrupt/unknown-version config, valid replacement and failed-write preservation. Use unique isolated temp directories for filesystem tests; no actual user config. Existing G1 focus configuration regression remains cross-platform-safe with the single app_context macro.
10. Update G2 implementation notes, architecture/TODO accurately, preserving M0/GUI gaps. Run focused tests while developing, then all affected Rust checks and frontend regression once; commit without pushing. No subagents or global toolchain changes. Report exact commands/results, test-first evidence where applicable and remaining GUI validation.

Acceptance: automated checks and independent review before technical approval; both-platform CI and real tests for tray mode recovery, persistence, topology and DPI before closing M1. This task must not claim those human observations.
