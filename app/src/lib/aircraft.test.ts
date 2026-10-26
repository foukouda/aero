import { describe, expect, it } from "vitest";
import type { SectionParams, SurfaceParams } from "./bindings";
import {
  addTipSection,
  defaultSurface,
  insertSection,
  panelAngle,
  removeSection,
  setAllAirfoils,
  setPanelAngle,
  setSection,
  setSurfaceSpan,
  surfaceSpan,
} from "./aircraft";

const section = (y: number, chord: number, x = 0, z = 0, airfoil = "naca:2412"): SectionParams => ({
  y,
  chord,
  x,
  z,
  twist_deg: 0,
  airfoil,
});

/** Root, a kink at 0.3 m (10° dihedral) and the tip at 0.6 m. */
const wing: SurfaceParams = {
  x: 0.25,
  z: 0,
  incidence_deg: 2,
  sections: [
    section(0, 0.2),
    section(0.3, 0.2, 0, 0.3 * Math.tan(Math.PI / 18)),
    section(0.6, 0.1, 0.05, 0.3 * Math.tan(Math.PI / 18), "catalogue:sd7037"),
  ],
};

describe("panel angles", () => {
  it("are read from the sections", () => {
    expect(panelAngle(wing.sections, 1, "z")).toBeCloseTo(10, 12);
    expect(panelAngle(wing.sections, 2, "z")).toBeCloseTo(0, 12);
    expect(panelAngle(wing.sections, 2, "x")).toBeCloseTo((Math.atan(0.05 / 0.3) * 180) / Math.PI, 12);
    expect(panelAngle(wing.sections, 0, "z")).toBe(0);
  });

  it("move the sections further out with them", () => {
    const s = setPanelAngle(wing.sections, 1, "z", 5);
    expect(panelAngle(s, 1, "z")).toBeCloseTo(5, 12);
    // The outer panel keeps its own (zero) dihedral: the tip moved too.
    expect(panelAngle(s, 2, "z")).toBeCloseTo(0, 12);
    expect(s[0]).toBe(wing.sections[0]);
    const swept = setPanelAngle(wing.sections, 2, "x", 20);
    expect(panelAngle(swept, 2, "x")).toBeCloseTo(20, 12);
    expect(swept[1]).toEqual(wing.sections[1]);
  });
});

describe("sections", () => {
  it("are inserted halfway with the inner airfoil", () => {
    const s = insertSection(wing.sections, 2);
    expect(s).toHaveLength(4);
    expect(s[2].y).toBeCloseTo(0.45, 15);
    expect(s[2].chord).toBeCloseTo(0.15, 15);
    expect(s[2].x).toBeCloseTo(0.025, 15);
    expect(s[2].airfoil).toBe("naca:2412");
    expect(insertSection(wing.sections, 0)).toBe(wing.sections);
  });

  it("are added at the tip, continuing the last panel", () => {
    const s = addTipSection(wing.sections);
    expect(s).toHaveLength(4);
    expect(s[3].y).toBeCloseTo(0.75, 15);
    expect(s[3].chord).toBeCloseTo(0.08, 15);
    expect(panelAngle(s, 3, "x")).toBeCloseTo(panelAngle(wing.sections, 2, "x"), 12);
    expect(panelAngle(s, 3, "z")).toBeCloseTo(0, 12);
    expect(s[3].airfoil).toBe("catalogue:sd7037");
  });

  it("are removed, keeping the root and two sections", () => {
    expect(removeSection(wing.sections, 1)).toHaveLength(2);
    expect(removeSection(wing.sections, 0)).toBe(wing.sections);
    const two = removeSection(wing.sections, 1);
    expect(removeSection(two, 1)).toBe(two);
  });

  it("change one at a time", () => {
    const s = setSection(wing.sections, 1, { twist_deg: -2, airfoil: "naca:0012" });
    expect(s[1].twist_deg).toBe(-2);
    expect(s[1].airfoil).toBe("naca:0012");
    expect(s[2]).toBe(wing.sections[2]);
  });
});

describe("surfaces", () => {
  it("are stretched along the span keeping their angles", () => {
    expect(surfaceSpan(wing, "wing")).toBeCloseTo(1.2, 15);
    const w = setSurfaceSpan(wing, "wing", 1.8);
    expect(surfaceSpan(w, "wing")).toBeCloseTo(1.8, 12);
    expect(panelAngle(w.sections, 1, "z")).toBeCloseTo(10, 12);
    expect(w.sections[2].chord).toBe(0.1);
    expect(surfaceSpan(wing, "fin")).toBeCloseTo(0.6, 15);
  });

  it("get one airfoil everywhere", () => {
    expect(setAllAirfoils(wing, "naca:0015").sections.every((s) => s.airfoil === "naca:0015")).toBe(true);
  });

  it("are created in proportion to the wing", () => {
    const canard = defaultSurface("canard", wing);
    // Behind the nose, ahead of the wing.
    expect(canard.x).toBeCloseTo(0.036, 12);
    expect(canard.x! + canard.sections[0].chord!).toBeLessThan(wing.x!);
    expect(canard.incidence_deg).toBeGreaterThan(0);
    // No room ahead of a wing at the nose: ahead of the wing.
    expect(defaultSurface("canard", { ...wing, x: 0 }).x).toBeCloseTo(-0.54, 12);
    const tail = defaultSurface("tail", wing);
    expect(tail.x).toBeGreaterThan(wing.x!);
    expect(surfaceSpan(tail, "tail")).toBeCloseTo(0.35 * 1.2, 12);
    const fin = defaultSurface("fin", wing);
    expect(panelAngle(fin.sections, 1, "x")).toBeCloseTo(30, 10);
    expect(fin.sections[1].y).toBeCloseTo(0.14 * 1.2, 12);
  });
});
