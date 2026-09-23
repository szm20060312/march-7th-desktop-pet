import type { PetView } from "../application/ports";
import type { CharacterDefinition } from "../domain/character";

export function createDomPetView(sprite: HTMLElement, stage: HTMLElement, message: HTMLElement, body: HTMLElement): PetView {
  let character: CharacterDefinition;
  let lastFrame = "";
  let lastAsset: "main" | "waterOffer" = "main";
  let phrase: string | null = null;
  let presentationError: string | null = null;
  const updateMessage = () => {
    const text = presentationError ?? phrase;
    message.textContent = text ?? "";
    message.hidden = text === null;
    message.dataset.kind = presentationError ? "error" : "phrase";
  };
  return {
    configure(definition) {
      character = definition;
      lastFrame = "";
      lastAsset = "main";
      const { atlas } = character;
      sprite.style.width = `${atlas.cellWidth}px`;
      sprite.style.height = `${atlas.cellHeight}px`;
      sprite.style.backgroundImage = `url("${atlas.src}")`;
      sprite.style.backgroundSize = `${atlas.columns * atlas.cellWidth}px ${atlas.rows * atlas.cellHeight}px`;
      sprite.setAttribute("aria-label", character.displayName);
      stage.setAttribute("aria-label", `${character.displayName} desktop pet`);
    },
    center() {
      const bounds = sprite.getBoundingClientRect();
      return { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 };
    },
    render(frame) {
      const asset = frame.asset ?? "main";
      const key = `${asset}:${frame.row}:${frame.column}`;
      if (key === lastFrame) return;
      const { cellWidth, cellHeight } = character.atlas;
      if (asset !== lastAsset) {
        sprite.style.backgroundImage = `url("${asset === "waterOffer" ? character.waterOffer.src : character.atlas.src}")`;
        sprite.style.backgroundSize = asset === "waterOffer"
          ? `${character.waterOffer.columns * cellWidth}px ${character.waterOffer.rows * cellHeight}px`
          : `${character.atlas.columns * cellWidth}px ${character.atlas.rows * cellHeight}px`;
        lastAsset = asset;
      }
      sprite.style.backgroundPosition = `${-frame.column * cellWidth}px ${-frame.row * cellHeight}px`;
      sprite.dataset.frame = asset === "main" ? `${frame.row}:${frame.column}` : `water:${frame.row}:${frame.column}`;
      lastFrame = key;
    },
    setTracking: status => { body.dataset.cursorTracking = status; },
    showPhrase(text) { phrase = text; updateMessage(); },
    clearPhrase() { phrase = null; updateMessage(); },
    setPresentationError(error) { presentationError = error; updateMessage(); },
  };
}
