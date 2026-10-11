import type { PetView } from "../application/ports";
import type { CharacterDefinition } from "../domain/character";
import type { SpriteFrame } from "../domain/animation";
import { containsVisiblePoint, type CharacterFrameBounds } from "../domain/pointer-region";
import { loadBrowserFrameBounds, type FrameBoundsLoader } from "./browser-frame-bounds";

export function createDomPetView(sprite: HTMLElement, offerSprite: HTMLElement, stage: HTMLElement, message: HTMLElement, body: HTMLElement, loadBounds: FrameBoundsLoader = loadBrowserFrameBounds): PetView {
  let character: CharacterDefinition;
  let currentFrame: SpriteFrame = { row: 0, column: 0 };
  let pointerRegionEnabled = false;
  let frameBounds: CharacterFrameBounds | undefined;
  let boundsVersion = 0;
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
      boundsVersion++;
      pointerRegionEnabled = false;
      frameBounds = undefined;
      character = definition;
      currentFrame = { row: definition.clips.idle.row, column: 0 };
      lastFrame = "";
      lastAsset = "main";
      const { atlas } = character;
      sprite.style.width = `${atlas.cellWidth}px`;
      sprite.style.height = `${atlas.cellHeight}px`;
      sprite.style.backgroundImage = `url("${atlas.src}")`;
      sprite.style.backgroundSize = `${atlas.columns * atlas.cellWidth}px ${atlas.rows * atlas.cellHeight}px`;
      sprite.style.opacity = "1";
      // The main-atlas capability uses one visible layer even across action boundaries.
      sprite.style.transition = character.waterOffer.kind === "mainAtlas" ? "none" : "";
      offerSprite.style.width = `${atlas.cellWidth}px`;
      offerSprite.style.height = `${atlas.cellHeight}px`;
      const offer = character.waterOffer;
      offerSprite.style.display = offer.kind === "mainAtlas" ? "none" : "";
      offerSprite.style.backgroundImage = offer.kind === "mainAtlas" ? "none" : `url("${offer.src}")`;
      offerSprite.style.backgroundSize = offer.kind === "mainAtlas" ? "" : `${offer.columns * atlas.cellWidth}px ${offer.rows * atlas.cellHeight}px`;
      offerSprite.style.backgroundPosition = "0px 0px";
      offerSprite.style.opacity = "0";
      sprite.setAttribute("aria-label", character.displayName);
      stage.setAttribute("aria-label", `${character.displayName} desktop pet`);
    },
    center() {
      const bounds = sprite.getBoundingClientRect();
      return { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 };
    },
    render(frame) {
      currentFrame = frame;
      const asset = frame.asset ?? "main";
      const key = `${asset}:${frame.row}:${frame.column}`;
      if (key === lastFrame) return;
      const { cellWidth, cellHeight } = character.atlas;
      if (asset !== lastAsset) {
        sprite.style.opacity = asset === "waterOffer" ? "0" : "1";
        offerSprite.style.opacity = asset === "waterOffer" ? "1" : "0";
        lastAsset = asset;
      }
      const target = asset === "waterOffer" ? offerSprite : sprite;
      target.style.backgroundPosition = `${-frame.column * cellWidth}px ${-frame.row * cellHeight}px`;
      sprite.dataset.frame = asset === "main" ? `${frame.row}:${frame.column}` : `water:${frame.row}:${frame.column}`;
      lastFrame = key;
    },
    setTracking: status => { body.dataset.cursorTracking = status; },
    showPhrase(text) { phrase = text; updateMessage(); },
    clearPhrase() { phrase = null; updateMessage(); },
    setPresentationError(error) { presentationError = error; updateMessage(); },
    setPointerRegionEnabled(enabled) {
      const version = ++boundsVersion;
      pointerRegionEnabled = enabled && character.interaction?.pointerRegion.kind === "visibleBounds";
      if (!pointerRegionEnabled) { frameBounds = undefined; return; }
      void loadBounds(character).then(bounds => {
        if (version === boundsVersion && pointerRegionEnabled) frameBounds = bounds;
      }).catch(() => {
        // Keep a usable sprite-sized region if native pixels cannot be read.
        if (version === boundsVersion) frameBounds = undefined;
      });
    },
    containsPointer(point) {
      if (!pointerRegionEnabled) return true;
      const asset = currentFrame.asset ?? "main";
      const target = asset === "waterOffer" ? offerSprite : sprite;
      const rect = target.getBoundingClientRect();
      const { cellWidth, cellHeight } = character.atlas;
      if (rect.width <= 0 || rect.height <= 0) return true;
      const columns = asset === "waterOffer" && character.waterOffer.kind !== "mainAtlas" ? character.waterOffer.columns : character.atlas.columns;
      const bounds = frameBounds?.[asset]?.[currentFrame.row * columns + currentFrame.column];
      const paddingPx = character.interaction!.pointerRegion.paddingPx;
      if (bounds === undefined) return point.x >= rect.left - paddingPx && point.x < rect.left + rect.width + paddingPx &&
        point.y >= rect.top - paddingPx && point.y < rect.top + rect.height + paddingPx;
      return containsVisiblePoint(bounds, { x: (point.x - rect.left) * cellWidth / rect.width, y: (point.y - rect.top) * cellHeight / rect.height }, paddingPx * cellWidth / rect.width);
    },
  };
}
