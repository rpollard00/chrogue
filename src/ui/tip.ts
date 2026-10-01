// The info button and its tooltip. All the info buttons share one tooltip element.
import { h } from './dom';

const EDGE_PX = 8;
const GAP_PX = 10;

const title = h('strong'), body = h('span');
const bubble = h('div', { class: 'tip', role: 'tooltip', id: 'tip', hidden: true }, title, body);
let owner: HTMLElement | null = null;
let hovered: HTMLElement | null = null;

// A new screen removes the button but not the tooltip, thus the tooltip goes when its button goes.
const gone = new MutationObserver(() => {
  if (owner && !owner.isConnected) hide();
});

function hide(): void {
  if (!owner) return;
  owner.removeAttribute('aria-describedby');
  owner.setAttribute('aria-expanded', 'false');
  owner = null;
  bubble.hidden = true;
  gone.disconnect();
}

function show(button: HTMLElement, subject: string, text: string): void {
  hide();
  owner = button;
  title.textContent = subject;
  body.textContent = text;
  document.body.append(bubble);
  bubble.hidden = false;
  button.setAttribute('aria-describedby', bubble.id);
  button.setAttribute('aria-expanded', 'true');
  gone.observe(document.body, { childList: true, subtree: true });

  // The tooltip goes above the button. It goes below when the space above is too small.
  const at = button.getBoundingClientRect(), width = bubble.offsetWidth, height = bubble.offsetHeight;
  const center = at.left + at.width / 2;
  const left = Math.max(EDGE_PX, Math.min(center - width / 2, document.documentElement.clientWidth - width - EDGE_PX));
  const below = at.top - height - GAP_PX < EDGE_PX;
  bubble.classList.toggle('below', below);
  bubble.style.left = `${left}px`;
  bubble.style.top = `${below ? at.bottom + GAP_PX : at.top - height - GAP_PX}px`;
  bubble.style.setProperty('--arrow', `${center - left}px`);
}

document.addEventListener('pointerdown', (event) => {
  if (owner && !(event.target instanceof Node && owner.contains(event.target))) hide();
});
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') hide();
});
addEventListener('scroll', hide, { capture: true, passive: true });
addEventListener('resize', hide);

/** A small button that shows a text about `subject`. A pointer shows the text on hover. A touch or a key shows it on activation. */
export function infoTip(subject: string, text: string): HTMLElement {
  const button = h('button', { type: 'button', class: 'info', 'aria-label': `About ${subject}`, 'aria-expanded': 'false' }, 'i');
  const open = () => show(button, subject, text);
  button.addEventListener('pointerenter', (event) => {
    if (event.pointerType !== 'mouse') return;
    hovered = button;
    open();
  });
  button.addEventListener('pointerleave', () => {
    if (hovered !== button) return;
    hovered = null;
    hide();
  });
  button.addEventListener('focus', () => {
    if (button.matches(':focus-visible')) open();
  });
  button.addEventListener('blur', () => {
    if (owner === button) hide();
  });
  button.addEventListener('click', () => {
    // A click with the mouse does not close the tooltip that the hover opened.
    if (owner === button && hovered !== button) hide();
    else open();
  });
  return button;
}
