import type { CharacterDefinition } from "../domain/character";
import { visiblePixelBounds, type CharacterFrameBounds, type VisibleBounds } from "../domain/pointer-region";

export type FrameBoundsLoader = (character: CharacterDefinition) => Promise<CharacterFrameBounds>;
const cache = new Map<string, Promise<CharacterFrameBounds>>();

async function sheetBounds(src: string, columns: number, rows: number, width: number, height: number) {
  const image = new Image(); image.src = src;
  await image.decode();
  const canvas = document.createElement("canvas");
  canvas.width = width; canvas.height = height;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) throw new Error("Pointer-region canvas unavailable");
  const result: (VisibleBounds | null)[] = [];
  try {
    const sourceWidth = image.naturalWidth / columns;
    const sourceHeight = image.naturalHeight / rows;
    for (let row = 0; row < rows; row++) for (let column = 0; column < columns; column++) {
      context.clearRect(0, 0, width, height);
      context.drawImage(image, column * sourceWidth, row * sourceHeight, sourceWidth, sourceHeight, 0, 0, width, height);
      result.push(visiblePixelBounds(context.getImageData(0, 0, width, height).data, width, height));
    }
    return result;
  } finally {
    // Release the temporary pixels. The cache stores only four numbers per frame.
    canvas.width = 0; canvas.height = 0; image.src = "";
  }
}

export const loadBrowserFrameBounds: FrameBoundsLoader = character => {
  const { atlas, waterOffer } = character;
  const key = `${atlas.src}:${atlas.columns}:${atlas.rows}:${atlas.cellWidth}:${atlas.cellHeight}:${atlas.pixelDensity ?? 1}:${waterOffer.kind === "mainAtlas" ? "main" : waterOffer.src}`;
  let ready = cache.get(key);
  if (!ready) {
    ready = (async () => {
      const main = await sheetBounds(atlas.src, atlas.columns, atlas.rows, atlas.cellWidth, atlas.cellHeight);
      if (waterOffer.kind === "mainAtlas") return { main };
      const offer = await sheetBounds(waterOffer.src, waterOffer.columns, waterOffer.rows, atlas.cellWidth, atlas.cellHeight);
      return { main, waterOffer: offer };
    })();
    cache.set(key, ready);
    void ready.catch(() => { if (cache.get(key) === ready) cache.delete(key); });
  }
  return ready;
};
