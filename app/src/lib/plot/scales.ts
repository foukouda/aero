// Pure helpers for plot scales and data layout.

export type Range = [number, number];

/**
 * Ranges giving the same scale (units per pixel) on both axes, containing
 * the data box with a relative margin, for a plot area of the given size.
 */
export function equalAxes(
  xmin: number,
  xmax: number,
  ymin: number,
  ymax: number,
  widthPx: number,
  heightPx: number,
  margin = 0.05,
): { x: Range; y: Range } {
  const w = Math.max(widthPx, 1);
  const h = Math.max(heightPx, 1);
  let dx = Math.max(xmax - xmin, 1e-12);
  let dy = Math.max(ymax - ymin, 1e-12);
  dx *= 1 + 2 * margin;
  dy *= 1 + 2 * margin;
  // Units per pixel needed by each axis; keep the larger one for both.
  const upp = Math.max(dx / w, dy / h);
  const cx = 0.5 * (xmin + xmax);
  const cy = 0.5 * (ymin + ymax);
  const hx = 0.5 * upp * w;
  const hy = 0.5 * upp * h;
  return { x: [cx - hx, cx + hx], y: [cy - hy, cy + hy] };
}

/** "Nice" tick values covering [min, max] with about `count` intervals. */
export function niceTicks(min: number, max: number, count = 6): number[] {
  if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) return [];
  const raw = (max - min) / Math.max(count, 1);
  const mag = 10 ** Math.floor(Math.log10(raw));
  const norm = raw / mag;
  const step = (norm < 1.5 ? 1 : norm < 3 ? 2 : norm < 7 ? 5 : 10) * mag;
  const first = Math.ceil(min / step - 1e-9) * step;
  const ticks: number[] = [];
  for (let v = first; v <= max + step * 1e-9; v += step) {
    ticks.push(Math.abs(v) < step * 1e-9 ? 0 : Number(v.toPrecision(12)));
  }
  return ticks;
}

/** Upper and lower surfaces of a Selig-ordered contour, both with
 * increasing x, split at node `leIndex` (included in both). */
export function splitSurfaces(
  x: (number | null)[],
  y: (number | null)[],
  leIndex: number,
): { upper: { x: number[]; y: number[] }; lower: { x: number[]; y: number[] } } {
  const num = (v: number | null) => (v === null ? Number.NaN : v);
  const upper = { x: [] as number[], y: [] as number[] };
  for (let i = leIndex; i >= 0; i--) {
    upper.x.push(num(x[i]));
    upper.y.push(num(y[i]));
  }
  const lower = { x: [] as number[], y: [] as number[] };
  for (let i = leIndex; i < x.length; i++) {
    lower.x.push(num(x[i]));
    lower.y.push(num(y[i]));
  }
  return { upper, lower };
}

/** Index of the smallest x (leading-edge node). */
export function minIndex(values: (number | null)[]): number {
  let best = 0;
  for (let i = 1; i < values.length; i++) {
    const v = values[i];
    const b = values[best];
    if (v !== null && (b === null || v < b)) best = i;
  }
  return best;
}

/** One curve to align with others on a common x axis. */
export interface Curve {
  x: number[];
  y: (number | null)[];
}

/**
 * Aligns curves on the sorted union of their x values (uPlot's "aligned
 * data"): returns the x column and one y column per curve, with `null`
 * where a curve has no point. Within one curve, points must have strictly
 * increasing x; duplicates keep the last value.
 */
export function align(curves: Curve[]): [number[], ...(number | null)[][]] {
  const xs = Array.from(new Set(curves.flatMap((c) => c.x.filter(Number.isFinite)))).sort(
    (a, b) => a - b,
  );
  const index = new Map(xs.map((v, i) => [v, i]));
  const columns = curves.map((c) => {
    const col: (number | null)[] = new Array(xs.length).fill(null);
    c.x.forEach((v, i) => {
      const k = index.get(v);
      if (k !== undefined) {
        const y = c.y[i];
        col[k] = y === null || y === undefined || !Number.isFinite(y) ? null : y;
      }
    });
    return col;
  });
  return [xs, ...columns];
}

/** Data extent of several curves (ignores non-finite values). */
export function extent(curves: Curve[]): { xmin: number; xmax: number; ymin: number; ymax: number } {
  let xmin = Infinity;
  let xmax = -Infinity;
  let ymin = Infinity;
  let ymax = -Infinity;
  for (const c of curves) {
    c.x.forEach((x, i) => {
      const y = c.y[i];
      if (!Number.isFinite(x) || y === null || y === undefined || !Number.isFinite(y)) return;
      xmin = Math.min(xmin, x);
      xmax = Math.max(xmax, x);
      ymin = Math.min(ymin, y);
      ymax = Math.max(ymax, y);
    });
  }
  return { xmin, xmax, ymin, ymax };
}
