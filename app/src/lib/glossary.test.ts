/// <reference types="node" />
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { CATEGORY_TITLES, GLOSSARY, glossaryEntry, glossaryMarkdown, searchGlossary } from "./glossary";

/** Glossary ids referenced by `<Term id="...">` in the Svelte sources. */
function termIds(): Set<string> {
  const sources = import.meta.glob("../**/*.svelte", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>;
  const found = new Set<string>();
  for (const text of Object.values(sources)) {
    for (const m of text.matchAll(/<Term id="([\w-]+)"/g)) found.add(m[1]);
  }
  return found;
}

function docPath(name: string): string {
  return fileURLToPath(new URL(`../../../docs/${name}`, import.meta.url));
}

describe("glossary", () => {
  it("has unique ids and complete texts in both languages", () => {
    const ids = GLOSSARY.map((e) => e.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const e of GLOSSARY) {
      expect(CATEGORY_TITLES[e.category], e.id).toBeDefined();
      for (const lang of ["en", "fr"] as const) {
        const t = e.text[lang];
        expect(t.term.length, `${e.id} ${lang}`).toBeGreaterThan(1);
        expect(t.short.length, `${e.id} ${lang}`).toBeGreaterThan(20);
        expect(t.long.length, `${e.id} ${lang}`).toBeGreaterThan(t.short.length);
      }
    }
  });

  it("explains every term used in the interface", () => {
    const used = termIds();
    expect(used.size).toBeGreaterThan(20);
    for (const id of used) expect(glossaryEntry(id), id).toBeDefined();
  });

  it("searches symbols and texts in the chosen language", () => {
    expect(searchGlossary("", "en")).toHaveLength(GLOSSARY.length);
    expect(searchGlossary("cd", "en").map((e) => e.id)).toContain("cd");
    expect(searchGlossary("traînée", "fr").map((e) => e.id)).toContain("cd");
    expect(searchGlossary("traînée", "en")).toHaveLength(0);
  });

  // `npm run glossary` rewrites the pages.
  it("is published in docs/ (glossary.md, glossaire.md)", () => {
    for (const [lang, file] of [
      ["en", "glossary.md"],
      ["fr", "glossaire.md"],
    ] as const) {
      const markdown = glossaryMarkdown(lang);
      if (process.env.AERO_UPDATE_GLOSSARY) writeFileSync(docPath(file), markdown);
      const current = readFileSync(docPath(file), "utf8").replace(/\r\n/g, "\n");
      expect(current, `docs/${file} is out of date: run npm run glossary in app/`).toBe(markdown);
    }
  });
});
