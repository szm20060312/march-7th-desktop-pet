import type { AtlasPreloader } from "../application/ports";

export const preloadBrowserAtlas: AtlasPreloader = async src => {
  const image = new Image();
  image.src = src;
  await image.decode();
  return { width: image.naturalWidth, height: image.naturalHeight };
};
