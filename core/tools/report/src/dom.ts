type Child = Node | string;

interface Props {
  class?: string;
  text?: string;
  title?: string;
  attrs?: Record<string, string>;
}

export function h<K extends keyof HTMLElementTagNameMap>(tag: K, props: Props = {}, ...children: Child[]): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  if (props.class) el.className = props.class;
  if (props.text !== undefined) el.textContent = props.text;
  if (props.title) el.title = props.title;
  for (const [name, value] of Object.entries(props.attrs ?? {})) el.setAttribute(name, value);
  el.append(...children);
  return el;
}

export function svg<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | number> = {},
  ...children: Child[]
): SVGElementTagNameMap[K] {
  const el = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [name, value] of Object.entries(attrs)) el.setAttribute(name, String(value));
  el.append(...children);
  return el;
}

export function button(label: string, onClick: () => void, props: Props = {}): HTMLButtonElement {
  const el = h("button", { ...props, text: label, attrs: { type: "button", ...props.attrs } });
  el.addEventListener("click", onClick);
  return el;
}

export interface TipContent {
  title: string;
  rows: [value: string, label: string][];
}

const tip = h("div", { class: "tip", attrs: { role: "tooltip", hidden: "" } });

function placeTip(x: number, y: number): void {
  const pad = 12;
  const { width, height } = tip.getBoundingClientRect();
  const left = x + pad + width > window.innerWidth ? x - pad - width : x + pad;
  const top = y + pad + height > window.innerHeight ? y - pad - height : y + pad;
  tip.style.left = `${Math.max(4, left)}px`;
  tip.style.top = `${Math.max(4, top)}px`;
}

function showTip(content: TipContent, x: number, y: number): void {
  if (!tip.isConnected) document.body.append(tip);
  tip.replaceChildren(
    h("div", { class: "tip-title", text: content.title }),
    ...content.rows.map(([value, label]) => h("div", { class: "tip-row" }, h("b", { text: value }), h("span", { text: label }))),
  );
  tip.hidden = false;
  placeTip(x, y);
}

export function attachTip(el: Element, content: () => TipContent): void {
  el.addEventListener("pointermove", (event) => {
    if (event instanceof PointerEvent) showTip(content(), event.clientX, event.clientY);
  });
  el.addEventListener("pointerleave", () => (tip.hidden = true));
  el.addEventListener("focus", () => {
    const box = el.getBoundingClientRect();
    showTip(content(), box.left + box.width / 2, box.bottom - 8);
  });
  el.addEventListener("blur", () => (tip.hidden = true));
}

export function download(name: string, content: string, type: string): void {
  const url = URL.createObjectURL(new Blob([content], { type }));
  const link = h("a", { attrs: { href: url, download: name } });
  link.click();
  URL.revokeObjectURL(url);
}
