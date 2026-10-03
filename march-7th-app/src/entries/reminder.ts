import { createDomReminderView } from "../adapters/dom-reminder-view";
import { connectCharacterSelection } from "../adapters/tauri-characters";
import { connectReminders, createReminderReady } from "../adapters/tauri-reminders";
import { createReminderPresentation } from "../application/reminder-presentation";
import { characterCatalog } from "../characters/catalog";

const view = createDomReminderView(document);
let disposed = false;
let releaseCharacterReady = () => {};
const characterReady = new Promise<void>(resolve => { releaseCharacterReady = resolve; });
let characterFallback: ReturnType<typeof setTimeout> | undefined;
let characterSettled = false;
const settleCharacter = () => {
  if (characterSettled) return;
  characterSettled = true;
  if (characterFallback !== undefined) clearTimeout(characterFallback);
  releaseCharacterReady();
};
characterFallback = setTimeout(settleCharacter, 800);
const nativeReady = createReminderReady();
const controller = createReminderPresentation({ render: view.render, command: command => connection.command(command),
  async ready(id) { await characterReady; if (!disposed) await nativeReady(id); },
  reportError: error => console.error("Reminder readiness failed", error) });
const connection = connectReminders({ select: controller.receive,
  reportError(error) {
    console.error("Reminder presentation connection failed", error);
    // Command feedback belongs to the controller's presentation operation.
    if (error.stage !== "command") controller.error(error.recovery === "restart" ? "提醒连接未建立，请重启应用。" : "读取或操作失败，请从托盘重新查看提醒。");
  },
});
const disconnectCharacter = connectCharacterSelection({
  ids: characterCatalog.characters.map(character => character.id),
  select(snapshot) { view.setCharacter(characterCatalog.characters.find(character => character.id === snapshot.characterId) ?? null); settleCharacter(); },
  reportError(error) { console.error("Reminder character selection unavailable", error); settleCharacter(); },
});
const unbind = view.bind(controller);
function dispose() { disposed = true; settleCharacter(); disconnectCharacter(); connection.dispose(); controller.dispose(); unbind(); window.removeEventListener("pagehide", dispose); }
window.addEventListener("pagehide", dispose, { once: true });
import.meta.hot?.dispose(dispose);
