// Helpers of the Optimize tab: objective display, convergence history.

import type { HistoryEntry, Objective, PointValues, Target } from "./bindings";

/** Name of the displayed objective quantity. */
export function objectiveName(objective: Objective): string {
  return objective === "max_lift_to_drag" ? "L/D" : "Cd";
}

/**
 * Displayed value of an objective: the optimizer minimizes the weighted
 * mean of Cd, or of −L/D, which is shown as L/D.
 */
export function displayObjective(objective: Objective, value: number | null | undefined): number | null {
  if (value === null || value === undefined || !Number.isFinite(value)) return null;
  return objective === "max_lift_to_drag" ? -value : value;
}

/** Improvement of the best design over the initial one, in percent (positive is better). */
export function improvement(
  objective: Objective,
  initial: number | null | undefined,
  best: number | null | undefined,
): number | null {
  const a = displayObjective(objective, initial);
  const b = displayObjective(objective, best);
  if (a === null || b === null || a === 0) return null;
  return objective === "max_lift_to_drag" ? 100 * (b / a - 1) : 100 * (1 - b / a);
}

/** True for an evaluation that satisfies every constraint. */
export function feasible(entry: HistoryEntry): boolean {
  return entry.objective !== null && (entry.violation ?? 1) <= 0;
}

/** Best feasible objective after each evaluation (null before the first feasible one). */
export function bestSoFar(history: HistoryEntry[]): (number | null)[] {
  let best: number | null = null;
  return history.map((h) => {
    if (feasible(h) && (best === null || (h.objective as number) < best)) best = h.objective;
    return best;
  });
}

/** Lift-to-drag ratio of an operating point. */
export function liftToDrag(v: PointValues | null | undefined): number | null {
  if (!v || v.cl === null || v.cd === null || v.cd === 0) return null;
  return v.cl / v.cd;
}

/** Short label of an operating point target, e.g. "Cl = 0.5" or "α = 2°". */
export function targetLabel(target: Target): string {
  const value = target.value ?? Number.NaN;
  return target.kind === "cl" ? `Cl = ${value}` : `α = ${value}°`;
}
