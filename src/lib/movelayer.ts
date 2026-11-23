/** Geometry of the move layer: physical pixels (absolute, as the backend and
 *  the settings use them) versus CSS pixels inside the layer window, and the
 *  snapping while dragging. Pure functions, see movelayer.test.ts. */

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Absolute physical coordinate → CSS pixel inside the layer window, whose
 *  top-left corner sits at `origin` (the monitor's origin). */
export const toCss = (abs: number, origin: number, dpr: number): number => (abs - origin) / dpr;

/** CSS pixel inside the layer → absolute physical coordinate (whole pixels). */
export const toPhysical = (css: number, origin: number, dpr: number): number => Math.round(css * dpr) + origin;

/** Keeps a rectangle of `size` fully inside `area` (like the backend does). */
export function clampInto(pos: { x: number; y: number }, size: { w: number; h: number }, area: Rect): { x: number; y: number } {
  const x = Math.max(area.x, Math.min(pos.x, area.x + area.w - size.w));
  const y = Math.max(area.y, Math.min(pos.y, area.y + area.h - size.h));
  return { x, y };
}

export interface SnapOptions {
  /** Distance in physical pixels within which an edge snaps. */
  threshold: number;
  /** Grid step in physical pixels (0 = no grid); used where nothing snaps. */
  grid: number;
  /** false (Alt held): neither snapping nor grid. */
  enabled: boolean;
}

export interface SnapResult {
  x: number;
  y: number;
  /** Guide lines to draw (absolute physical x / y). */
  guidesX: number[];
  guidesY: number[];
}

type Cand = [pos: number, guide: number];

function nearest(v: number, cands: Cand[], threshold: number): Cand | null {
  let best: Cand | null = null;
  let d = threshold + 1;
  for (const c of cands) {
    const dd = Math.abs(c[0] - v);
    if (dd <= threshold && dd < d) {
      d = dd;
      best = c;
    }
  }
  return best;
}

/** Snaps the top-left corner `pos` of an object of `size` to the monitor edges
 *  and centre and to the edges of the other objects; otherwise to the grid.
 *  The result always lies inside `area`. */
export function snap(pos: { x: number; y: number }, size: { w: number; h: number }, area: Rect, others: Rect[], opt: SnapOptions): SnapResult {
  let { x, y } = pos;
  const guidesX: number[] = [];
  const guidesY: number[] = [];
  if (opt.enabled) {
    const cx: Cand[] = [
      [area.x, area.x],
      [area.x + area.w - size.w, area.x + area.w],
      [area.x + Math.round((area.w - size.w) / 2), area.x + Math.round(area.w / 2)],
    ];
    const cy: Cand[] = [
      [area.y, area.y],
      [area.y + area.h - size.h, area.y + area.h],
      [area.y + Math.round((area.h - size.h) / 2), area.y + Math.round(area.h / 2)],
    ];
    for (const o of others) {
      // left/right edges aligned, or side by side
      cx.push([o.x, o.x], [o.x + o.w - size.w, o.x + o.w], [o.x + o.w, o.x + o.w], [o.x - size.w, o.x]);
      cy.push([o.y, o.y], [o.y + o.h - size.h, o.y + o.h], [o.y + o.h, o.y + o.h], [o.y - size.h, o.y]);
    }
    const bx = nearest(x, cx, opt.threshold);
    const by = nearest(y, cy, opt.threshold);
    if (bx) {
      x = bx[0];
      guidesX.push(bx[1]);
    } else if (opt.grid > 0) {
      x = area.x + Math.round((x - area.x) / opt.grid) * opt.grid;
    }
    if (by) {
      y = by[0];
      guidesY.push(by[1]);
    } else if (opt.grid > 0) {
      y = area.y + Math.round((y - area.y) / opt.grid) * opt.grid;
    }
  }
  const c = clampInto({ x, y }, size, area);
  return { x: c.x, y: c.y, guidesX, guidesY };
}

/** Same rectangle (monitor of an object = monitor of the layer?). */
export const sameArea = (a: [number, number, number, number], b: { x: number; y: number; width: number; height: number }): boolean =>
  a[0] === b.x && a[1] === b.y && a[2] === b.width && a[3] === b.height;
