import { afterEach, describe, expect, it, vi } from "vitest";
import { preloadBrowserAtlas } from "./browser-atlas-preloader";

afterEach(() => vi.unstubAllGlobals());

describe("browser atlas preloader", () => {
  it("decodes the image and returns its intrinsic dimensions", async () => {
    const decode = vi.fn().mockResolvedValue(undefined);
    class ImageDouble {
      src = "";
      naturalWidth = 1536;
      naturalHeight = 2288;
      decode = decode;
    }
    vi.stubGlobal("Image", ImageDouble);
    await expect(preloadBrowserAtlas("/assets/test/spritesheet.webp")).resolves.toEqual({ width: 1536, height: 2288 });
    expect(decode).toHaveBeenCalledOnce();
  });
});
