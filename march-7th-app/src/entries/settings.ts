import { createDomReminderSettings } from "../adapters/dom-reminder-settings";
import { connectReminders } from "../adapters/tauri-reminders";
import { hideSettingsWindow } from "../adapters/tauri-reminder-window";
import { createReminderSettingsController } from "../application/reminder-settings";
import { mountBuildInfo } from "../adapters/dom-build-info";

const view = createDomReminderSettings(document);
const controller = createReminderSettingsController({ render: view.render, command: command => connection.command(command), close: hideSettingsWindow });
const connection = connectReminders({
  select: controller.receive,
  opened() { controller.reopen(); void connection.refresh(); },
  reportError(error) {
    console.error("Reminder settings connection failed", error);
    // Command feedback belongs to the controller's submitting session. The
    // connection may outlive that session when the native window is hidden.
    if (error.stage !== "command") controller.error(error.recovery === "restart" ? "提醒连接未建立，请重启应用。" : "读取或操作失败，请重试；也可从托盘重新打开设置。");
  },
});
const unbind = view.bind(controller);
const disposeBuildInfo = mountBuildInfo(document);
function dispose() { connection.dispose(); controller.dispose(); unbind(); disposeBuildInfo(); window.removeEventListener("pagehide", dispose); }
window.addEventListener("pagehide", dispose, { once: true });
import.meta.hot?.dispose(dispose);
