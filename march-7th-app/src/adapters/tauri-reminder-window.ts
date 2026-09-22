import { getCurrentWindow } from "@tauri-apps/api/window";
export const hideSettingsWindow = () => getCurrentWindow().hide();
