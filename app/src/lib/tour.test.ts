import { describe, expect, it } from "vitest";
import { TOUR_STEPS, defaultTourLang, placeCard } from "./tour";

/** All `data-tour="..."` anchors used in the Svelte sources. */
function tourTargets(): Set<string> {
  const sources = import.meta.glob("../**/*.svelte", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>;
  const found = new Set<string>();
  for (const text of Object.values(sources)) {
    for (const m of text.matchAll(/data-tour="([\w-]+)"|tour="([\w-]+)"/g)) {
      found.add(m[1] ?? m[2]);
    }
  }
  return found;
}

describe("guided tour", () => {
  it("has unique step ids and complete texts in both languages", () => {
    const ids = TOUR_STEPS.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const step of TOUR_STEPS) {
      for (const lang of ["en", "fr"] as const) {
        const t = step.text[lang];
        expect(t.title.length, `${step.id} ${lang} title`).toBeGreaterThan(0);
        expect(t.body.length, `${step.id} ${lang} body`).toBeGreaterThan(20);
        if (step.action) expect(t.action, `${step.id} ${lang} action label`).toBeTruthy();
      }
    }
  });

  it("only targets elements that exist in the interface", () => {
    const targets = tourTargets();
    for (const step of TOUR_STEPS) {
      if (step.target) expect(targets, step.id).toContain(step.target);
    }
  });

  it("starts with a welcome and ends with a conclusion", () => {
    expect(TOUR_STEPS[0].target).toBeUndefined();
    expect(TOUR_STEPS.at(-1)?.target).toBeUndefined();
  });

  it("picks French for French locales", () => {
    expect(defaultTourLang("fr-FR")).toBe("fr");
    expect(defaultTourLang("en-US")).toBe("en");
    expect(defaultTourLang(undefined)).toBe("en");
  });
});

describe("placeCard", () => {
  const view = { width: 1400, height: 900 };
  const card = { width: 360, height: 220 };

  it("centers the card without a target", () => {
    expect(placeCard(null, card, view)).toEqual({ left: 520, top: 340 });
  });

  it("puts the card beside a small target", () => {
    const p = placeCard({ left: 20, top: 80, width: 280, height: 120 }, card, view);
    expect(p).toEqual({ left: 314, top: 80 });
    const q = placeCard({ left: 1100, top: 80, width: 280, height: 120 }, card, view);
    expect(q.left).toBe(1100 - 14 - 360);
  });

  it("stays inside the viewport for a target filling the window", () => {
    const p = placeCard({ left: 0, top: 0, width: 1400, height: 900 }, card, view);
    expect(p.left).toBeGreaterThanOrEqual(12);
    expect(p.left + card.width).toBeLessThanOrEqual(1400 - 12);
    expect(p.top + card.height).toBeLessThanOrEqual(900 - 12);
  });
});
