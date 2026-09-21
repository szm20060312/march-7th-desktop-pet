import { browserScheduler } from "./adapters/browser-scheduler";
import { createDomPetView } from "./adapters/dom-pet-view";
import { createTauriHost } from "./adapters/tauri-host";
import { startPetRuntime } from "./application/pet-runtime";
import { march7th } from "./characters/march-7th";

function requiredElement(selector: string): HTMLElement {
  const element = document.querySelector<HTMLElement>(selector);
  if (!element) throw new Error(`Missing required element: ${selector}`);
  return element;
}

const stop = startPetRuntime({
  character: march7th,
  host: createTauriHost(),
  view: createDomPetView(requiredElement("#pet-sprite"), requiredElement("#pet-stage"), document.body),
  scheduler: browserScheduler,
  reportError: (operation, error) => console.error(`Pet ${operation} failed`, error),
});

const dispose = () => {
  stop();
  window.removeEventListener("pagehide", dispose);
};
window.addEventListener("pagehide", dispose, { once: true });
import.meta.hot?.dispose(dispose);
