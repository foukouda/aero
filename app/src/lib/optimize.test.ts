import { describe, expect, it } from "vitest";
import type { HistoryEntry } from "./bindings";
import { bestSoFar, displayObjective, feasible, improvement, liftToDrag, targetLabel } from "./optimize";

function entry(evaluation: number, objective: number | null, violation = 0): HistoryEntry {
  return { evaluation, phase: "global", objective, violation };
}

describe("optimization helpers", () => {
  it("shows L/D for the lift-to-drag objective", () => {
    expect(displayObjective("min_drag", 0.006)).toBe(0.006);
    expect(displayObjective("max_lift_to_drag", -80)).toBe(80);
    expect(displayObjective("min_drag", null)).toBeNull();
  });

  it("measures improvements as positive percentages", () => {
    expect(improvement("min_drag", 0.006, 0.0048)).toBeCloseTo(20);
    expect(improvement("max_lift_to_drag", -80, -100)).toBeCloseTo(25);
    expect(improvement("min_drag", null, 0.005)).toBeNull();
  });

  it("tracks the best feasible design", () => {
    const history = [
      entry(1, 0.006, 0.2), // infeasible
      entry(2, null, 10), // failed
      entry(3, 0.0062),
      entry(4, 0.0058),
      entry(5, 0.004, 0.1), // better but infeasible
      entry(6, 0.0059),
    ];
    expect(history.map(feasible)).toEqual([false, false, true, true, false, true]);
    expect(bestSoFar(history)).toEqual([null, null, 0.0062, 0.0058, 0.0058, 0.0058]);
  });

  it("formats points", () => {
    const v = { alpha_deg: 2, cl: 0.5, cd: 0.005, cdp: 0, cdf: 0, cm: 0, transition: [1, 1] as [number, number] };
    expect(liftToDrag(v)).toBeCloseTo(100);
    expect(liftToDrag(null)).toBeNull();
    expect(targetLabel({ kind: "cl", value: 0.5 })).toBe("Cl = 0.5");
    expect(targetLabel({ kind: "alpha", value: 2 })).toBe("α = 2°");
  });
});
