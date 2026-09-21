import { browserScheduler } from "./adapters/browser-scheduler";
import { preloadBrowserAtlas } from "./adapters/browser-atlas-preloader";
import { createDomPetGestures } from "./adapters/dom-pet-gestures";
import { createDomPetView } from "./adapters/dom-pet-view";
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

const presentation = createCharacterPresentationController({
  catalog: characterCatalog,
  initial: { characterId: characterCatalog.defaultId, revision: 0 },
  preloadAtlas: preloadBrowserAtlas,
  status: view,
  reportError: (character, error) => console.error(`Character presentation failed (${character?.id ?? "unknown"})`, error),
  createRuntime: character => startPetRuntime({
    character,
    host,
    view,
    gestures,
    scheduler: browserScheduler,
    reportError: (operation, error) => console.error(`Pet ${operation} failed`, error),
  }),
});

const dispose = () => {
  presentation.destroy();
  window.removeEventListener("pagehide", dispose);
};
window.addEventListener("pagehide", dispose, { once: true });
import.meta.hot?.dispose(dispose);
