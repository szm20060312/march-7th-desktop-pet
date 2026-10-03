import type { CharacterCatalog, CharacterDefinition, OneShotAction, ResponseContext } from "../domain/character";
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

  const findCharacter = (id: string) => catalog.characters.find(character => character.id === id);

  const select = async (snapshot: SelectedCharacterSnapshot): Promise<boolean> => {
    if (destroyed || !Number.isSafeInteger(snapshot.revision) || snapshot.revision < latestRevision) return false;
    latestRevision = snapshot.revision;
    const token = ++request;
    const character = findCharacter(snapshot.characterId);
    if (!character) {
      const error = new Error(`unknown character id: ${snapshot.characterId}`);
      reportError(null, error);
      if (!destroyed && token === request) status.setPresentationError("无法显示所选角色，请重试。");
      return false;
    }
    if (activeId === character.id) {
      status.setPresentationError(null);
      return true;
    }

    try {
      const [size, offerSize] = await Promise.all([preloadAtlas(character.atlas.src), preloadAtlas(character.waterOffer.src)]);
      if (destroyed || token !== request || snapshot.revision !== latestRevision) return false;
      const expectedWidth = character.atlas.cellWidth * character.atlas.columns;
      const expectedHeight = character.atlas.cellHeight * character.atlas.rows;
      if (size.width !== expectedWidth || size.height !== expectedHeight) {
        throw new Error(`图集尺寸应为 ${expectedWidth}×${expectedHeight}，实际为 ${size.width}×${size.height}`);
      }
      if (offerSize.width !== character.waterOffer.sourceWidth || offerSize.height !== character.waterOffer.sourceHeight) {
        throw new Error(`递杯图集尺寸应为 ${character.waterOffer.sourceWidth}×${character.waterOffer.sourceHeight}`);
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
      const character = activeId ? findCharacter(activeId) : null;
      const phrase = character && reminderPrompt(character, items, presentationId);
      return phrase ? runtime?.respond("reminderDue", phrase, items.includes("water") ? "offerWater" : undefined) ?? false : false;
    },
    endReminder() { runtime?.endReminder(); },
    destroy() {
      if (destroyed) return;
      destroyed = true;
      request++;
      runtime?.stop();
      runtime = null;
      activeId = null;
    },
  };
}
