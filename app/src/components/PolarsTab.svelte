<script lang="ts">
  import Term from "./Term.svelte";
  import type { PlotCurve } from "./Plot.svelte";
  import PlotPanel from "./PlotPanel.svelte";
  import ViscousControls from "./ViscousControls.svelte";
  import ComparisonTable from "./ComparisonTable.svelte";
  import type { AlphaSweep, ClSweep, PolarPoint, ViscousPolarPoint } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import { compactExp, numbers } from "../lib/format";

  const s = $derived(app.settings);
  const sweep = $derived(s?.sweep);
  const clSweep = $derived(s?.cl_sweep ?? { start: 0, end: 1.2, step: 0.1 });
  const mode = $derived(s?.mode ?? "alpha");
  const running = $derived(app.sweep !== null);
  let view = $state<"plots" | "table">("plots");

  /** A point of either kind of polar, with the viscous columns optional. */
  interface Point {
    alpha_deg: number | null;
    cl: number | null;
    cm: number | null;
    cd?: number | null;
    xtr_top?: number | null;
    xtr_bottom?: number | null;
    hinge_moment?: number | null;
  }

  interface Shown {
    name: string;
    color: string;
    viscous: boolean;
    /** Polar description for the titles. */
    tag: string;
    points: Point[];
  }

  /** Points to plot per airfoil: live points during a sweep, else stored. */
  const polars = $derived.by(() => {
    const out: Shown[] = [];
    for (const a of app.visible) {
      const run = app.sweep;
      let points: Point[] | undefined;
      let viscous = false;
      let tag = "";
      if (run) {
        viscous = run.viscous;
        points = viscous
          ? (run.liveViscous[a.id] as ViscousPolarPoint[] | undefined)
          : (run.live[a.id] as PolarPoint[] | undefined);
      }
      if (!points) {
        const stored = app.session?.polars.find((p) => p.airfoil_id === a.id)?.polar;
        if (stored?.kind === "viscous") {
          viscous = true;
          points = stored.points;
          tag = `Re ${compactExp(stored.options.reynolds)}`;
        } else if (stored) {
          viscous = false;
          points = stored.points;
          tag = "inviscid";
        }
      }
      if (points?.length) out.push({ name: a.name, color: a.color, viscous, tag, points });
    }
    return out;
  });

  function curvesOf(
    pick: (p: Point) => number | null | undefined,
    x: (p: Point) => number | null | undefined = (p) => p.alpha_deg,
    onlyViscous = false,
  ): PlotCurve[] {
    return polars
      .filter((p) => !onlyViscous || p.viscous)
      .map((p) => {
        const sorted = [...p.points].sort((a, b) => (a.alpha_deg ?? 0) - (b.alpha_deg ?? 0));
        return {
          label: p.tag ? `${p.name} (${p.tag})` : p.name,
          color: p.color,
          x: numbers(sorted.map((q) => x(q) ?? null)),
          y: sorted.map((q) => pick(q) ?? null),
          // Markers only while they stay readable.
          points: sorted.length <= 200,
          dash: p.viscous ? undefined : [6, 4],
        };
      });
  }

  const clCurves = $derived(curvesOf((p) => p.cl));
  const cmCurves = $derived.by(() => {
    const cm = curvesOf((p) => p.cm);
    // Flap hinge moments, when computed, on the same plot.
    const hinge = curvesOf((p) => p.hinge_moment)
      .filter((c) => c.y.some((v) => v !== null))
      .map((c) => ({ ...c, label: `${c.label} Ch`, dash: [2, 3] }));
    return [...cm, ...hinge];
  });
  const hasHinge = $derived(cmCurves.some((c) => c.label.endsWith(" Ch")));
  const dragCurves = $derived(curvesOf((p) => p.cl, (p) => p.cd, true));
  const transitionCurves = $derived.by(() => {
    const top = curvesOf((p) => p.xtr_top, undefined, true);
    const bottom = curvesOf((p) => p.xtr_bottom, undefined, true).map((c) => ({
      ...c,
      label: `${c.label} bottom`,
      dash: [6, 4],
      hideInLegend: true,
    }));
    return [...top, ...bottom];
  });

  function updateSweep(patch: Partial<AlphaSweep>) {
    if (!sweep) return;
    app.updateSettings({ sweep: { ...$state.snapshot(sweep), ...patch } as AlphaSweep });
  }

  function updateClSweep(patch: Partial<ClSweep>) {
    app.updateSettings({ cl_sweep: { ...$state.snapshot(clSweep), ...patch } as ClSweep });
  }

  function value(e: Event): number {
    return Number((e.currentTarget as HTMLInputElement).value);
  }

  const progress = $derived(
    app.sweep && app.sweep.total > 0 ? Math.round((100 * app.sweep.done) / app.sweep.total) : 0,
  );
  const mach = $derived(s?.mach ?? 0);
  const machTag = $derived(mach > 0 ? ` (M = ${mach.toFixed(2)})` : "");
</script>

<div class="polars">
  <section class="panel controls" data-tour="sweep">
    <div class="row">
      <div class="segmented" role="group" aria-label="Swept quantity">
        <button class:active={mode === "alpha"} disabled={running} title="Sweep the angle of attack (XFOIL ASEQ)" onclick={() => app.updateSettings({ mode: "alpha" })}>α</button>
        <button class:active={mode === "cl"} disabled={running} title="Sweep the lift coefficient (XFOIL CSEQ)" onclick={() => app.updateSettings({ mode: "cl" })}>Cl</button>
      </div>
      {#if mode === "alpha"}
        <label>α start <input type="number" step="0.5" value={sweep?.start_deg} disabled={running} onchange={(e) => updateSweep({ start_deg: value(e) })} /></label>
        <label>α end <input type="number" step="0.5" value={sweep?.end_deg} disabled={running} onchange={(e) => updateSweep({ end_deg: value(e) })} /></label>
        <label>step <input type="number" min="0.01" step="0.25" value={sweep?.step_deg} disabled={running} onchange={(e) => updateSweep({ step_deg: value(e) })} /></label>
      {:else}
        <label>Cl start <input type="number" step="0.1" value={clSweep.start} disabled={running} onchange={(e) => updateClSweep({ start: value(e) })} /></label>
        <label>Cl end <input type="number" step="0.1" value={clSweep.end} disabled={running} onchange={(e) => updateClSweep({ end: value(e) })} /></label>
        <label>step <input type="number" min="0.005" step="0.05" value={clSweep.step} disabled={running} onchange={(e) => updateClSweep({ step: value(e) })} /></label>
      {/if}
      <label><Term id="mach">Mach</Term> <input type="number" min="0" max="0.99" step="0.05" value={mach} disabled={running} onchange={(e) => app.updateSettings({ mach: Math.max(0, Math.min(0.99, value(e))) })} /></label>
      <ViscousControls disabled={running} />
    </div>
    <div class="row">
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
        <div class="segmented" role="group" aria-label="View" data-tour="polar-view">
          <button class:active={view === "plots"} onclick={() => (view = "plots")}>Plots</button>
          <button
            class:active={view === "table"}
            title="Key figures of every polar side by side: Cl max, Cd min, L/D max…"
            onclick={() => (view = "table")}>Table</button
          >
        </div>
        <span class="muted">
          Visible airfoils are analyzed; results are kept in the project.
          {#if app.viscous.enabled}
            Viscous points start from the previous one (XFOIL {mode === "cl" ? "CSEQ" : "ASEQ"}); unconverged points are left out.
          {/if}
        </span>
      {/if}
    </div>
  </section>

  {#if view === "table" && !running}
    <ComparisonTable />
  {:else}
  <div class="plots" data-tour="polar-plots">
    <PlotPanel
      title={`Lift coefficient${machTag}`}
      fileName="cl_alpha"
      curves={clCurves}
      xLabel="α (deg)"
      yLabel="Cl"
      emptyMessage="Run a sweep to plot Cl(α)"
    />
    <PlotPanel
      title="Drag polar"
      fileName="cl_cd"
      tour="drag-polar"
      curves={dragCurves}
      xLabel="Cd"
      yLabel="Cl"
      emptyMessage="Run a viscous sweep to plot Cl(Cd)"
    />
    <PlotPanel
      title={hasHinge ? "Moment coefficient (c/4) and flap hinge moment Ch (dotted)" : "Moment coefficient (c/4)"}
      fileName="cm_alpha"
      curves={cmCurves}
      xLabel="α (deg)"
      yLabel="Cm"
      emptyMessage="Run a sweep to plot Cm(α)"
    />
    <PlotPanel
      title="Transition x/c — top solid, bottom dashed"
      fileName="xtr_alpha"
      curves={transitionCurves}
      xLabel="α (deg)"
      yLabel="x/c"
      emptyMessage="Run a viscous sweep to plot the transition points"
    />
  </div>
  {/if}
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
    flex-direction: column;
    gap: 8px;
    padding: 10px 14px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
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
  .segmented {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .segmented button {
    border: none;
    border-radius: 0;
    padding: 2px 10px;
    background: transparent;
  }
  .segmented button.active {
    background: var(--accent);
    color: white;
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
    grid-template-rows: 1fr 1fr;
    gap: 12px;
    min-height: 0;
  }
</style>
