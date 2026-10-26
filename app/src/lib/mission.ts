// "My aircraft" tab: example flights, how to read the flight's numbers, and
// the order of the recommended airfoils. Pure functions, tested in
// mission.test.ts; the physics is in the Rust crate aero-mission.

import type { CandidateResult, Conditions, Goal, MissionSetup, RecommendResult } from "./bindings";
import type { Lang, MessageKey } from "./i18n";

/** Standard gravity, m/s². */
export const G0 = 9.80665;

export type Level = "ok" | "info" | "warning" | "danger";

export interface Reading {
  level: Level;
  text: MessageKey;
}

export interface Preset {
  id: string;
  label: MessageKey;
  speedKmh: number;
  altitude: number;
  mass: number;
  span: number;
  chord: number;
}

/** Example flights; the wing loadings and lift coefficients are typical of
 * each kind of aircraft (checked in mission.test.ts). */
export const PRESETS: Preset[] = [
  { id: "parkflyer", label: "preset.parkflyer", speedKmh: 22, altitude: 50, mass: 0.25, span: 0.9, chord: 0.18 },
  { id: "glider", label: "preset.glider", speedKmh: 30, altitude: 100, mass: 0.6, span: 1.5, chord: 0.17 },
  { id: "trainer", label: "preset.trainer", speedKmh: 40, altitude: 100, mass: 0.9, span: 1.2, chord: 0.22 },
  { id: "drone", label: "preset.drone", speedKmh: 60, altitude: 120, mass: 2, span: 1.8, chord: 0.25 },
  // Cessna 172: 1111 kg, 11.0 m span, 16.2 m² wing, cruise 226 km/h at 2400 m.
  { id: "cessna", label: "preset.cessna", speedKmh: 226, altitude: 2400, mass: 1111, span: 11.0, chord: 1.47 },
];

/** The setup with the preset's flight and wing (the wing is given). */
export function applyPreset(setup: MissionSetup, preset: Preset): MissionSetup {
  return {
    ...setup,
    flight: {
      ...setup.flight,
      speed: preset.speedKmh / 3.6,
      altitude: preset.altitude,
      mass: preset.mass,
    },
    wing: { span: preset.span, chord: preset.chord },
    sizing_cl: null,
  };
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
