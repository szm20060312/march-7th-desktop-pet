import { invoke } from "@tauri-apps/api/core";
export const hideSettingsWindow = () => invoke<void>("hide_reminder_settings");
