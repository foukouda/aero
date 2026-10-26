// "My aircraft" tab: example flights, how to read the flight's numbers, and
// the order of the recommended airfoils. Pure functions, tested in
// mission.test.ts; the physics is in the Rust crate aero-mission.

import type {
  Aircraft,
  CandidateResult,
  Conditions,
  DragBreakdown,
  Goal,
  MissionSetup,
  Preset as AircraftPreset,
  RecommendResult,
  StallResult,
} from "./bindings";
import type { Lang, MessageKey } from "./i18n";

/** Standard gravity, m/s². */
export const G0 = 9.80665;

export type Level = "ok" | "info" | "warning" | "danger";

export interface Reading {
  level: Level;
  text: MessageKey;
}

export interface Preset {
  /** The example aircraft (aero-aircraft `Preset`). */
  aircraft: AircraftPreset;
  label: MessageKey;
  speedKmh: number;
  altitude: number;
  mass: number;
  /** Wing area of the example aircraft, m² (checked in mission.test.ts). */
  wingArea: number;
}

/** Example aircraft and a typical flight for each; the wing loadings and
 * lift coefficients are typical of each kind of aircraft. */
export const PRESETS: Preset[] = [
  { aircraft: "trainer", label: "aircraftPreset.trainer", speedKmh: 40, altitude: 100, mass: 0.9, wingArea: 0.24 },
  { aircraft: "glider", label: "aircraftPreset.glider", speedKmh: 30, altitude: 100, mass: 0.6, wingArea: 0.2325 },
  { aircraft: "flying_wing", label: "aircraftPreset.flying_wing", speedKmh: 35, altitude: 100, mass: 0.5, wingArea: 0.21 },
  // Cessna 172: 1111 kg, 16.2 m², cruise 226 km/h at 2400 m.
  {
    aircraft: "light_aircraft",
    label: "aircraftPreset.light_aircraft",
    speedKmh: 226,
    altitude: 2400,
    mass: 1111,
    wingArea: 16.225,
  },
];

/** The setup with the preset's flight and aircraft (the chords are given). */
export function applyPreset(setup: MissionSetup, preset: Preset, aircraft: Aircraft): MissionSetup {
  return {
    ...setup,
    flight: {
      ...setup.flight,
      speed: preset.speedKmh / 3.6,
      altitude: preset.altitude,
      mass: preset.mass,
    },
    aircraft,
    sizing_cl: null,
  };
}

/** Reading of the horizontal tail volume coefficient (usual 0.35 to 0.8). */
export function readTailVolume(v: number | null | undefined): Reading {
  if (v === null || v === undefined || !Number.isFinite(v)) return { level: "info", text: "tail.none" };
  if (v < 0.3) return { level: "warning", text: "tail.small" };
  if (v <= 0.8) return { level: "ok", text: "tail.ok" };
  return { level: "info", text: "tail.large" };
}

/** Reading of the vertical tail volume coefficient (usual 0.02 to 0.06;
 * a swept flying wing does with less). */
export function readFinVolume(v: number | null | undefined, flyingWing = false): Reading {
  if (v === null || v === undefined || !Number.isFinite(v)) return { level: "info", text: "fin.none" };
  if (v < 0.02) return flyingWing ? { level: "info", text: "fin.flyingWing" } : { level: "warning", text: "fin.small" };
  if (v <= 0.06) return { level: "ok", text: "fin.ok" };
  return { level: "info", text: "fin.large" };
}

/** Critical amplification exponents offered as "air turbulence". */
export const TURBULENCE: { ncrit: number; label: MessageKey }[] = [
  { ncrit: 9, label: "turbulence.calm" },
  { ncrit: 7, label: "turbulence.normal" },
  { ncrit: 5, label: "turbulence.rough" },
];

export const GOALS: { goal: Goal; label: MessageKey; hint: MessageKey }[] = [
  { goal: "least_drag", label: "goal.least_drag", hint: "goal.least_drag.hint" },
  { goal: "slow_flight", label: "goal.slow_flight", hint: "goal.slow_flight.hint" },
  { goal: "easy_to_build", label: "goal.easy_to_build", hint: "goal.easy_to_build.hint" },
];

/** How demanding the required lift coefficient is, before any airfoil is
 * chosen (model airfoils reach about 1.2 to 1.4 at low Reynolds numbers). */
export function readCl(cl: number): Reading {
  if (cl < 0.2) return { level: "info", text: "card.cl.low" };
  if (cl <= 0.8) return { level: "ok", text: "card.cl.ok" };
  if (cl <= 1.1) return { level: "warning", text: "card.cl.high" };
  return { level: "danger", text: "card.cl.veryHigh" };
}

/** The flow regime of a Reynolds number. */
export function readReynolds(re: number): Reading {
  if (re < 5e4) return { level: "danger", text: "card.re.veryLow" };
  if (re < 1e5) return { level: "warning", text: "card.re.low" };
  if (re < 5e5) return { level: "ok", text: "card.re.model" };
  if (re < 3e6) return { level: "ok", text: "card.re.large" };
  return { level: "ok", text: "card.re.full" };
}

/** Wing loading in g/dm² (1 kg/m² = 10 g/dm²); NaN when unknown. */
export function wingLoadingGdm2(conditions: Conditions): number {
  return ((conditions.wing_loading ?? Number.NaN) / G0) * 10;
}

/** The kind of aircraft a wing loading (g/dm²) is typical of. */
export function readWingLoading(gdm2: number): MessageKey {
  if (gdm2 < 15) return "loading.veryLight";
  if (gdm2 < 35) return "loading.light";
  if (gdm2 < 70) return "loading.medium";
  if (gdm2 < 150) return "loading.heavy";
  return "loading.fullSize";
}

export function readMach(mach: number): Reading {
  return mach < 0.3 ? { level: "ok", text: "card.mach.ok" } : { level: "info", text: "card.mach.high" };
}

/** Candidates in the order of `goal`, best first. */
export function ranked(result: RecommendResult, goal: Goal): CandidateResult[] {
  return result.rankings[goal].map((i) => result.candidates[i]);
}

/** Whether a candidate can be recommended for `goal`. */
export function recommendable(c: CandidateResult, goal: Goal): boolean {
  const e = c.evaluation;
  if (!e || !(e.fit === "comfortable" || e.fit === "tight")) return false;
  return goal !== "easy_to_build" || e.buildability.easy;
}

/** The best `n` recommendable candidates for `goal`. */
export function best(result: RecommendResult, goal: Goal, n = 3): CandidateResult[] {
  return ranked(result, goal)
    .filter((c) => recommendable(c, goal))
    .slice(0, n);
}

// ------------------------------------------------------------- formatting

function locale(lang: Lang): string {
  return lang === "fr" ? "fr-FR" : "en-US";
}

/** A number with `digits` decimals, "—" when missing. */
export function num(value: number | null | undefined, lang: Lang, digits = 2): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  return new Intl.NumberFormat(locale(lang), {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

/** A number rounded to `significant` digits, grouped ("75 500"). */
export function sig(value: number | null | undefined, lang: Lang, significant = 3): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  return new Intl.NumberFormat(locale(lang), { maximumSignificantDigits: significant }).format(value);
}

/** A length in m, shown in cm below 1 m. */
export function length(m: number | null | undefined, lang: Lang): string {
  if (m === null || m === undefined || !Number.isFinite(m)) return "—";
  return Math.abs(m) < 1 ? `${sig(m * 100, lang, 3)} cm` : `${sig(m, lang, 3)} m`;
}

/** A speed in m/s, shown in km/h. */
export function speedKmh(ms: number | null | undefined, lang: Lang): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms)) return "—";
  return `${sig(ms * 3.6, lang, 3)} km/h`;
}

/** A mass in kg, shown in g below 1 kg. */
export function mass(kg: number | null | undefined, lang: Lang): string {
  if (kg === null || kg === undefined || !Number.isFinite(kg)) return "—";
  return kg < 1 ? `${sig(kg * 1000, lang, 3)} g` : `${sig(kg, lang, 4)} kg`;
}

/** A power in W, shown in kW from 1 kW. */
export function power(w: number | null | undefined, lang: Lang): string {
  if (w === null || w === undefined || !Number.isFinite(w)) return "—";
  return w < 1000 ? `${sig(w, lang, 3)} W` : `${sig(w / 1000, lang, 3)} kW`;
}

/** Colours of the three recommended airfoils: the first three slots of the
 * dataviz reference palette, distinguishable together under colour-vision
 * deficiencies in both themes; the other candidates are grey. */
export const TOP_COLORS: Record<"light" | "dark", string[]> = {
  light: ["#2a78d6", "#eb6834", "#1baf7a"],
  dark: ["#3987e5", "#d95926", "#199e70"],
};

// ------------------------------------------------------ how the aircraft flies

/** Usual static margins, as fractions of the mean chord. */
export const MARGIN = { low: 0.05, high: 0.2, target: 0.15 };

/** How to read a static margin (fraction of the mean aerodynamic chord
 * `mac`, m): the reading and how far to move the centre of gravity (m) to
 * reach a usual margin — forward for a small one, back for a large one. */
export function readMargin(margin: number, mac: number): Reading & { move: number | null } {
  if (margin < 0) return { level: "danger", text: "flies.margin.unstable", move: (MARGIN.low - margin) * mac };
  if (margin < MARGIN.low) return { level: "warning", text: "flies.margin.low", move: (MARGIN.low - margin) * mac };
  if (margin <= MARGIN.high) return { level: "ok", text: "flies.margin.good", move: null };
  return { level: "info", text: "flies.margin.high", move: (margin - MARGIN.target) * mac };
}

/** Where the stall starts: canard first (safe), near the root (gentle),
 * halfway, or at the tips (a wing may drop). */
export function readStallStart(stall: StallResult): Reading {
  if (stall.part === "canard") return { level: "ok", text: "flies.stall.canard" };
  const f = stall.span_fraction ?? 0;
  if (f < 0.35) return { level: "ok", text: "flies.stall.root" };
  if (f < 0.7) return { level: "info", text: "flies.stall.middle" };
  return { level: "warning", text: "flies.stall.tip" };
}

/** The parts of the drag, as fractions of the total. */
export function dragShares(d: DragBreakdown): { key: MessageKey; share: number }[] {
  const total = d.total ?? 0;
  const parts: [MessageKey, number | null][] = [
    ["flies.drag.induced", d.induced],
    ["flies.drag.wing", d.wing_profile],
    ["flies.drag.tail", d.tail_profile],
    ["flies.drag.fuselage", d.fuselage],
  ];
  return parts.map(([key, v]) => ({ key, share: total > 0 ? (v ?? 0) / total : 0 }));
}
