import type { PetView } from "../application/ports";
import type { CharacterDefinition } from "../domain/character";

export function createDomPetView(sprite: HTMLElement, stage: HTMLElement, message: HTMLElement, body: HTMLElement): PetView {
  let character: CharacterDefinition;
  let lastFrame = "";
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
      const key = `${frame.row}:${frame.column}`;
      if (key === lastFrame) return;
      const { cellWidth, cellHeight } = character.atlas;
      sprite.style.backgroundPosition = `${-frame.column * cellWidth}px ${-frame.row * cellHeight}px`;
      sprite.dataset.frame = key;
      lastFrame = key;
    },
    setTracking: status => { body.dataset.cursorTracking = status; },
    showPhrase(text) { phrase = text; updateMessage(); },
    clearPhrase() { phrase = null; updateMessage(); },
    setPresentationError(error) { presentationError = error; updateMessage(); },
  };
}
