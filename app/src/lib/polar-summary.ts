// Key figures of a polar, for the comparison table of the Polars tab.

/** The polar columns used here (inviscid polars have no Cd). */
export interface SummaryPoint {
  alpha_deg: number | null;
  cl: number | null;
  cm: number | null;
  cd?: number | null;
}

export interface PolarSummary {
  points: number;
  /** Largest Cl and its angle of attack. */
  clMax: number | null;
  alphaClMax: number | null;
  /** Zero-lift angle of attack (interpolated). */
  alpha0: number | null;
  /** Moment coefficient at zero lift (interpolated). */
  cm0: number | null;
  /** Smallest drag and the Cl where it occurs (viscous polars). */
  cdMin: number | null;
  clCdMin: number | null;
  /** Largest lift-to-drag ratio, with its Cl and α (viscous polars). */
  ldMax: number | null;
  clLdMax: number | null;
  alphaLdMax: number | null;
  /** Values at the requested lift coefficient, interpolated below the stall. */
  atCl: { alpha: number; cd: number | null; ld: number | null; cm: number | null } | null;
}

interface Point {
  alpha: number;
  cl: number;
  cm: number;
  cd: number | null;
}

function clean(points: SummaryPoint[]): Point[] {
  const out: Point[] = [];
  for (const p of points) {
    if (p.alpha_deg === null || p.cl === null || p.cm === null) continue;
    if (!Number.isFinite(p.alpha_deg) || !Number.isFinite(p.cl) || !Number.isFinite(p.cm)) continue;
    const cd = p.cd !== undefined && p.cd !== null && Number.isFinite(p.cd) && p.cd > 0 ? p.cd : null;
    out.push({ alpha: p.alpha_deg, cl: p.cl, cm: p.cm, cd });
  }
  return out.sort((a, b) => a.alpha - b.alpha);
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

/**
 * Interpolates the branch below the stall (points up to Cl max) at the
 * lift coefficient `cl`; null if the polar does not reach it.
 */
function interpolateAtCl(branch: Point[], cl: number): PolarSummary["atCl"] {
  for (let i = 0; i + 1 < branch.length; i++) {
    const a = branch[i];
    const b = branch[i + 1];
    if ((a.cl - cl) * (b.cl - cl) > 0 || a.cl === b.cl) continue;
    const t = (cl - a.cl) / (b.cl - a.cl);
    const cd = a.cd !== null && b.cd !== null ? lerp(a.cd, b.cd, t) : null;
    return { alpha: lerp(a.alpha, b.alpha, t), cd, ld: cd ? cl / cd : null, cm: lerp(a.cm, b.cm, t) };
  }
  const exact = branch.find((p) => p.cl === cl);
  return exact ? { alpha: exact.alpha, cd: exact.cd, ld: exact.cd ? cl / exact.cd : null, cm: exact.cm } : null;
}

/** Key figures of a polar; `cl` is the lift coefficient of the "at Cl" columns. */
export function summarizePolar(points: SummaryPoint[], cl: number): PolarSummary {
  const p = clean(points);
  const empty: PolarSummary = {
    points: p.length,
    clMax: null,
    alphaClMax: null,
    alpha0: null,
    cm0: null,
    cdMin: null,
    clCdMin: null,
    ldMax: null,
    clLdMax: null,
    alphaLdMax: null,
    atCl: null,
  };
  if (p.length === 0) return empty;

  let iMax = 0;
  p.forEach((q, i) => {
    if (q.cl > p[iMax].cl) iMax = i;
  });
  const branch = p.slice(0, iMax + 1);
  const zero = interpolateAtCl(branch, 0);

  let cdMin: Point | null = null;
  let ldMax: Point | null = null;
  for (const q of p) {
    if (q.cd === null) continue;
    if (!cdMin || q.cd < (cdMin.cd as number)) cdMin = q;
    if (q.cl > 0 && (!ldMax || q.cl / q.cd > ldMax.cl / (ldMax.cd as number))) ldMax = q;
  }

  return {
    ...empty,
    clMax: p[iMax].cl,
    alphaClMax: p[iMax].alpha,
    alpha0: zero?.alpha ?? null,
    cm0: zero?.cm ?? null,
    cdMin: cdMin?.cd ?? null,
    clCdMin: cdMin?.cl ?? null,
    ldMax: ldMax ? ldMax.cl / (ldMax.cd as number) : null,
    clLdMax: ldMax?.cl ?? null,
    alphaLdMax: ldMax?.alpha ?? null,
    atCl: Number.isFinite(cl) ? interpolateAtCl(branch, cl) : null,
  };
}

/** Value of a summary used to sort the table. */
export type SummaryKey =
  | "name"
  | "clMax"
  | "alpha0"
  | "cm0"
  | "cdMin"
  | "ldMax"
  | "atClAlpha"
  | "atClCd"
  | "atClLd"
  | "atClCm";

export function summaryValue(s: PolarSummary, key: Exclude<SummaryKey, "name">): number | null {
  switch (key) {
    case "atClAlpha":
      return s.atCl?.alpha ?? null;
    case "atClCd":
      return s.atCl?.cd ?? null;
    case "atClLd":
      return s.atCl?.ld ?? null;
    case "atClCm":
      return s.atCl?.cm ?? null;
    default:
      return s[key];
  }
}

/** Sorts rows by a key; missing values always come last. */
export function sortRows<T extends { name: string; summary: PolarSummary }>(
  rows: T[],
  key: SummaryKey,
  descending: boolean,
): T[] {
  const sign = descending ? -1 : 1;
  return [...rows].sort((a, b) => {
    if (key === "name") return sign * a.name.localeCompare(b.name);
    const x = summaryValue(a.summary, key);
    const y = summaryValue(b.summary, key);
    if (x === null && y === null) return 0;
    if (x === null) return 1;
    if (y === null) return -1;
    return sign * (x - y);
  });
}
