import { describe, expect, it } from "vitest";
import { align, equalAxes, extent, minIndex, niceTicks, splitSurfaces, monotonicPieces } from "./scales";

describe("equalAxes", () => {
  it("gives the same units per pixel on both axes", () => {
    const { x, y } = equalAxes(0, 1, -0.06, 0.06, 800, 400);
    const uppX = (x[1] - x[0]) / 800;
    const uppY = (y[1] - y[0]) / 400;
    expect(uppX).toBeCloseTo(uppY, 12);
    // The data box fits inside.
    expect(x[0]).toBeLessThan(0);
    expect(x[1]).toBeGreaterThan(1);
    expect(y[0]).toBeLessThan(-0.06);
    expect(y[1]).toBeGreaterThan(0.06);
  });

  it("widens x when the data is tall", () => {
    const { x, y } = equalAxes(0, 1, 0, 10, 100, 100);
    expect(y[1] - y[0]).toBeCloseTo(11, 9);
    expect(x[1] - x[0]).toBeCloseTo(11, 9);
    expect((x[0] + x[1]) / 2).toBeCloseTo(0.5, 12);
  });

  it("survives degenerate input", () => {
    const { x, y } = equalAxes(1, 1, 2, 2, 0, 0);
    expect(Number.isFinite(x[0]) && Number.isFinite(y[1])).toBe(true);
  });
});

describe("niceTicks", () => {
  it("produces round steps", () => {
    expect(niceTicks(0, 1, 5)).toEqual([0, 0.2, 0.4, 0.6, 0.8, 1]);
    expect(niceTicks(-6, 12, 6)).toEqual([-5, 0, 5, 10]);
    expect(niceTicks(-6, 12, 9)).toEqual([-6, -4, -2, 0, 2, 4, 6, 8, 10, 12]);
    expect(niceTicks(-0.013, 0.051, 4)).toEqual([0, 0.02, 0.04]);
  });

  it("returns nothing for invalid ranges", () => {
    expect(niceTicks(1, 1)).toEqual([]);
    expect(niceTicks(Number.NaN, 1)).toEqual([]);
  });
});

describe("splitSurfaces", () => {
  it("splits a Selig contour at the leading edge with increasing x", () => {
    const x = [1, 0.5, 0, 0.5, 1];
    const y = [0, 0.1, 0, -0.1, 0];
    const le = minIndex(x);
    expect(le).toBe(2);
    const { upper, lower } = splitSurfaces(x, y, le);
    expect(upper.x).toEqual([0, 0.5, 1]);
    expect(upper.y).toEqual([0, 0.1, 0]);
    expect(lower.x).toEqual([0, 0.5, 1]);
    expect(lower.y).toEqual([0, -0.1, 0]);
  });
});

describe("align", () => {
  it("joins curves on the union of x values", () => {
    const data = align([
      { x: [0, 1, 2], y: [10, 11, 12] },
      { x: [1, 3], y: [21, null] },
    ]);
    expect(data[0]).toEqual([0, 1, 2, 3]);
    expect(data[1]).toEqual([10, 11, 12, null]);
    expect(data[2]).toEqual([null, 21, null, null]);
  });

  it("computes the extent of finite values", () => {
    const e = extent([
      { x: [0, 1], y: [5, null] },
      { x: [-1, 2], y: [Number.NaN, -3] },
    ]);
    expect(e).toEqual({ xmin: 0, xmax: 2, ymin: -3, ymax: 5 });
  });
});

describe("monotonicPieces", () => {
  it("keeps a monotonic curve whole", () => {
    expect(monotonicPieces([0, 1, 2], [5, 6, 7])).toEqual([{ x: [0, 1, 2], y: [5, 6, 7] }]);
  });

  it("splits a drag polar at its minimum drag, with increasing x pieces", () => {
    // Cd decreases then increases as the angle grows.
    const cd = [0.012, 0.008, 0.006, 0.007, 0.01];
    const cl = [-0.4, -0.2, 0.1, 0.4, 0.8];
    expect(monotonicPieces(cd, cl)).toEqual([
      { x: [0.006, 0.008, 0.012], y: [0.1, -0.2, -0.4] },
      { x: [0.006, 0.007, 0.01], y: [0.1, 0.4, 0.8] },
    ]);
  });

  it("drops non-finite x and restarts on repeated x", () => {
    expect(monotonicPieces([0, Number.NaN, 1, 1, 2], [1, 2, 3, 4, 5])).toEqual([
      { x: [0, 1], y: [1, 3] },
      { x: [1, 2], y: [4, 5] },
    ]);
  });
});
