export type MenuItem =
  | { separator: true }
  | { label: string; onclick: () => void; danger?: boolean; disabled?: boolean; title?: string };

export interface Rect {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

/** Viewport-aware position of a menu: below its trigger and left-aligned to it;
 *  flipped to end at the trigger's right edge when it would leave the viewport
 *  on the right, clamped to a margin on both sides, and opened upwards when
 *  there is no room below (and enough above). */
export function placeMenu(
  trigger: Rect,
  menu: { width: number; height: number },
  viewport: { width: number; height: number },
  margin = 8,
): { left: number; top: number } {
  let left = trigger.left;
  if (left + menu.width > viewport.width - margin) left = trigger.right - menu.width;
  left = Math.max(margin, Math.min(left, viewport.width - margin - menu.width));
  let top = trigger.bottom + 4;
  if (top + menu.height > viewport.height - margin && trigger.top - 4 - menu.height >= margin) {
    top = trigger.top - 4 - menu.height;
  }
  return { left: Math.round(left), top: Math.round(top) };
}
