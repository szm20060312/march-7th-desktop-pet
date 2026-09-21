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
