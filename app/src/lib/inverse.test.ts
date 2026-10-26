import { describe, expect, it } from "vitest";
import { interpolate, splitSpeed, stagnationIndex, strokeEdit } from "./inverse";

// A small airfoil-like distribution: upper surface from the trailing edge
// (x = 1) to the nose, then the lower surface back to the trailing edge.
const x = [1, 0.75, 0.5, 0.25, 0, 0.25, 0.5, 0.75, 1];
const target = [0.9, 1.1, 1.2, 1.3, -0.2, -0.95, -0.9, -0.85, -0.8];

describe("inverse design helpers", () => {
  it("finds the stagnation point and splits the surfaces", () => {
    expect(stagnationIndex(target)).toBe(3);
    const { upper, lower } = splitSpeed(x, target);
    expect(upper.index).toEqual([0, 1, 2, 3]);
    expect(lower.index).toEqual([4, 5, 6, 7, 8]);
    expect(lower.q[1]).toBe(0.95);
    expect(stagnationIndex([1, 2])).toBe(-1);
  });

  it("interpolates and clamps", () => {
    const pts: [number, number][] = [
      [0, 0],
      [1, 2],
      [3, 4],
    ];
    expect(interpolate(pts, 0.5)).toBe(1);
    expect(interpolate(pts, 2)).toBe(3);
    expect(interpolate(pts, -1)).toBe(0);
    expect(interpolate(pts, 9)).toBe(4);
  });

  it("applies a stroke to the nearest surface", () => {
    // Drawn just below the upper curve between x = 0.3 and 0.8.
    const up = strokeEdit(x, target, [
      [0.8, 1.0],
      [0.3, 1.1],
    ]);
    expect(up?.first).toBe(1);
    expect(up?.speed.length).toBe(2);
    expect(up?.speed[0]).toBeCloseTo(1.01, 12);
    expect(up?.speed[1]).toBeCloseTo(1.06, 12);
    // Drawn near the lower curve: negative values.
    const down = strokeEdit(x, target, [
      [0.2, 0.9],
      [0.8, 0.9],
    ]);
    expect(down?.first).toBe(5);
    expect(down?.speed).toEqual([-0.9, -0.9, -0.9]);
    // Too short or degenerate strokes change nothing.
    expect(
      strokeEdit(x, target, [
        [0.5, 1],
        [0.5, 2],
      ]),
    ).toBeNull();
    expect(strokeEdit(x, target, [[0.5, 1]])).toBeNull();
  });
});
