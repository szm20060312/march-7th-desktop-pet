import type { CharacterCatalog, CharacterDefinition, OneShotAction, ResponseContext } from "../domain/character";
import { atlasBitmapDimensions } from "../domain/character";
import type { ReminderId } from "../domain/reminder";
import { reminderPrompt } from "../domain/reminder-prompt";
import type { AtlasPreloader, PresentationStatus } from "./ports";

export type SelectedCharacterSnapshot = Readonly<{ characterId: string; revision: number }>;
export interface CharacterRuntime {
  stop(): void;
  respond(context: ResponseContext, phrase?: string, action?: OneShotAction): boolean;
  endReminder(): void;
}

export function createCharacterPresentationController(options: {
  catalog: CharacterCatalog;
  initial: SelectedCharacterSnapshot;
  preloadAtlas: AtlasPreloader;
  createRuntime(character: CharacterDefinition): CharacterRuntime;
  status: PresentationStatus;
  reportError(character: CharacterDefinition | null, error: unknown): void;
}) {
  const { catalog, preloadAtlas, createRuntime, status, reportError } = options;
  let runtime: CharacterRuntime | null = null;
  let activeId: string | null = null;
  let latestRevision = Number.NEGATIVE_INFINITY;
  let request = 0;
  let destroyed = false;
  let requestedId: string | null = null;
  let pendingReminder: { characterId: string; items: readonly ReminderId[]; presentationId: number } | null = null;

  const findCharacter = (id: string) => catalog.characters.find(character => character.id === id);
  const showReminder = (character: CharacterDefinition, items: readonly ReminderId[], presentationId: number) => {
    const phrase = reminderPrompt(character, items, presentationId);
    return phrase ? runtime?.respond("reminderDue", phrase, items.includes("water") ? "offerWater" : undefined) ?? false : false;
  };

  const select = async (snapshot: SelectedCharacterSnapshot): Promise<boolean> => {
    if (destroyed || !Number.isSafeInteger(snapshot.revision) || snapshot.revision < latestRevision) return false;
    latestRevision = snapshot.revision;
    const token = ++request;
    const character = findCharacter(snapshot.characterId);
    if (!character) {
      pendingReminder = null;
      requestedId = null;
      const error = new Error(`unknown character id: ${snapshot.characterId}`);
      reportError(null, error);
      if (!destroyed && token === request) status.setPresentationError("无法显示所选角色，请重试。");
      return false;
    }
    requestedId = character.id;
    if (pendingReminder && pendingReminder.characterId !== requestedId) pendingReminder = null;
    if (activeId === character.id) {
      status.setPresentationError(null);
      return true;
    }

    try {
      const offer = character.waterOffer;
      const [size, offerSize] = await Promise.all([preloadAtlas(character.atlas.src), offer.kind === "mainAtlas" ? Promise.resolve(null) : preloadAtlas(offer.src)]);
      if (destroyed || token !== request || snapshot.revision !== latestRevision) return false;
      const { width: expectedWidth, height: expectedHeight } = atlasBitmapDimensions(character.atlas);
      if (size.width !== expectedWidth || size.height !== expectedHeight) {
        throw new Error(`图集尺寸应为 ${expectedWidth}×${expectedHeight}，实际为 ${size.width}×${size.height}`);
      }
      if (offer.kind !== "mainAtlas" && offerSize && (offerSize.width !== offer.sourceWidth || offerSize.height !== offer.sourceHeight)) {
        throw new Error(`递杯图集尺寸应为 ${offer.sourceWidth}×${offer.sourceHeight}`);
      }
      const previousCharacter = activeId ? findCharacter(activeId) ?? null : null;
      runtime?.stop();
      runtime = null;
      activeId = null;
      try {
        runtime = createRuntime(character);
        activeId = character.id;
      } catch (error) {
        if (previousCharacter) {
          try {
            runtime = createRuntime(previousCharacter);
            activeId = previousCharacter.id;
          } catch (restoreError) {
            reportError(previousCharacter, restoreError);
          }
        }
        throw error;
      }
      status.setPresentationError(null);
      const deferredReminder = pendingReminder;
      pendingReminder = null;
      if (deferredReminder?.characterId === character.id) showReminder(character, deferredReminder.items, deferredReminder.presentationId);
      return true;
    } catch (error) {
      if (destroyed || token !== request || snapshot.revision !== latestRevision) return false;
      reportError(character, error);
      status.setPresentationError(`无法显示${character.displayName}，请重试。`);
      return false;
    }
  };

  void select(options.initial);

  return {
    select,
    respond(context: ResponseContext) { return runtime?.respond(context) ?? false; },
    remind(items: readonly ReminderId[], presentationId: number) {
      const requested = requestedId ? findCharacter(requestedId) : null;
      if (requested?.reminderPresentation?.deferUntilReady && activeId !== requested.id) {
        if (!reminderPrompt(requested, items, presentationId)) return false;
        pendingReminder = { characterId: requested.id, items: [...items], presentationId };
        return true;
      }
      const character = activeId ? findCharacter(activeId) : null;
      return character ? showReminder(character, items, presentationId) : false;
    },
    endReminder() { pendingReminder = null; runtime?.endReminder(); },
    destroy() {
      if (destroyed) return;
      destroyed = true;
      pendingReminder = null;
      requestedId = null;
      request++;
      runtime?.stop();
      runtime = null;
      activeId = null;
    },
  };
}
