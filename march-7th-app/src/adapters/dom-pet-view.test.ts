import { describe, expect, it } from "vitest";
import { march7th } from "../characters/march-7th";
import { createDomPetView } from "./dom-pet-view";

function element() {
  const attributes = new Map<string, string>();
  const style: Record<string, string> = {};
  const dataset: Record<string, string> = {};
  const node = {
    style, dataset, textContent: "", hidden: false,
    getBoundingClientRect: () => ({ left: 24, top: 26, width: 192, height: 208 }),
    setAttribute: (name: string, value: string) => attributes.set(name, value),
  } as unknown as HTMLElement;
  return { node, style, dataset, attributes };
}

describe("DOM pet view", () => {
  it("renders the configured atlas and invalidates its frame cache on reconfiguration", () => {
    const sprite = element();
    const stage = element();
    const message = element();
    const body = element();
    const view = createDomPetView(sprite.node, element().node, stage.node, message.node, body.node);
    view.configure(march7th);
    view.render({ row: 1, column: 2 });
    expect(sprite.style.backgroundSize).toBe("1536px 2288px");
    expect(sprite.style.backgroundPosition).toBe("-384px -208px");
    expect(view.center()).toEqual({ x: 120, y: 130 });
    view.configure({ ...march7th, displayName: "Test", atlas: { ...march7th.atlas, src: "/test.webp", cellWidth: 96 } });
    view.render({ row: 1, column: 2 });
    expect(sprite.style.backgroundImage).toBe('url("/test.webp")');
    expect(sprite.style.backgroundPosition).toBe("-192px -208px");
    expect(sprite.attributes.get("aria-label")).toBe("Test");
  });
  it("keeps response text and presentation errors separate and uses text content", () => {
    const sprite = element();
    const stage = element();
    const message = element();
    const body = element();
    const view = createDomPetView(sprite.node, element().node, stage.node, message.node, body.node);
    view.showPhrase("<b>纯文本</b>");
    expect(message.node.textContent).toBe("<b>纯文本</b>");
    expect(message.node.hidden).toBe(false);
    expect(message.dataset.kind).toBe("phrase");
    view.setPresentationError("图集加载失败");
    view.clearPhrase();
    expect(message.node.textContent).toBe("图集加载失败");
    expect(message.dataset.kind).toBe("error");
    view.setPresentationError(null);
    expect(message.node.hidden).toBe(true);
  });
  it("keeps both sheets layered so the offered cup can crossfade into the original atlas", () => {
    const sprite = element(); const offer = element();
    const view = createDomPetView(sprite.node, offer.node, element().node, element().node, element().node);
    view.configure(march7th);
    view.render({ asset: "waterOffer", row: 1, column: 0 });
    expect(sprite.style.backgroundImage).toBe('url("/assets/march-7th/spritesheet.webp")');
    expect(offer.style.backgroundImage).toBe('url("/assets/march-7th/water-offer.png")');
    expect(offer.style.backgroundSize).toBe("384px 416px");
    expect(offer.style.backgroundPosition).toBe("0px -208px");
    expect(sprite.style.opacity).toBe("0");
    expect(offer.style.opacity).toBe("1");
    view.render({ row: 0, column: 0 });
    expect(sprite.style.backgroundImage).toBe('url("/assets/march-7th/spritesheet.webp")');
    expect(sprite.style.backgroundSize).toBe("1536px 2288px");
    expect(sprite.style.opacity).toBe("1");
    expect(offer.style.opacity).toBe("0");
  });
});
