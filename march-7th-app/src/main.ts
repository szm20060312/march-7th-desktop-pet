import { browserScheduler } from "./adapters/browser-scheduler";
import { preloadBrowserAtlas } from "./adapters/browser-atlas-preloader";
import { createDomPetGestures } from "./adapters/dom-pet-gestures";
import { createDomPetView } from "./adapters/dom-pet-view";
import { connectCharacterSelection } from "./adapters/tauri-characters";
import { createTauriHost } from "./adapters/tauri-host";
import { createCharacterPresentationController } from "./application/character-presentation";
import { startPetRuntime } from "./application/pet-runtime";
import { characterCatalog } from "./characters/catalog";

function requiredElement(selector: string): HTMLElement {
  const element = document.querySelector<HTMLElement>(selector);
  if (!element) throw new Error(`Missing required element: ${selector}`);
  return element;
}
const stage = requiredElement("#pet-stage");
const view = createDomPetView(requiredElement("#pet-sprite"), stage, requiredElement("#pet-message"), document.body);
const gestures = createDomPetGestures(stage, document, window, browserScheduler);
const host = createTauriHost();
let errorTimer: number | undefined;
const status = {
  setPresentationError(error: string | null, autoClear = true) {
    if (errorTimer !== undefined) window.clearTimeout(errorTimer);
    view.setPresentationError(error);
    errorTimer = error && autoClear ? window.setTimeout(() => { view.setPresentationError(null); errorTimer = undefined; }, 4000) : undefined;
  },
};
let presentation: ReturnType<typeof createCharacterPresentationController> | undefined;
const disconnect = connectCharacterSelection({
  ids: characterCatalog.characters.map(character => character.id),
  reportError(error) {
    console.error("Character selection connection failed", error);
    if (error.recovery === "restart") {
      status.setPresentationError("角色连接失败，请重启。", false);
    } else {
      status.setPresentationError("无法读取角色选择，请从托盘重试。");
    }
  },
  select(snapshot) {
    if (presentation) { void presentation.select(snapshot); return; }
    presentation = createCharacterPresentationController({
      catalog: characterCatalog,
      initial: snapshot,
      preloadAtlas: preloadBrowserAtlas,
      status,
      reportError: (character, error) => console.error(`Character presentation failed (${character?.id ?? "unknown"})`, error),
      createRuntime: character => startPetRuntime({
        character, host, view, gestures, scheduler: browserScheduler,
        reportError: (operation, error) => console.error(`Pet ${operation} failed`, error),
      }),
    });
  },
});
const dispose = () => {
  disconnect();
  presentation?.destroy();
  if (errorTimer !== undefined) window.clearTimeout(errorTimer);
  window.removeEventListener("pagehide", dispose);
};
window.addEventListener("pagehide", dispose, { once: true });
import.meta.hot?.dispose(dispose);
