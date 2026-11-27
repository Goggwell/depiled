// Small popups that must not be cut off by the scrolling box or dialog they open from.

/** Moves the element to the end of the page, so no dialog, scroll box or transform around
 *  where it was written can clip or shift it. */
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return { destroy: () => node.remove() };
}

/** Where to put a popup of `width` × `height` next to `anchor`: below it if there's room,
 *  otherwise above, lined up with its left edge (or `align` right) and kept on screen. */
export function place(anchor: HTMLElement, width: number, height: number, align: 'left' | 'right' = 'left') {
  const r = anchor.getBoundingClientRect();
  const gap = 4;
  const room = window.innerHeight - r.bottom - gap;
  const top = room >= height || room >= r.top ? r.bottom + gap : Math.max(8, r.top - gap - height);
  const left = align === 'left' ? r.left : r.right - width;
  return { top, left: Math.min(Math.max(8, left), window.innerWidth - width - 8), maxHeight: Math.max(160, room >= height || room >= r.top ? room - 8 : r.top - 16) };
}
