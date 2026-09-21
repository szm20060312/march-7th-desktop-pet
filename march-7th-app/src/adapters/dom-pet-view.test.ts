import { describe, expect, it } from "vitest";
import { march7th } from "../characters/march-7th";
import { createDomPetView } from "./dom-pet-view";

function element() {
  const listeners = new Map<string, (event: { button: number }) => void>();
  const attributes = new Map<string, string>();
  const style: Record<string, string> = {};
  const dataset: Record<string, string> = {};
  const node = {
    style, dataset,
    getBoundingClientRect: () => ({ left: 24, top: 26, width: 192, height: 208 }),
    setAttribute: (name: string, value: string) => attributes.set(name, value),
    addEventListener: (name: string, callback: (event: { button: number }) => void) => listeners.set(name, callback),
    removeEventListener: (name: string) => listeners.delete(name),
  } as unknown as HTMLElement;
  return { node, style, dataset, listeners, attributes };
}

describe("DOM pet view", () => {
  it("renders the configured atlas and invalidates its frame cache on reconfiguration", () => {
    const sprite = element();
    const stage = element();
    const body = element();
    const view = createDomPetView(sprite.node, stage.node, body.node);
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
  it("only forwards primary pointer input and removes the listener", () => {
    const sprite = element();
    const stage = element();
    const body = element();
    const view = createDomPetView(sprite.node, stage.node, body.node);
    let count = 0;
    const remove = view.onDragStart(() => { count++; });
    stage.listeners.get("pointerdown")!({ button: 2 });
    stage.listeners.get("pointerdown")!({ button: 0 });
    expect(count).toBe(1);
    remove();
    expect(stage.listeners.size).toBe(0);
  });
});
