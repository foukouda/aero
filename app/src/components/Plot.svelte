<script lang="ts" module>
  export interface PlotCurve {
    label: string;
    color: string;
    x: number[];
    y: (number | null)[];
    dash?: number[];
    /** Show point markers. */
    points?: boolean;
    /** Draw the connecting line (default true). */
    line?: boolean;
    /** Hide from the legend (e.g. second surface of the same airfoil). */
    hideInLegend?: boolean;
    width?: number;
  }
</script>

<script lang="ts">
  import uPlot from "uplot";
  import { onDestroy, untrack } from "svelte";
  import { align, equalAxes, extent } from "../lib/plot/scales";
  import { renderSvg } from "../lib/plot/svg";

  interface Props {
    curves: PlotCurve[];
    xLabel: string;
    yLabel: string;
    title?: string;
    /** Same scale on both axes (airfoil shapes). */
    equal?: boolean;
    /** y increasing downwards (Cp plots: -Cp up). */
    yInverted?: boolean;
    /** Redraw key: changing it (e.g. the theme) rebuilds the chart. */
    styleKey?: string;
    emptyMessage?: string;
  }

  let {
    curves,
    xLabel,
    yLabel,
    title = "",
    equal = false,
    yInverted = false,
    styleKey = "",
    emptyMessage = "No data",
  }: Props = $props();

  let container: HTMLDivElement | undefined = $state();
  let chart: uPlot | null = null;
  let width = $state(0);
  let height = $state(0);
  let structure = "";

  const hasData = $derived(curves.some((c) => c.x.length > 0));

  function cssVar(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  function colors() {
    return {
      background: cssVar("--plot-bg") || "#ffffff",
      text: cssVar("--text") || "#111827",
      grid: cssVar("--grid") || "#e5e7eb",
      axis: cssVar("--axis") || "#6b7280",
    };
  }

  function currentExtent() {
    const e = extent(curves);
    return Number.isFinite(e.xmin) ? e : { xmin: 0, xmax: 1, ymin: -0.1, ymax: 0.1 };
  }

  function options(w: number, h: number): uPlot.Options {
    const c = colors();
    const axis = (label: string): uPlot.Axis => ({
      label,
      stroke: c.text,
      grid: { stroke: c.grid, width: 1 },
      ticks: { stroke: c.grid, width: 1 },
      // Locale-independent tick labels (dot decimal separator).
      values: (_u: uPlot, splits: number[]) => splits.map((v) => Number(v.toPrecision(10)).toString()),
    });
    const equalRange = (key: "x" | "y") => (u: uPlot) => {
      const e = currentExtent();
      const r = equalAxes(e.xmin, e.xmax, e.ymin, e.ymax, u.bbox.width, u.bbox.height);
      return r[key];
    };
    return {
      width: w,
      height: h,
      cursor: { drag: { x: !equal, y: false } },
      legend: { show: true, live: false },
      scales: {
        x: equal ? { time: false, auto: false, range: equalRange("x") } : { time: false },
        y: equal
          ? { auto: false, range: equalRange("y") }
          : { dir: yInverted ? -1 : 1 },
      },
      axes: [axis(xLabel), axis(yLabel)],
      series: [
        {},
        ...curves.map(
          (curve): uPlot.Series => ({
            label: curve.label,
            stroke: curve.color,
            width: curve.width ?? 1.6,
            dash: curve.dash,
            spanGaps: true,
            class: curve.hideInLegend ? "no-legend" : undefined,
            points: { show: curve.points ?? false, size: 4, fill: curve.color },
            ...(curve.line === false ? { paths: () => null } : {}),
          }),
        ),
      ],
    };
  }

  function data(): uPlot.AlignedData {
    return align(curves) as unknown as uPlot.AlignedData;
  }

  function signature(): string {
    return JSON.stringify([
      styleKey,
      title,
      xLabel,
      yLabel,
      equal,
      yInverted,
      curves.map((c) => [c.label, c.color, c.dash, c.points, c.line, c.hideInLegend]),
    ]);
  }

  /** Fits canvas + legend into the container. uPlot applies sizes
   * asynchronously, so the legend is measured directly. */
  function fit(w: number, h: number) {
    if (!chart || w < 10 || h < 10) return;
    const legend = chart.root.querySelector<HTMLElement>(".u-legend");
    const extra = (legend?.offsetHeight ?? 0) + 4;
    chart.setSize({ width: w, height: Math.max(h - extra, 60) });
    // Second pass once laid out: match the real total height to the container.
    requestAnimationFrame(() => {
      if (!chart) return;
      const over = chart.root.offsetHeight - h;
      if (Math.abs(over) > 2) chart.setSize({ width: w, height: Math.max(chart.height - over, 60) });
    });
  }

  function rebuild() {
    chart?.destroy();
    chart = null;
    if (!container || width < 10 || height < 10 || !hasData) return;
    chart = new uPlot(options(width, height), data(), container);
    structure = signature();
    fit(width, height);
  }

  // Data or structure changes.
  $effect(() => {
    const sig = signature();
    const d = data();
    untrack(() => {
      if (!chart || sig !== structure) {
        rebuild();
      } else {
        chart.setData(d, true);
      }
    });
  });

  // Size changes.
  $effect(() => {
    if (!container) return;
    const observer = new ResizeObserver((entries) => {
      const box = entries[0]?.contentRect;
      if (!box) return;
      width = Math.floor(box.width);
      height = Math.floor(box.height);
    });
    observer.observe(container);
    return () => observer.disconnect();
  });

  $effect(() => {
    const w = width;
    const h = height;
    untrack(() => {
      if (!chart) rebuild();
      else fit(w, h);
    });
  });

  onDestroy(() => chart?.destroy());

  /** PNG image of the chart (canvas, title and legend). */
  export async function png(): Promise<Uint8Array> {
    if (!chart) throw new Error("nothing to export");
    const source = chart.ctx.canvas;
    const ratio = source.width / Math.max(chart.width, 1);
    const legendItems = curves.filter((c) => !c.hideInLegend);
    const legendHeight = 28 * ratio;
    const titleHeight = title ? 30 * ratio : 0;
    const canvas = document.createElement("canvas");
    canvas.width = source.width;
    canvas.height = titleHeight + source.height + legendHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("canvas not available");
    const c = colors();
    ctx.fillStyle = c.background;
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    if (title) {
      ctx.fillStyle = c.text;
      ctx.font = `600 ${14 * ratio}px sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(title, canvas.width / 2, titleHeight / 2);
      ctx.textAlign = "left";
    }
    ctx.drawImage(source, 0, titleHeight);
    ctx.font = `${12 * ratio}px sans-serif`;
    ctx.textBaseline = "middle";
    let x = 12 * ratio;
    const y = titleHeight + source.height + legendHeight / 2;
    for (const item of legendItems) {
      ctx.strokeStyle = item.color;
      ctx.lineWidth = 2 * ratio;
      ctx.beginPath();
      ctx.moveTo(x, y);
      ctx.lineTo(x + 18 * ratio, y);
      ctx.stroke();
      ctx.fillStyle = c.text;
      ctx.fillText(item.label, x + 24 * ratio, y);
      x += 40 * ratio + ctx.measureText(item.label).width;
    }
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
    if (!blob) throw new Error("PNG encoding failed");
    return new Uint8Array(await blob.arrayBuffer());
  }

  /** SVG rendering of the chart with its current scales. */
  export async function svg(): Promise<Uint8Array> {
    if (!chart) throw new Error("nothing to export");
    const sx = chart.scales.x;
    const sy = chart.scales.y;
    const text = renderSvg({
      width: Math.max(chart.width, 400),
      height: Math.max(chart.height, 300),
      title: title || undefined,
      xLabel,
      yLabel,
      xRange: [sx.min ?? 0, sx.max ?? 1],
      yRange: [sy.min ?? 0, sy.max ?? 1],
      yInverted,
      series: curves.map((c) => ({
        label: c.hideInLegend ? "" : c.label,
        color: c.color,
        dash: c.dash,
        points: c.points,
        line: c.line,
        x: c.x,
        y: c.y,
      })),
      colors: colors(),
    });
    return new TextEncoder().encode(text);
  }
</script>

<div class="plot" bind:this={container}>
  {#if !hasData}
    <p class="empty">{emptyMessage}</p>
  {/if}
</div>

<style>
  .plot {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 160px;
    overflow: hidden;
  }
  .empty {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    margin: 0;
    color: var(--muted);
  }
  .plot :global(.u-legend .no-legend) {
    display: none;
  }
</style>
