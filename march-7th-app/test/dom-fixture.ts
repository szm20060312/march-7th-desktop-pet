// Explicit minimal DOM host for view-port tests; real browser layout is a separate QA step.
export class ElementDouble {
  children: ElementDouble[] = []; dataset: Record<string, string> = {}; attributes: Record<string, string> = {};
  style: Record<string, string> = {};
  textContent = ""; hidden = false; disabled = false; checked = false; value = ""; className = ""; type = "";
  listeners = new Map<string, Set<(event: Event) => void>>();
  get valueAsNumber() { return this.value === "" ? NaN : Number(this.value); }
  append(...nodes: ElementDouble[]) { this.children.push(...nodes); }
  replaceChildren(...nodes: ElementDouble[]) { this.children = nodes; }
  setAttribute(name: string, value: string) { this.attributes[name] = value; }
  addEventListener(name: string, callback: (event: Event) => void) { const listeners = this.listeners.get(name) ?? new Set(); listeners.add(callback); this.listeners.set(name, listeners); }
  removeEventListener(name: string, callback: (event: Event) => void) { this.listeners.get(name)?.delete(callback); }
  dispatch(name: string, event = { preventDefault() {} } as Event) { for (const callback of this.listeners.get(name) ?? []) callback(event); }
}
export function documentDouble() {
  const elements = new Map<string, ElementDouble>();
  const get = (id: string) => { let el = elements.get(id); if (!el) { el = new ElementDouble(); elements.set(id, el); } return el; };
  return { get, document: { getElementById: get, createElement: () => new ElementDouble() } as unknown as Document };
}
