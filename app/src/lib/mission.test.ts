import { describe, expect, it } from "vitest";
import type { Aircraft, CandidateResult, Fit, RecommendResult } from "./bindings";
import { MESSAGES, placeholders, translate } from "./i18n";
import {
  G0,
  PRESETS,
  applyPreset,
  best,
  length,
  mass,
  num,
  power,
  dragShares,
  MARGIN,
  readCl,
  readFinVolume,
  readMargin,
  readStallStart,
  readReynolds,
  readTailVolume,
  readWingLoading,
  ranked,
  sig,
  speedKmh,
} from "./mission";

/** ISA troposphere density and kinematic viscosity (test-only). */
function air(h: number): { rho: number; nu: number } {
  const t = 288.15 - 0.0065 * h;
  const p = 101325 * (t / 288.15) ** 5.25588;
  const rho = p / (287.05287 * t);
  const mu = (1.458e-6 * t ** 1.5) / (t + 110.4);
  return { rho, nu: mu / rho };
}

describe("example flights", () => {
  it("are typical of their kind of aircraft", () => {
    const expected: Record<string, string> = {
      trainer: "loading.medium",
      glider: "loading.light",
      flying_wing: "loading.light",
      light_aircraft: "loading.fullSize",
    };
    for (const p of PRESETS) {
      const { rho } = air(p.altitude);
      const v = p.speedKmh / 3.6;
      const cl = (p.mass * G0) / (0.5 * rho * v * v * p.wingArea);
      expect(readCl(cl).level, p.aircraft).toBe("ok");
      expect(readWingLoading((p.mass / p.wingArea) * 10), p.aircraft).toBe(expected[p.aircraft]);
    }
  });

  it("replace the flight and the aircraft", () => {
    const setup = {
      flight: { speed: 1, altitude: 0, temperature_offset: 5, mass: 1 },
      aircraft: {} as Aircraft,
      sizing_cl: 0.6,
      goal: "slow_flight" as const,
      ncrit: 7,
      model: { span_efficiency: 0.9, cl_max_factor: 0.9 },
    };
    const glider = { cg_mac_fraction: 0.33 } as Aircraft;
    const s = applyPreset(setup, PRESETS[1], glider);
    expect(s.flight.speed).toBeCloseTo(30 / 3.6, 12);
    expect(s.flight.temperature_offset).toBe(5);
    expect(s.aircraft).toBe(glider);
    expect(s.sizing_cl).toBeNull();
    expect(s.goal).toBe("slow_flight");
    expect(s.ncrit).toBe(7);
  });
});

describe("readings", () => {
  it("grade the required lift coefficient", () => {
    expect(readCl(0.1).level).toBe("info");
    expect(readCl(0.6).level).toBe("ok");
    expect(readCl(0.95).level).toBe("warning");
    expect(readCl(1.3).level).toBe("danger");
  });

  it("grade the tail sizes", () => {
    expect(readTailVolume(null).text).toBe("tail.none");
    expect(readTailVolume(0.2).level).toBe("warning");
    expect(readTailVolume(0.55).text).toBe("tail.ok");
    expect(readTailVolume(1.0).text).toBe("tail.large");
    expect(readFinVolume(0.01).level).toBe("warning");
    expect(readFinVolume(0.005, true).text).toBe("fin.flyingWing");
    expect(readFinVolume(0.035).text).toBe("fin.ok");
    expect(readFinVolume(undefined).text).toBe("fin.none");
  });

  it("grade the Reynolds number", () => {
    expect(readReynolds(3e4).text).toBe("card.re.veryLow");
    expect(readReynolds(7e4).level).toBe("warning");
    expect(readReynolds(2e5).text).toBe("card.re.model");
    expect(readReynolds(1e6).text).toBe("card.re.large");
    expect(readReynolds(6e6).text).toBe("card.re.full");
  });
});

function candidate(name: string, fit: Fit | null, easy = true): CandidateResult {
  return {
    source: { kind: "catalogue", id: name },
    name,
    outline_x: [],
    outline_y: [],
    evaluation: fit
      ? {
          polar: [],
          cl_max: null,
          operating_point: null,
          performance: null,
          fit,
          buildability: { thickness: 0.1, undercamber: 0, easy },
        }
      : null,
    error: fit ? null : "failed",
  };
}

describe("recommendations", () => {
  const result = {
    candidates: [
      candidate("a", "comfortable"),
      candidate("b", "near_stall"),
      candidate("c", "tight", false),
      candidate("d", null),
    ],
    rankings: { least_drag: [2, 0, 1, 3], slow_flight: [0, 2, 1, 3], easy_to_build: [0, 2, 1, 3] },
  } as unknown as RecommendResult;

  it("follow the ranking of the goal", () => {
    expect(ranked(result, "least_drag").map((c) => c.name)).toEqual(["c", "a", "b", "d"]);
  });

  it("only recommend airfoils with a margin, easy to build when asked", () => {
    expect(best(result, "least_drag").map((c) => c.name)).toEqual(["c", "a"]);
    expect(best(result, "easy_to_build").map((c) => c.name)).toEqual(["a"]);
    expect(best(result, "slow_flight", 1).map((c) => c.name)).toEqual(["a"]);
  });
});

describe("formatting", () => {
  it("uses the language's number format", () => {
    expect(num(0.5, "en")).toBe("0.50");
    expect(num(0.5, "fr")).toBe("0,50");
    expect(num(null, "fr")).toBe("—");
    expect(sig(75471, "en")).toBe("75,500");
    expect(sig(75471, "fr").replace(/\s/g, " ")).toBe("75 500");
  });

  it("picks readable units", () => {
    expect(length(0.2, "en")).toBe("20 cm");
    expect(length(1.5, "en")).toBe("1.5 m");
    expect(speedKmh(20 / 3.6, "en")).toBe("20 km/h");
    expect(mass(0.3, "en")).toBe("300 g");
    expect(mass(1111, "en")).toBe("1,111 kg");
    expect(power(12.3, "en")).toBe("12.3 W");
    expect(power(45000, "en")).toBe("45 kW");
  });
});

describe("interface texts", () => {
  it("exist in both languages with the same placeholders", () => {
    const keys = Object.keys(MESSAGES.en);
    expect(Object.keys(MESSAGES.fr).sort()).toEqual([...keys].sort());
    for (const key of keys as (keyof typeof MESSAGES.en)[]) {
      expect(MESSAGES.fr[key].trim(), key).not.toBe("");
      expect(placeholders(MESSAGES.fr[key]), key).toEqual(placeholders(MESSAGES.en[key]));
    }
  });

  it("fill their placeholders", () => {
    expect(translate("en", "mission.finding", { done: 3, total: 20 })).toBe("Analysing 3 of 20…");
    expect(translate("fr", "mission.finding", { done: 3 })).toBe("Analyse 3 sur {total}…");
  });
});

describe("how the aircraft flies", () => {
  it("reads the static margin and how far to move the centre of gravity", () => {
    const mac = 0.2;
    const unstable = readMargin(-0.05, mac);
    expect(unstable.level).toBe("danger");
    // From −5 % to 5 %: 10 % of a 20 cm chord forward.
    expect(unstable.move).toBeCloseTo(0.02, 12);
    expect(readMargin(0.03, mac)).toMatchObject({ level: "warning", text: "flies.margin.low" });
    expect(readMargin(0.03, mac).move).toBeCloseTo(0.004, 12);
    expect(readMargin(MARGIN.low, mac)).toMatchObject({ level: "ok", move: null });
    expect(readMargin(0.12, mac).text).toBe("flies.margin.good");
    const heavy = readMargin(0.35, mac);
    expect(heavy).toMatchObject({ level: "info", text: "flies.margin.high" });
    // Back to 15 %.
    expect(heavy.move).toBeCloseTo(0.04, 12);
  });

  it("says where the stall starts", () => {
    const at = (part: "wing" | "canard", f: number) =>
      readStallStart({ cl_max: 1.1, speed: 7, part, span_fraction: f }).text;
    expect(at("wing", 0.0)).toBe("flies.stall.root");
    expect(at("wing", 0.5)).toBe("flies.stall.middle");
    expect(at("wing", 0.9)).toBe("flies.stall.tip");
    expect(at("canard", 0.9)).toBe("flies.stall.canard");
    expect(readStallStart({ cl_max: 1, speed: 7, part: "wing", span_fraction: 0.95 }).level).toBe("warning");
  });

  it("shares the drag among its parts", () => {
    const shares = dragShares({
      induced: 0.008,
      wing_profile: 0.01,
      tail_profile: 0.002,
      fuselage: 0.0,
      total: 0.02,
      span_efficiency: 0.97,
      lift_to_drag: 15,
      drag: 0.5,
      power: 6,
    });
    [0.4, 0.5, 0.1, 0].forEach((v, i) => expect(shares[i]?.share).toBeCloseTo(v, 12));
    expect(shares.map((s) => s.key)).toEqual([
      "flies.drag.induced",
      "flies.drag.wing",
      "flies.drag.tail",
      "flies.drag.fuselage",
    ]);
  });
});
