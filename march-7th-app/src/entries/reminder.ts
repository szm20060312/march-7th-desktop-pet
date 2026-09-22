import { createDomReminderView } from "../adapters/dom-reminder-view";
import { connectReminders, createReminderReady } from "../adapters/tauri-reminders";
import { createReminderPresentation } from "../application/reminder-presentation";

const view = createDomReminderView(document);
const controller = createReminderPresentation({ render: view.render, command: command => connection.command(command), ready: createReminderReady(), reportError: error => console.error("Reminder readiness failed", error) });
const connection = connectReminders({ select: controller.receive,
  reportError(error) { console.error("Reminder presentation connection failed", error); controller.error(error.recovery === "restart" ? "提醒连接未建立，请重启应用。" : "读取或操作失败，请从托盘重新查看提醒。"); },
});
const unbind = view.bind(controller);
function dispose() { connection.dispose(); controller.dispose(); unbind(); window.removeEventListener("pagehide", dispose); }
window.addEventListener("pagehide", dispose, { once: true });
import.meta.hot?.dispose(dispose);
