<script lang="ts">
  import Plot, { type PlotCurve } from "./Plot.svelte";
  import { app } from "../lib/state.svelte";

  interface Props {
    title: string;
    fileName: string;
    curves: PlotCurve[];
    xLabel: string;
    yLabel: string;
    equal?: boolean;
    yInverted?: boolean;
    emptyMessage?: string;
    /** Anchor for the guided tour. */
    tour?: string;
    /** Freehand drawing instead of zooming (see Plot). */
    draw?: boolean;
    onDraw?: (points: [number, number][]) => void;
  }

  let { title, fileName, curves, xLabel, yLabel, equal, yInverted, emptyMessage, tour, draw, onDraw }: Props =
    $props();

  let plot: Plot | undefined = $state();
  const hasData = $derived(curves.some((c) => c.x.length > 0));
</script>

<section class="panel plot-panel" data-tour={tour}>
  <header>
    <h2>{title}</h2>
    <div class="actions">
      <button
        disabled={!hasData}
        title="Export as PNG"
        onclick={() => plot && app.savePlot(fileName, "png", () => plot!.png())}>PNG</button
      >
      <button
        disabled={!hasData}
        title="Export as SVG"
        onclick={() => plot && app.savePlot(fileName, "svg", () => plot!.svg())}>SVG</button
      >
    </div>
  </header>
  <div class="body">
    <Plot
      bind:this={plot}
      {curves}
      {xLabel}
      {yLabel}
      {equal}
      {yInverted}
      {emptyMessage}
      {draw}
      {onDraw}
      title={title}
      styleKey={app.theme}
    />
  </div>
</section>

<style>
  .plot-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
  }
  .actions {
    display: flex;
    gap: 4px;
  }
  .actions button {
    font-size: 11px;
    padding: 2px 8px;
  }
  .body {
    flex: 1;
    min-height: 0;
    padding: 6px 8px 4px;
  }
</style>
