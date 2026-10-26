<script lang="ts">
  import Term from "./Term.svelte";
  import { app } from "../lib/state.svelte";
  import { fixed } from "../lib/format";
  import { sortRows, summarizePolar, type SummaryKey } from "../lib/polar-summary";

  let atCl = $state(0.5);
  let sortKey = $state<SummaryKey>("ldMax");
  let descending = $state(true);

  /** Stored polars of the visible airfoils. */
  const rows = $derived.by(() => {
    const out = [];
    for (const a of app.visible) {
      const stored = app.session?.polars.find((p) => p.airfoil_id === a.id);
      if (!stored) continue;
      const viscous = stored.polar.kind === "viscous";
      out.push({
        id: a.id,
        name: a.name,
        color: a.color,
        viscous,
        stopped: stored.polar.kind === "viscous" && stored.polar.stopped,
        summary: summarizePolar(stored.polar.points, atCl),
      });
    }
    return sortRows(out, sortKey, descending);
  });

  const missing = $derived(
    app.visible.filter((a) => !app.session?.polars.some((p) => p.airfoil_id === a.id)).length,
  );

  function sortBy(key: SummaryKey) {
    if (sortKey === key) descending = !descending;
    else {
      sortKey = key;
      // Larger is better for these, smaller for the others.
      descending = ["clMax", "ldMax", "atClLd", "cm0", "atClCm"].includes(key);
    }
  }

  function arrow(key: SummaryKey): string {
    return sortKey === key ? (descending ? "▼" : "▲") : "⇅";
  }

  function csvNumber(v: number | null | undefined, digits: number): string {
    return v === null || v === undefined || !Number.isFinite(v) ? "" : v.toFixed(digits);
  }

  function exportCsv() {
    const header = [
      "airfoil",
      "points",
      "alpha_zero_lift_deg",
      "cl_max",
      "alpha_cl_max_deg",
      "cd_min",
      "cl_at_cd_min",
      "ld_max",
      "cl_at_ld_max",
      "cm_zero_lift",
      `alpha_at_cl_${atCl}`,
      `cd_at_cl_${atCl}`,
      `ld_at_cl_${atCl}`,
      `cm_at_cl_${atCl}`,
    ];
    const lines = rows.map((r) => {
      const s = r.summary;
      return [
        `"${r.name.replace(/"/g, '""')}"`,
        String(s.points),
        csvNumber(s.alpha0, 3),
        csvNumber(s.clMax, 4),
        csvNumber(s.alphaClMax, 2),
        csvNumber(s.cdMin, 6),
        csvNumber(s.clCdMin, 4),
        csvNumber(s.ldMax, 2),
        csvNumber(s.clLdMax, 4),
        csvNumber(s.cm0, 4),
        csvNumber(s.atCl?.alpha, 3),
        csvNumber(s.atCl?.cd, 6),
        csvNumber(s.atCl?.ld, 2),
        csvNumber(s.atCl?.cm, 4),
      ].join(",");
    });
    void app.saveText("polar_comparison.csv", [header.join(","), ...lines].join("\n") + "\n");
  }
</script>

<section class="panel comparison" data-tour="comparison">
  <div class="toolbar">
    <label>
      Values at <Term id="cl">Cl</Term> =
      <input
        type="number"
        step="0.1"
        value={atCl}
        onchange={(e) => {
          const x = Number(e.currentTarget.value);
          if (Number.isFinite(x)) atCl = x;
        }}
      />
    </label>
    <span class="muted">
      {rows.length} airfoil(s){missing ? ` · ${missing} visible airfoil(s) without a polar: run a sweep` : ""}. Click
      ⇅ to sort; click a row to select the airfoil.
    </span>
    <button onclick={exportCsv} disabled={rows.length === 0}>Export table CSV</button>
  </div>
  {#if rows.length === 0}
    <p class="muted empty">Run a sweep: the key figures of every visible airfoil's polar are compared here.</p>
  {:else}
    <div class="scroll">
      <table>
        <thead>
          <tr>
            <th class="left">Airfoil <button class="sort" onclick={() => sortBy("name")}>{arrow("name")}</button></th>
            <th title="Zero-lift angle of attack"
              ><Term id="alpha">α₀</Term> <button class="sort" onclick={() => sortBy("alpha0")}>{arrow("alpha0")}</button></th
            >
            <th title="Maximum lift coefficient (and its angle of attack)"
              ><Term id="stall">Cl max</Term> <button class="sort" onclick={() => sortBy("clMax")}>{arrow("clMax")}</button></th
            >
            <th title="Minimum drag coefficient (and the Cl where it occurs)"
              ><Term id="cd">Cd min</Term> <button class="sort" onclick={() => sortBy("cdMin")}>{arrow("cdMin")}</button></th
            >
            <th title="Maximum lift-to-drag ratio (and the Cl where it occurs)"
              ><Term id="ld">L/D max</Term> <button class="sort" onclick={() => sortBy("ldMax")}>{arrow("ldMax")}</button></th
            >
            <th title="Moment coefficient at zero lift"
              ><Term id="cm">Cm₀</Term> <button class="sort" onclick={() => sortBy("cm0")}>{arrow("cm0")}</button></th
            >
            <th class="group" title={`Interpolated at Cl = ${atCl}, below the stall`}
              >α <button class="sort" onclick={() => sortBy("atClAlpha")}>{arrow("atClAlpha")}</button></th
            >
            <th title={`Interpolated at Cl = ${atCl}`}
              >Cd <button class="sort" onclick={() => sortBy("atClCd")}>{arrow("atClCd")}</button></th
            >
            <th title={`Interpolated at Cl = ${atCl}`}
              >L/D <button class="sort" onclick={() => sortBy("atClLd")}>{arrow("atClLd")}</button></th
            >
            <th title={`Interpolated at Cl = ${atCl}`}
              >Cm <button class="sort" onclick={() => sortBy("atClCm")}>{arrow("atClCm")}</button></th
            >
          </tr>
          <tr class="sub">
            <th></th>
            <th>°</th>
            <th>(at α)</th>
            <th>(at Cl)</th>
            <th>(at Cl)</th>
            <th></th>
            <th class="group" colspan="4">at Cl = {atCl}</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as r (r.id)}
            {@const s = r.summary}
            <tr class:selected={r.id === app.selectedId} onclick={() => app.select(r.id)}>
              <td class="left">
                <span class="swatch" style:background={r.color}></span>
                {r.name}
                <small class="muted">{r.viscous ? "" : "inviscid"}</small>
                {#if r.stopped}
                  <span
                    class="warn"
                    title="A sequence of the sweep stopped after 4 points in a row did not converge (as XFOIL does): part of the range is missing, so some figures may be wrong. Try a narrower range or another Reynolds number."
                    >⚠ incomplete</span
                  >
                {/if}
              </td>
              <td>{fixed(s.alpha0, 2)}</td>
              <td>{fixed(s.clMax, 3)} <small class="muted">{fixed(s.alphaClMax, 1)}°</small></td>
              <td>{fixed(s.cdMin, 5)} <small class="muted">{fixed(s.clCdMin, 2)}</small></td>
              <td>{fixed(s.ldMax, 1)} <small class="muted">{fixed(s.clLdMax, 2)}</small></td>
              <td>{fixed(s.cm0, 4)}</td>
              <td class="group">{fixed(s.atCl?.alpha, 2)}</td>
              <td>{fixed(s.atCl?.cd, 5)}</td>
              <td>{fixed(s.atCl?.ld, 1)}</td>
              <td>{fixed(s.atCl?.cm, 4)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="muted note">
      From the computed points (no extrapolation): Cl max is the largest Cl of the sweep — widen the sweep if it is
      the last point. Near the stall, viscous results are approximate.
    </p>
  {/if}
</section>

<style>
  .comparison {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 14px;
    min-height: 0;
    height: 100%;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  .toolbar label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .toolbar input {
    width: 70px;
  }
  .toolbar button {
    margin-left: auto;
  }
  .scroll {
    overflow: auto;
    min-height: 0;
    flex: 1;
  }
  table {
    border-collapse: collapse;
    width: 100%;
    font-variant-numeric: tabular-nums;
  }
  th,
  td {
    padding: 4px 10px;
    text-align: right;
    white-space: nowrap;
    border-bottom: 1px solid var(--border);
  }
  th {
    position: sticky;
    top: 0;
    background: var(--panel);
    font-weight: 600;
  }
  tr.sub th {
    top: 28px;
    font-weight: 400;
    color: var(--muted);
    font-size: 11px;
  }
  .left {
    text-align: left;
  }
  .group {
    border-left: 2px solid var(--border);
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr:hover,
  tr.selected {
    background: var(--panel-2);
  }
  .swatch {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 3px;
    margin-right: 6px;
  }
  .sort {
    border: none;
    background: none;
    padding: 0 2px;
    font-size: 10px;
    color: var(--muted);
    cursor: pointer;
  }
  .warn {
    color: #d97706;
    font-size: 11px;
    margin-left: 4px;
  }
  .empty,
  .note {
    margin: 0;
    font-size: 12px;
  }
</style>
