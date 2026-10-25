// SVG rendering of a 2D line plot from its data and current scales.
// uPlot draws on a canvas and cannot export vector graphics, so plots are
// re-rendered here for SVG export.

import { niceTicks, type Range } from "./scales";

export interface SvgSeries {
  /** Legend label; series with an empty label are not listed. */
  label: string;
  color: string;
  /** Dash pattern in pixels, e.g. [6, 4]; solid if absent. */
  dash?: number[];
  /** Draw markers at the points. */
  points?: boolean;
  /** Draw the connecting line (default true). */
  line?: boolean;
  x: (number | null)[];
  y: (number | null)[];
}

export interface SvgPlot {
  width: number;
  height: number;
  title?: string;
  xLabel: string;
  yLabel: string;
  xRange: Range;
  yRange: Range;
  /** Draw y increasing downwards (aerodynamic -Cp up convention). */
  yInverted?: boolean;
  series: SvgSeries[];
  colors: { background: string; text: string; grid: string; axis: string };
}

function esc(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function num(v: number): string {
  return Number(v.toFixed(2)).toString();
}

function label(v: number): string {
  return Number(v.toPrecision(6)).toString();
}

/** Renders the plot as a standalone SVG document. */
export function renderSvg(plot: SvgPlot): string {
  const { width, height, colors } = plot;
  const left = 64;
  const right = 16;
  const top = plot.title ? 36 : 16;
  const legendHeight = 22;
  const bottom = 48 + legendHeight;
  const pw = Math.max(width - left - right, 1);
  const ph = Math.max(height - top - bottom, 1);
  const [x0, x1] = plot.xRange;
  const [y0, y1] = plot.yRange;
  const sx = (x: number) => left + ((x - x0) / (x1 - x0)) * pw;
  const sy = (y: number) => {
    const t = (y - y0) / (y1 - y0);
    return plot.yInverted ? top + t * ph : top + (1 - t) * ph;
  };

  const out: string[] = [];
  out.push(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" font-family="Helvetica, Arial, sans-serif" font-size="12">`,
  );
  out.push(`<rect width="100%" height="100%" fill="${esc(colors.background)}"/>`);
  if (plot.title) {
    out.push(
      `<text x="${num(width / 2)}" y="22" text-anchor="middle" font-size="14" font-weight="600" fill="${esc(colors.text)}">${esc(plot.title)}</text>`,
    );
  }

  // Grid and tick labels.
  out.push(`<g stroke="${esc(colors.grid)}" stroke-width="1">`);
  const xt = niceTicks(x0, x1, Math.max(2, Math.round(pw / 90)));
  const yt = niceTicks(Math.min(y0, y1), Math.max(y0, y1), Math.max(2, Math.round(ph / 60)));
  for (const t of xt) out.push(`<line x1="${num(sx(t))}" y1="${top}" x2="${num(sx(t))}" y2="${num(top + ph)}"/>`);
  for (const t of yt) out.push(`<line x1="${left}" y1="${num(sy(t))}" x2="${num(left + pw)}" y2="${num(sy(t))}"/>`);
  out.push(`</g>`);
  out.push(`<g fill="${esc(colors.text)}">`);
  for (const t of xt) {
    out.push(`<text x="${num(sx(t))}" y="${num(top + ph + 16)}" text-anchor="middle">${label(t)}</text>`);
  }
  for (const t of yt) {
    out.push(`<text x="${left - 6}" y="${num(sy(t) + 4)}" text-anchor="end">${label(t)}</text>`);
  }
  out.push(
    `<text x="${num(left + pw / 2)}" y="${num(top + ph + 34)}" text-anchor="middle">${esc(plot.xLabel)}</text>`,
  );
  out.push(
    `<text transform="translate(16 ${num(top + ph / 2)}) rotate(-90)" text-anchor="middle">${esc(plot.yLabel)}</text>`,
  );
  out.push(`</g>`);
  out.push(
    `<rect x="${left}" y="${top}" width="${num(pw)}" height="${num(ph)}" fill="none" stroke="${esc(colors.axis)}"/>`,
  );

  // Series, clipped to the plot area; null values break the line.
  out.push(`<defs><clipPath id="plot-area"><rect x="${left}" y="${top}" width="${num(pw)}" height="${num(ph)}"/></clipPath></defs>`);
  out.push(`<g clip-path="url(#plot-area)" fill="none" stroke-width="1.6" stroke-linejoin="round">`);
  for (const s of plot.series) {
    const dash = s.dash && s.dash.length ? ` stroke-dasharray="${s.dash.join(" ")}"` : "";
    let d = "";
    let pen = false;
    const markers: string[] = [];
    for (let i = 0; i < s.x.length; i++) {
      const x = s.x[i];
      const y = s.y[i];
      if (x === null || y === null || !Number.isFinite(x) || !Number.isFinite(y)) {
        pen = false;
        continue;
      }
      d += `${pen ? "L" : "M"}${num(sx(x))} ${num(sy(y))}`;
      pen = true;
      if (s.points) markers.push(`<circle cx="${num(sx(x))}" cy="${num(sy(y))}" r="2"/>`);
    }
    if (d && s.line !== false) out.push(`<path d="${d}" stroke="${esc(s.color)}"${dash}/>`);
    if (markers.length) out.push(`<g fill="${esc(s.color)}" stroke="none">${markers.join("")}</g>`);
  }
  out.push(`</g>`);

  // Legend.
  let lx = left;
  const ly = height - 10;
  out.push(`<g fill="${esc(colors.text)}">`);
  for (const s of plot.series) {
    if (!s.label) continue;
    out.push(`<line x1="${lx}" y1="${ly - 4}" x2="${lx + 18}" y2="${ly - 4}" stroke="${esc(s.color)}" stroke-width="2"/>`);
    out.push(`<text x="${lx + 22}" y="${ly}">${esc(s.label)}</text>`);
    lx += 34 + 7 * s.label.length;
  }
  out.push(`</g>`);
  out.push(`</svg>`);
  return out.join("\n");
}
