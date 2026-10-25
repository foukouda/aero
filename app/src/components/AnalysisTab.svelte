<script lang="ts">
  import type { PlotCurve } from "./Plot.svelte";
  import PlotPanel from "./PlotPanel.svelte";
  import { app } from "../lib/state.svelte";
  import { fixed } from "../lib/format";
  import { splitSurfaces } from "../lib/plot/scales";

  const s = $derived(app.settings);
  const alpha = $derived(s?.alpha_deg ?? 0);
  const mach = $derived(s?.mach ?? 0);

  const curves = $derived.by(() => {
    const out: PlotCurve[] = [];
    let cpStar: number | null = null;
    for (const a of app.visible) {
      const r = app.analyses[a.id];
      if (!r) continue;
      const { upper, lower } = splitSurfaces(r.x, r.cp, r.le_index);
      out.push({ label: `${a.name} upper`, color: a.color, x: upper.x, y: upper.y });
      out.push({ label: `${a.name} lower`, color: a.color, x: lower.x, y: lower.y, dash: [6, 4] });
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
</script>

<div class="analysis">
  <section class="panel controls">
    <div class="alpha">
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
    <div class="mach">
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
  </section>

  <section class="panel coefficients">
    <table class="mono">
      <thead>
        <tr>
          <th>Airfoil</th>
          <th>Cl</th>
          <th>Cm c/4</th>
          <th>Cdp</th>
          <th>Cp min</th>
          <th>at x</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each app.visible as a (a.id)}
          {@const r = app.analyses[a.id]}
          {@const err = app.analysisErrors[a.id]}
          <tr>
            <td><span class="swatch" style:background={a.color}></span>{a.name}</td>
            {#if r}
              <td>{fixed(r.cl)}</td>
              <td>{fixed(r.cm)}</td>
              <td>{fixed(r.cdp, 5)}</td>
              <td>{fixed(r.cp_min, 3)}</td>
              <td>{fixed(r.cp_min_x, 3)}</td>
              <td>
                {#if r.supercritical}<span class="warn" title="Cp min below Cp*">supercritical</span>{/if}
                <button class="small" title="Export Cp distribution as CSV" onclick={() => app.exportCpCsv(a.id)}>CSV</button>
              </td>
            {:else if err}
              <td colspan="6" class="error">{err}</td>
            {:else}
              <td colspan="6" class="muted">computing…</td>
            {/if}
          </tr>
        {:else}
          <tr><td colspan="7" class="muted">Show at least one airfoil of the library.</td></tr>
        {/each}
      </tbody>
    </table>
  </section>

  <PlotPanel
    title={`Pressure coefficient, α = ${fixed(alpha, 1)}°${mach > 0 ? `, M = ${fixed(mach, 2)}` : ""}`}
    fileName="cp"
    {curves}
    xLabel="x"
    yLabel="Cp"
    yInverted
    emptyMessage={Object.keys(app.analysisErrors).length ? "No result: see the messages above" : "No analysis yet"}
  />
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
    width: 320px;
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
    max-height: 180px;
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
</style>
