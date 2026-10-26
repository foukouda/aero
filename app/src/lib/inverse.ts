// Helpers of the inverse design view: surface speed distributions split
// into the two surfaces, and target edits from strokes drawn on the plot.

/** Points of one surface: abscissas, speed magnitudes and their indices. */
export interface SurfaceSpeed {
  x: number[];
  q: number[];
  index: number[];
}

/** Last index before the stagnation point (where the signed speed goes
 * from positive to negative), or -1. */
export function stagnationIndex(speed: number[]): number {
  for (let i = 0; i + 1 < speed.length; i++) {
    if (speed[i] >= 0 && speed[i + 1] < 0) return i;
  }
  return -1;
}

/** Splits a signed speed distribution (XFOIL's γ: positive from the upper
 * trailing edge to the stagnation point, negative after) into the upper
 * and lower surfaces, as speed magnitudes q/V∞. */
export function splitSpeed(x: number[], speed: number[]): { upper: SurfaceSpeed; lower: SurfaceSpeed } {
  const stag = stagnationIndex(speed);
  const upper: SurfaceSpeed = { x: [], q: [], index: [] };
  const lower: SurfaceSpeed = { x: [], q: [], index: [] };
  const n = Math.min(x.length, speed.length);
  for (let i = 0; i < n; i++) {
    const s = i <= stag ? upper : lower;
    s.x.push(x[i]);
    s.q.push(Math.abs(speed[i]));
    s.index.push(i);
  }
  return { upper, lower };
}

/** Linear interpolation in points sorted by x, clamped at the ends. */
export function interpolate(points: [number, number][], x: number): number {
  const n = points.length;
  if (x <= points[0][0]) return points[0][1];
  if (x >= points[n - 1][0]) return points[n - 1][1];
  let lo = 0;
  let hi = n - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (points[mid][0] <= x) lo = mid;
    else hi = mid;
  }
  const [x0, y0] = points[lo];
  const [x1, y1] = points[hi];
  return x1 > x0 ? y0 + ((y1 - y0) * (x - x0)) / (x1 - x0) : y0;
}

/** Speed magnitude of a surface at x (its points sorted by x). */
function surfaceAt(s: SurfaceSpeed, x: number): number {
  const pts = s.x.map((v, k): [number, number] => [v, s.q[k]]).sort((a, b) => a[0] - b[0]);
  return pts.length ? interpolate(pts, x) : Number.NaN;
}

/** The target change made by a stroke drawn over the speed plot: the
 * surface whose target is nearest to the stroke, and on it the points
 * between the ends of the stroke take the drawn speed. Returns the first
 * changed point and the new signed values, or null. */
export function strokeEdit(
  x: number[],
  target: number[],
  stroke: [number, number][],
): { first: number; speed: number[] } | null {
  const pts = stroke
    .filter(([a, b]) => Number.isFinite(a) && Number.isFinite(b))
    .sort((a, b) => a[0] - b[0]);
  if (pts.length < 2) return null;
  const x0 = pts[0][0];
  const x1 = pts[pts.length - 1][0];
  if (!(x1 > x0)) return null;
  const { upper, lower } = splitSpeed(x, target);
  const xm = 0.5 * (x0 + x1);
  const qm = interpolate(pts, xm);
  const dUpper = Math.abs(surfaceAt(upper, xm) - qm);
  const dLower = Math.abs(surfaceAt(lower, xm) - qm);
  const onUpper = !(dLower < dUpper);
  const surface = onUpper ? upper : lower;
  const sign = onUpper ? 1 : -1;
  const inside = surface.index.filter((i) => x[i] >= x0 && x[i] <= x1);
  if (inside.length < 2) return null;
  const first = Math.min(...inside);
  const last = Math.max(...inside);
  const speed: number[] = [];
  for (let i = first; i <= last; i++) {
    speed.push(x[i] >= x0 && x[i] <= x1 ? sign * Math.abs(interpolate(pts, x[i])) : target[i]);
  }
  return { first, speed };
}
