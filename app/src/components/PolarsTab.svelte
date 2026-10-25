<script lang="ts">
  import type { PlotCurve } from "./Plot.svelte";
  import PlotPanel from "./PlotPanel.svelte";
  import type { AlphaSweep, PolarPoint } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import { numbers } from "../lib/format";

  const s = $derived(app.settings);
  const sweep = $derived(s?.sweep);
  const running = $derived(app.sweep !== null);

  /** Points to plot per airfoil: live points during a sweep, else stored. */
  const polars = $derived.by(() => {
    const out: { name: string; color: string; points: PolarPoint[] }[] = [];
    for (const a of app.visible) {
      const live = app.sweep?.live[a.id];
      const stored = app.session?.polars.find((p) => p.airfoil_id === a.id);
      const points = live ?? stored?.polar.points ?? [];
      if (points.length) out.push({ name: a.name, color: a.color, points });
    }
    return out;
  });

  function curvesOf(pick: (p: PolarPoint) => number | null): PlotCurve[] {
    return polars.map((p) => {
      const sorted = [...p.points].sort((a, b) => (a.alpha_deg ?? 0) - (b.alpha_deg ?? 0));
      return {
        label: p.name,
        color: p.color,
        x: numbers(sorted.map((q) => q.alpha_deg)),
        y: sorted.map(pick),
        // Markers only while they stay readable.
        points: sorted.length <= 200,
      };
    });
  }

  const clCurves = $derived(curvesOf((p) => p.cl));
  const cmCurves = $derived(curvesOf((p) => p.cm));

  function updateSweep(patch: Partial<AlphaSweep>) {
    if (!sweep) return;
    app.updateSettings({ sweep: { ...$state.snapshot(sweep), ...patch } as AlphaSweep });
  }

  function value(e: Event): number {
    return Number((e.currentTarget as HTMLInputElement).value);
  }

  const progress = $derived(
    app.sweep && app.sweep.total > 0 ? Math.round((100 * app.sweep.done) / app.sweep.total) : 0,
  );
  const mach = $derived(s?.mach ?? 0);
</script>

<div class="polars">
  <section class="panel controls">
    <label>α start <input type="number" step="0.5" value={sweep?.start_deg} disabled={running} onchange={(e) => updateSweep({ start_deg: value(e) })} /></label>
    <label>α end <input type="number" step="0.5" value={sweep?.end_deg} disabled={running} onchange={(e) => updateSweep({ end_deg: value(e) })} /></label>
    <label>step <input type="number" min="0.01" step="0.25" value={sweep?.step_deg} disabled={running} onchange={(e) => updateSweep({ step_deg: value(e) })} /></label>
    <label>Mach <input type="number" min="0" max="0.99" step="0.05" value={mach} disabled={running} onchange={(e) => app.updateSettings({ mach: Math.max(0, Math.min(0.99, value(e))) })} /></label>
    {#if running}
      <button class="danger" onclick={() => app.cancelSweep()}>Cancel</button>
      <div class="progress" role="progressbar" aria-valuenow={progress} aria-valuemin={0} aria-valuemax={100}>
        <div class="bar" style:width={`${progress}%`}></div>
      </div>
      <span class="mono muted">{app.sweep?.done} / {app.sweep?.total}</span>
    {:else}
      <button class="primary" onclick={() => app.runSweep()} disabled={app.visible.length === 0}>
        Run sweep
      </button>
      <button onclick={() => app.exportPolarsCsv()} disabled={!app.session?.polars.length}>Export CSV</button>
      <span class="muted">Visible airfoils are analyzed; results are kept in the project.</span>
    {/if}
  </section>

  <div class="plots">
    <PlotPanel
      title={`Lift coefficient${mach > 0 ? ` (M = ${mach.toFixed(2)})` : ""}`}
      fileName="cl_alpha"
      curves={clCurves}
      xLabel="α (deg)"
      yLabel="Cl"
      emptyMessage="Run a sweep to plot Cl(α)"
    />
    <PlotPanel
      title="Moment coefficient (c/4)"
      fileName="cm_alpha"
      curves={cmCurves}
      xLabel="α (deg)"
      yLabel="Cm"
      emptyMessage="Run a sweep to plot Cm(α)"
    />
  </div>
</div>

<style>
  .polars {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    flex-wrap: wrap;
  }
  label {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
  }
  label input {
    width: 70px;
  }
  .progress {
    width: 220px;
    height: 8px;
    border-radius: 4px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .bar {
    height: 100%;
    background: var(--accent);
    transition: width 0.1s;
  }
  .plots {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    min-height: 0;
  }
</style>
