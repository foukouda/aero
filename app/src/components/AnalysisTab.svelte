<script lang="ts">
  import type { PlotCurve } from "./Plot.svelte";
  import PlotPanel from "./PlotPanel.svelte";
  import BoundaryLayerPanel from "./BoundaryLayerPanel.svelte";
  import ViscousControls from "./ViscousControls.svelte";
  import { app } from "../lib/state.svelte";
  import { compactExp, fixed } from "../lib/format";
  import { splitSurfaces } from "../lib/plot/scales";

  const s = $derived(app.settings);
  const alpha = $derived(s?.alpha_deg ?? 0);
  const mach = $derived(s?.mach ?? 0);
  const viscous = $derived(app.viscous.enabled);

  const curves = $derived.by(() => {
    const out: PlotCurve[] = [];
    let cpStar: number | null = null;
    for (const a of app.visible) {
      const r = app.analyses[a.id];
      if (!r) continue;
      const { upper, lower } = splitSurfaces(r.x, r.cp, r.le_index);
      out.push({ label: `${a.name} upper`, color: a.color, x: upper.x, y: upper.y });
      out.push({ label: `${a.name} lower`, color: a.color, x: lower.x, y: lower.y, dash: [6, 4] });
      if (r.viscous) {
        // Inviscid Cp for comparison, thin.
        const inv = splitSurfaces(r.x, r.viscous.cp_inviscid, r.le_index);
        for (const [k, side] of [inv.upper, inv.lower].entries()) {
          out.push({
            label: `${a.name} inviscid`,
            color: a.color,
            x: side.x,
            y: side.y,
            dash: [1, 3],
            width: 1,
            hideInLegend: k > 0,
          });
        }
      }
      if (r.cp_star !== null) cpStar = r.cp_star;
    }
    if (cpStar !== null && out.length) {
      out.push({ label: "Cp* (sonic)", color: "#9ca3af", x: [0, 1], y: [cpStar, cpStar], dash: [2, 3] });
    }
    return out;
  });

  function setAlpha(value: number) {
    if (Number.isFinite(value)) app.updateSettings({ alpha_deg: Math.max(-30, Math.min(30, value)) });
  }

  function setMach(value: number) {
    if (Number.isFinite(value)) app.updateSettings({ mach: Math.max(0, Math.min(0.99, value)) });
  }

  function ratio(cl: number | null, cd: number | null | undefined): string {
    return cl === null || cd === null || cd === undefined || cd === 0 ? "—" : fixed(cl / cd, 1);
  }

  /** Transition forced by a trip set by the user (XFOIL also reports a
   * laminar layer reaching the trailing edge as forced there). */
  function tripped(forced: boolean, side: 0 | 1): boolean {
    return forced && (app.viscous.xtr[side] ?? 1) < 1;
  }

  // Columns after the airfoil name.
  const columns = $derived(viscous ? 9 : 6);
  const title = $derived(
    `Pressure coefficient, α = ${fixed(alpha, 1)}°` +
      (mach > 0 ? `, M = ${fixed(mach, 2)}` : "") +
      (viscous ? `, Re = ${compactExp(app.viscous.reynolds)} (inviscid dotted)` : ""),
  );
</script>

<div class="analysis">
  <section class="panel controls">
    <div class="alpha" data-tour="alpha">
      <label for="alpha">Angle of attack α</label>
      <input
        type="range"
        min="-15"
        max="20"
        step="0.1"
        value={alpha}
        oninput={(e) => setAlpha(Number(e.currentTarget.value))}
        aria-label="Angle of attack slider"
      />
      <input
        id="alpha"
        type="number"
        step="0.1"
        value={alpha}
        onchange={(e) => setAlpha(Number(e.currentTarget.value))}
      />
      <span class="muted">°</span>
    </div>
    <div class="mach" data-tour="mach">
      <label for="mach">Mach</label>
      <input
        id="mach"
        type="number"
        min="0"
        max="0.99"
        step="0.05"
        value={mach}
        onchange={(e) => setMach(Number(e.currentTarget.value))}
      />
      <label class="check" title="Return results beyond the critical Mach number (XFOIL behaviour)">
        <input
          type="checkbox"
          checked={s?.allow_supercritical ?? false}
          onchange={(e) => app.updateSettings({ allow_supercritical: e.currentTarget.checked })}
        />
        allow supercritical
      </label>
    </div>
    <ViscousControls tour="viscous" />
  </section>

  <section class="panel coefficients" data-tour="coefficients">
    <table class="mono">
      <thead>
        <tr>
          <th>Airfoil</th>
          <th>Cl</th>
          {#if viscous}
            <th>Cd</th>
            <th>L/D</th>
          {/if}
          <th>Cm c/4</th>
          {#if viscous}
            <th title="Transition x/c, top surface">Xtr top</th>
            <th title="Transition x/c, bottom surface">Xtr bot</th>
            <th title="Pressure drag / friction drag">Cdp / Cdf</th>
          {:else}
            <th>Cdp</th>
          {/if}
          <th>Cp min</th>
          {#if !viscous}<th>at x</th>{/if}
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each app.visible as a (a.id)}
          {@const r = app.analyses[a.id]}
          {@const v = r?.viscous}
          {@const err = app.analysisErrors[a.id]}
          <tr>
            <td><span class="swatch" style:background={a.color}></span>{a.name}</td>
            {#if r && (viscous ? v : !v)}
              <td>{fixed(r.cl)}</td>
              {#if v}
                <td>{fixed(v.cd, 5)}</td>
                <td>{ratio(r.cl, v.cd)}</td>
              {/if}
              <td>{fixed(r.cm)}</td>
              {#if v}
                <td>{fixed(v.transition[0].x, 3)}{tripped(v.transition[0].forced, 0) ? "ᵗ" : ""}</td>
                <td>{fixed(v.transition[1].x, 3)}{tripped(v.transition[1].forced, 1) ? "ᵗ" : ""}</td>
                <td>{fixed(r.cdp, 5)} / {fixed(v.cdf, 5)}</td>
              {:else}
                <td>{fixed(r.cdp, 5)}</td>
              {/if}
              <td>{fixed(r.cp_min, 3)}</td>
              {#if !v}<td>{fixed(r.cp_min_x, 3)}</td>{/if}
              <td>
                {#if v && !v.converged}
                  <span class="warn" title={`The Newton iteration did not converge in ${v.iterations} iterations (often beyond stall): the values are approximate`}
                    >not converged</span
                  >
                {/if}
                {#if r.supercritical}<span class="warn" title="Cp min below Cp*">supercritical</span>{/if}
                <button class="small" title="Export the Cp distribution as CSV" onclick={() => app.exportCpCsv(a.id)}>Cp CSV</button>
                {#if v}
                  <button class="small" title="Export the boundary layer as CSV" onclick={() => app.exportBoundaryLayerCsv(a.id)}>BL CSV</button>
                {/if}
              </td>
            {:else if err}
              <td colspan={columns} class="error">{err}</td>
            {:else}
              <td colspan={columns} class="muted">computing…</td>
            {/if}
          </tr>
        {:else}
          <tr><td colspan={columns + 1} class="muted">Show at least one airfoil of the library.</td></tr>
        {/each}
      </tbody>
    </table>
    {#if viscous}
      <p class="note muted">ᵗ: transition forced by the trip. Viscous points start from a fresh boundary layer, as XFOIL after INIT.</p>
    {/if}
  </section>

  <div class="plots" class:two={viscous}>
    <PlotPanel
      {title}
      fileName="cp"
      tour="cp-plot"
      {curves}
      xLabel="x"
      yLabel="Cp"
      yInverted
      emptyMessage={Object.keys(app.analysisErrors).length ? "No result: see the messages above" : "No analysis yet"}
    />
    {#if viscous}
      <BoundaryLayerPanel />
    {/if}
  </div>
</div>

<style>
  .analysis {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr);
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .controls {
    display: flex;
    gap: 24px;
    align-items: center;
    padding: 10px 14px;
    flex-wrap: wrap;
  }
  .alpha,
  .mach {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .alpha input[type="range"] {
    width: 280px;
    accent-color: var(--accent);
  }
  .alpha input[type="number"],
  .mach input[type="number"] {
    width: 72px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--muted);
  }
  .coefficients {
    padding: 6px 10px;
    overflow: auto;
    max-height: 200px;
  }
  table {
    border-collapse: collapse;
    width: 100%;
  }
  th,
  td {
    text-align: right;
    padding: 3px 8px;
    white-space: nowrap;
  }
  th:first-child,
  td:first-child {
    text-align: left;
  }
  th {
    color: var(--muted);
    font-weight: 500;
    border-bottom: 1px solid var(--border);
  }
  .swatch {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 2px;
    margin-right: 6px;
  }
  .error {
    color: var(--danger);
    text-align: left;
    white-space: normal;
  }
  .warn {
    color: var(--warning);
    margin-right: 6px;
  }
  .small {
    font-size: 11px;
    padding: 0 6px;
  }
  .note {
    margin: 4px 8px 0;
    font-size: 11px;
  }
  .plots {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
    min-height: 0;
  }
  .plots.two {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  }
</style>
