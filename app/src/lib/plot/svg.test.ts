import { describe, expect, it } from "vitest";
import { renderSvg, type SvgPlot } from "./svg";

const base: SvgPlot = {
  width: 400,
  height: 300,
  title: "Cl <alpha>",
  xLabel: "alpha (deg)",
  yLabel: "Cl",
  xRange: [0, 10],
  yRange: [0, 1],
  series: [
    { label: "NACA 0012", color: "#2563eb", x: [0, 5, 10], y: [0, 0.5, 1] },
    { label: "gappy", color: "#dc2626", dash: [4, 2], points: true, x: [0, 5, 10], y: [0.1, null, 0.9] },
  ],
  colors: { background: "#fff", text: "#111", grid: "#ddd", axis: "#333" },
};

describe("renderSvg", () => {
  it("produces a standalone SVG document with escaped text", () => {
    const svg = renderSvg(base);
    expect(svg.startsWith("<svg xmlns=\"http://www.w3.org/2000/svg\"")).toBe(true);
    expect(svg.trim().endsWith("</svg>")).toBe(true);
    expect(svg).toContain("Cl &lt;alpha&gt;");
    expect(svg).toContain("NACA 0012");
  });

  it("maps data to the plot area and breaks lines at gaps", () => {
    const svg = renderSvg(base);
    // left = 64, plot width = 400 - 64 - 16 = 320; x = 0 -> 64, x = 10 -> 384.
    expect(svg).toContain('d="M64 ');
    expect(svg).toMatch(/L384 /);
    // The second series has two separate moves (gap in the middle).
    const second = svg.split("\n").find((l) => l.includes('stroke="#dc2626"') && l.startsWith("<path"));
    expect(second?.match(/M/g)?.length).toBe(2);
    expect(svg).toContain('stroke-dasharray="4 2"');
    expect(svg).toContain("<circle");
  });

  it("inverts the y axis on request", () => {
    const normal = renderSvg({ ...base, series: [base.series[0]] });
    const inverted = renderSvg({ ...base, yInverted: true, series: [base.series[0]] });
    const firstY = (s: string) => Number(/d="M64 ([\d.]+)/.exec(s)?.[1]);
    expect(firstY(inverted)).toBeLessThan(firstY(normal));
  });
});
