import type { CharacterCatalog, CharacterDefinition, ResponseContext } from "../domain/character";
import type { AtlasPreloader, PresentationStatus } from "./ports";

export type SelectedCharacterSnapshot = Readonly<{ characterId: string; revision: number }>;
export interface CharacterRuntime {
  stop(): void;
  respond(context: ResponseContext): boolean;
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
      const size = await preloadAtlas(character.atlas.src);
      if (destroyed || token !== request || snapshot.revision !== latestRevision) return false;
      const expectedWidth = character.atlas.cellWidth * character.atlas.columns;
      const expectedHeight = character.atlas.cellHeight * character.atlas.rows;
      if (size.width !== expectedWidth || size.height !== expectedHeight) {
        throw new Error(`图集尺寸应为 ${expectedWidth}×${expectedHeight}，实际为 ${size.width}×${size.height}`);
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
      status.setPresentationError(`无法显示${character.displayName}，请稍后重试。`);
      return false;
    }
  };

  void select(options.initial);

  return {
    select,
    respond(context: ResponseContext) { return runtime?.respond(context) ?? false; },
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
