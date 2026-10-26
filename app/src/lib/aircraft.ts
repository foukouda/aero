// Editing the sections of a surface (My aircraft): angles of the panels,
// inserting and removing sections, scaling the span, default parts. Pure
// functions on copies, tested in aircraft.test.ts; the backend
// (aero-aircraft) validates the result.

import type { SectionParams, SurfaceParams } from "./bindings";

export type SurfaceKind = "wing" | "canard" | "tail" | "fin";
export type Axis = "x" | "z";

const DEG = Math.PI / 180;

function n(v: number | null | undefined): number {
  return v ?? 0;
}

/** Angle of the panel ending at section `i` (from section `i − 1`),
 * degrees: its sweep for `x` (leading edge going back), its dihedral for
 * `z`. 0 for the root. */
export function panelAngle(sections: SectionParams[], i: number, axis: Axis): number {
  if (i <= 0 || i >= sections.length) return 0;
  const dy = n(sections[i].y) - n(sections[i - 1].y);
  if (!(dy > 0)) return 0;
  return Math.atan((n(sections[i][axis]) - n(sections[i - 1][axis])) / dy) / DEG;
}

/** Sets the angle of the panel ending at section `i`; the sections further
 * out move with it, so that their own panels keep their angles. */
export function setPanelAngle(sections: SectionParams[], i: number, axis: Axis, deg: number): SectionParams[] {
  if (i <= 0 || i >= sections.length) return sections;
  const dy = n(sections[i].y) - n(sections[i - 1].y);
  const target = n(sections[i - 1][axis]) + dy * Math.tan(deg * DEG);
  const shift = target - n(sections[i][axis]);
  return sections.map((s, k) => (k >= i ? { ...s, [axis]: n(s[axis]) + shift } : s));
}

/** A section halfway between sections `i − 1` and `i` (the airfoil of the
 * inner one), inserted before `i`. */
export function insertSection(sections: SectionParams[], i: number): SectionParams[] {
  if (i <= 0 || i >= sections.length) return sections;
  const [a, b] = [sections[i - 1], sections[i]];
  const mid = (key: "y" | "chord" | "x" | "z" | "twist_deg") => 0.5 * (n(a[key]) + n(b[key]));
  const section: SectionParams = {
    y: mid("y"),
    chord: mid("chord"),
    x: mid("x"),
    z: mid("z"),
    twist_deg: mid("twist_deg"),
    airfoil: a.airfoil,
  };
  return [...sections.slice(0, i), section, ...sections.slice(i)];
}

/** A new tip section beyond the last one, continuing the last panel's
 * sweep and dihedral over a quarter of the half span, with 80 % of the
 * chord. */
export function addTipSection(sections: SectionParams[]): SectionParams[] {
  const last = sections[sections.length - 1];
  if (!last) return sections;
  const i = sections.length - 1;
  const dy = 0.25 * Math.max(n(last.y), 0.05);
  const slope = (axis: Axis) => Math.tan(panelAngle(sections, i, axis) * DEG);
  return [
    ...sections,
    {
      ...last,
      y: n(last.y) + dy,
      chord: 0.8 * n(last.chord),
      x: n(last.x) + dy * slope("x"),
      z: n(last.z) + dy * slope("z"),
    },
  ];
}

/** Removes section `i` (never the root, and at least two remain). */
export function removeSection(sections: SectionParams[], i: number): SectionParams[] {
  if (i <= 0 || i >= sections.length || sections.length <= 2) return sections;
  return sections.filter((_, k) => k !== i);
}

/** Changes a field of section `i` (only that section moves). */
export function setSection(
  sections: SectionParams[],
  i: number,
  patch: Partial<SectionParams>,
): SectionParams[] {
  return sections.map((s, k) => (k === i ? { ...s, ...patch } : s));
}

/** Span of a surface: tip to tip for a mirrored one, height for a fin. */
export function surfaceSpan(surface: SurfaceParams, kind: SurfaceKind): number {
  const half = n(surface.sections[surface.sections.length - 1]?.y);
  return kind === "fin" ? half : 2 * half;
}

/** The surface stretched along the span to `span` (positions, sweep
 * offsets and heights scaled, so the angles are kept; chords unchanged). */
export function setSurfaceSpan(surface: SurfaceParams, kind: SurfaceKind, span: number): SurfaceParams {
  const current = surfaceSpan(surface, kind);
  if (!(current > 0) || !(span > 0)) return surface;
  const k = span / current;
  return {
    ...surface,
    sections: surface.sections.map((s) => ({ ...s, y: n(s.y) * k, x: n(s.x) * k, z: n(s.z) * k })),
  };
}

/** Every section gets the airfoil `key`. */
export function setAllAirfoils(surface: SurfaceParams, key: string): SurfaceParams {
  return { ...surface, sections: surface.sections.map((s) => ({ ...s, airfoil: key })) };
}

/** A new part proportional to the wing: a canard ahead of it, a tail or a
 * fin behind it. */
export function defaultSurface(kind: Exclude<SurfaceKind, "wing">, wing: SurfaceParams): SurfaceParams {
  const span = surfaceSpan(wing, "wing") || 1;
  const chord = n(wing.sections[0]?.chord) || 0.2;
  const x = n(wing.x);
  const section = (y: number, c: number, dx: number): SectionParams => ({
    y,
    chord: c,
    x: dx,
    z: 0,
    twist_deg: 0,
    airfoil: kind === "canard" ? "naca:2412" : "naca:0009",
  });
  switch (kind) {
    case "canard": {
      // Just behind the nose when there is room ahead of the wing,
      // otherwise ahead of the wing (a flying wing has no nose).
      const nearNose = 0.03 * span;
      const room = nearNose + 0.6 * chord < x - 0.1 * chord;
      return {
        x: room ? nearNose : x - 0.45 * span,
        z: 0,
        incidence_deg: 2,
        sections: [section(0, 0.5 * chord, 0), section(0.2 * span, 0.4 * chord, 0)],
      };
    }
    case "tail":
      return {
        x: x + 0.55 * span,
        z: 0,
        incidence_deg: 0,
        sections: [section(0, 0.6 * chord, 0), section(0.175 * span, 0.45 * chord, 0)],
      };
    case "fin": {
      const height = 0.14 * span;
      return {
        x: x + 0.55 * span,
        z: 0,
        incidence_deg: 0,
        sections: [section(0, 0.6 * chord, 0), section(height, 0.35 * chord, height * Math.tan(30 * DEG))],
      };
    }
  }
}
