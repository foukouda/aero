import { describe, expect, it } from "vitest";
import { sortRows, summarizePolar, type SummaryPoint } from "./polar-summary";

/** A synthetic polar: Cl = 0.1 (α + 2), stall above 12°, Cd parabolic. */
function polar(): SummaryPoint[] {
  const out: SummaryPoint[] = [];
  for (let a = -4; a <= 16; a += 1) {
    const cl = a <= 12 ? 0.1 * (a + 2) : 1.4 - 0.15 * (a - 12);
    out.push({ alpha_deg: a, cl, cm: -0.05 + 0.001 * a, cd: 0.006 + 0.004 * (cl - 0.4) ** 2 });
  }
  return out;
}

describe("polar summary", () => {
  it("finds the key figures", () => {
    const s = summarizePolar(polar(), 0.5);
    expect(s.points).toBe(21);
    expect(s.clMax).toBeCloseTo(1.4);
    expect(s.alphaClMax).toBe(12);
    expect(s.alpha0).toBeCloseTo(-2);
    expect(s.cm0).toBeCloseTo(-0.052);
    expect(s.cdMin).toBeCloseTo(0.006);
    expect(s.clCdMin).toBeCloseTo(0.4);
    // At Cl = 0.5: α = 3°, Cd interpolated between Cl 0.4 and 0.6.
    expect(s.atCl?.alpha).toBeCloseTo(3);
    expect(s.atCl?.cd).toBeCloseTo(0.006 + 0.004 * 0.01);
    expect(s.atCl?.ld).toBeCloseTo(0.5 / (0.006 + 0.004 * 0.01));
    expect(s.ldMax).toBeGreaterThan(s.atCl?.ld ?? 0);
  });

  it("interpolates below the stall only and handles missing data", () => {
    // Cl = 1.3 is reached before and after the stall: the first one counts.
    expect(summarizePolar(polar(), 1.3).atCl?.alpha).toBeCloseTo(11);
    // Not reached at all.
    expect(summarizePolar(polar(), 2.5).atCl).toBeNull();
    // Inviscid polar: no drag.
    const inviscid = polar().map(({ alpha_deg, cl, cm }) => ({ alpha_deg, cl, cm }));
    const s = summarizePolar(inviscid, 0.5);
    expect(s.cdMin).toBeNull();
    expect(s.ldMax).toBeNull();
    expect(s.atCl?.cd).toBeNull();
    expect(s.atCl?.alpha).toBeCloseTo(3);
    expect(summarizePolar([], 0.5).clMax).toBeNull();
  });

  it("sorts with missing values last", () => {
    const rows = [
      { name: "b", summary: summarizePolar(polar(), 0.5) },
      { name: "a", summary: summarizePolar([], 0.5) },
      { name: "c", summary: summarizePolar(polar().slice(0, 10), 0.5) },
    ];
    expect(sortRows(rows, "clMax", true).map((r) => r.name)).toEqual(["b", "c", "a"]);
    expect(sortRows(rows, "clMax", false).map((r) => r.name)).toEqual(["c", "b", "a"]);
    expect(sortRows(rows, "name", false).map((r) => r.name)).toEqual(["a", "b", "c"]);
  });
});
