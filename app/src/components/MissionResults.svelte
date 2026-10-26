<script lang="ts">
  // Recommended airfoils for the flight of the My aircraft tab: the best
  // three for the chosen goal, their drag polars, and every candidate.
  import Term from "./Term.svelte";
  import AirfoilThumb from "./AirfoilThumb.svelte";
  import PlotPanel from "./PlotPanel.svelte";
  import type { PlotCurve } from "./Plot.svelte";
  import type { CandidateResult, Fit, Goal } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import type { MessageKey } from "../lib/i18n";
  import { GOALS, TOP_COLORS, best, num, power, ranked, sig, speedKmh } from "../lib/mission";

  const result = $derived(app.recommendation);
  const goal = $derived<Goal>(app.mission?.goal ?? "least_drag");
  const lang = $derived(app.tourLang);
  const colors = $derived(TOP_COLORS[app.theme]);
  const top = $derived(result ? best(result, goal) : []);
  const all = $derived(result ? ranked(result, goal) : []);
  const re = $derived(result?.conditions.reynolds ?? null);
  const requiredCl = $derived(result?.conditions.required_cl ?? null);

  const FIT_TEXT: Record<Fit, MessageKey> = {
    comfortable: "fit.comfortable",
    tight: "fit.tight",
    near_stall: "fit.near_stall",
    cannot_fly: "fit.cannot_fly",
    no_data: "fit.no_data",
  };
  const FIT_LEVEL: Record<Fit, string> = {
    comfortable: "ok",
    tight: "warning",
    near_stall: "danger",
    cannot_fly: "danger",
    no_data: "info",
  };

  function goalLabel(g: Goal): string {
    const entry = GOALS.find((x) => x.goal === g);
    return entry ? app.t(entry.label) : g;
  }

  /** The catalogue description of a candidate, in the current language. */
  function description(c: CandidateResult): string | null {
    if (c.source.kind !== "catalogue") return null;
    const id = c.source.id;
    return app.catalogue.find((e) => e.id === id)?.description[lang] ?? null;
  }

  function buildText(c: CandidateResult): MessageKey {
    const b = c.evaluation?.buildability;
    if (!b || b.easy) return "cand.easy";
    return (b.undercamber ?? 0) > 0.005 ? "cand.hard" : "cand.thin";
  }

  const curves = $derived.by((): PlotCurve[] => {
    if (!result) return [];
    const out: PlotCurve[] = [];
    const highlighted = new Map(top.map((c, i) => [c, colors[i]]));
    let cdMin = Number.POSITIVE_INFINITY;
    let cdMax = 0;
    // The useful part of the polars: up to the onset of stall and three
    // times the lowest drag; separated points would stretch the drag axis.
    const lowest = Math.min(
      ...all.flatMap((c) => (c.evaluation?.polar ?? []).map((p) => p.cd ?? Number.POSITIVE_INFINITY)),
    );
    const cdLimit = Math.max(3 * lowest, 0.03);
    for (const c of all) {
      const stall = c.evaluation?.cl_max?.alpha_deg ?? Number.POSITIVE_INFINITY;
      const polar = (c.evaluation?.polar ?? []).filter(
        (p) => (p.alpha_deg ?? 0) <= stall + 1 && (p.cd ?? Number.POSITIVE_INFINITY) <= cdLimit,
      );
      if (!polar.length) continue;
      const cd = polar.map((p) => p.cd ?? Number.NaN);
      for (const v of cd) {
        if (Number.isFinite(v)) {
          cdMin = Math.min(cdMin, v);
          cdMax = Math.max(cdMax, Math.min(v, 0.1));
        }
      }
      const color = highlighted.get(c);
      out.push({
        label: c.name,
        color: color ?? "#9ca3af",
        x: cd,
        y: polar.map((p) => p.cl),
        width: color ? 2 : 1,
        hideInLegend: !color,
      });
    }
    // Highlighted polars drawn last, on top.
    out.sort((a, b) => Number(a.hideInLegend === false) - Number(b.hideInLegend === false));
    if (requiredCl !== null && Number.isFinite(cdMin)) {
      out.push({
        label: app.t("polar.required"),
        color: "#6b7280",
        x: [cdMin, cdMax],
        y: [requiredCl, requiredCl],
        dash: [5, 4],
        width: 1,
      });
    }
    return out;
  });
</script>

<section class="results" aria-labelledby="results-title">
  <header class="head">
    <h2 id="results-title">{app.t("results.title")}</h2>
    {#if result && requiredCl !== null && re !== null}
      <p class="muted">
        {app.t("results.context", { cl: num(requiredCl, lang, 2), re: sig(re, lang, 3), goal: goalLabel(goal) })}
      </p>
    {/if}
  </header>

  {#if !result}
    <p class="panel empty">{app.t("results.empty", { count: app.catalogue.length })}</p>
  {:else}
    {#if re !== null && re < 1e5}
      <p class="callout warning">{app.t("results.lowRe")}</p>
    {/if}
    {#if top.length === 0}
      <p class="callout danger">{app.t("results.none")}</p>
    {/if}

    <div class="top">
      {#each top as c, i (c.name + i)}
        {@const e = c.evaluation}
        {@const p = e?.performance}
        {@const op = e?.operating_point}
        <article class="panel cand">
          <header>
            <span class="rank" style:background={colors[i]}>{i + 1}</span>
            <div class="title">
              <h3>{c.name}</h3>
              {#if c.source.kind === "library"}<span class="tag">{app.t("cand.fromLibrary")}</span>{/if}
            </div>
            {#if e}
              <span class="chip-level {FIT_LEVEL[e.fit]}">{app.t(FIT_TEXT[e.fit])}</span>
            {/if}
          </header>
          <AirfoilThumb x={c.outline_x} y={c.outline_y} color={colors[i]} width={220} />
          {#if description(c)}
            <p class="desc">{description(c)}</p>
          {/if}
          <dl>
            <div><dt><Term id="stall">{app.t("cand.stall")}</Term></dt><dd>{speedKmh(p?.stall_speed, lang)}</dd></div>
            <div><dt><Term id="ld">{app.t("cand.glide")}</Term></dt><dd>{sig(p?.lift_to_drag, lang, 3)}</dd></div>
            <div><dt>{app.t("cand.power")}</dt><dd>{power(p?.power, lang)}</dd></div>
            <div><dt><Term id="alpha">{app.t("cand.angle")}</Term></dt><dd>{num(op?.alpha_deg, lang, 1)}°</dd></div>
            <div><dt><Term id="cd">{app.t("cand.cd")}</Term></dt><dd>{num(op?.cd, lang, 4)}</dd></div>
            <div><dt><Term id="stall">{app.t("cand.clmax")}</Term></dt><dd>{num(e?.cl_max?.cl, lang, 2)}</dd></div>
          </dl>
          <p class="build muted">
            {app.t(buildText(c))} · <Term id="thickness">{app.t("cand.thickness")}</Term>
            {sig((e?.buildability.thickness ?? Number.NaN) * 100, lang, 3)} %
          </p>
          <div class="actions">
            <button class="primary" onclick={() => app.putOnWing(c)}>{app.t("cand.putOnWing")}</button>
            <button onclick={() => app.analyseCandidate(c)}>{app.t("cand.analyse")}</button>
            <button onclick={() => app.optimizeCandidate(c)}>{app.t("cand.optimize")}</button>
          </div>
        </article>
      {/each}
    </div>

    <div class="plot">
      <PlotPanel
        title={app.t("polar.title")}
        fileName="recommended-polars"
        {curves}
        xLabel="Cd"
        yLabel="Cl"
      />
    </div>

    <section class="panel table-panel">
      <h3>{app.t("table.title")}</h3>
      <div class="scroll">
        <table>
          <thead>
            <tr>
              <th>{app.t("table.airfoil")}</th>
              <th>{app.t("table.fit")}</th>
              <th class="num">Cd</th>
              <th class="num">Cl max</th>
              <th class="num">{app.t("cand.stall")}</th>
              <th class="num">{app.t("cand.glide")}</th>
              <th>{app.t("table.build")}</th>
            </tr>
          </thead>
          <tbody>
            {#each all as c, i (c.name + i)}
              {@const e = c.evaluation}
              <tr>
                <td>{c.name}</td>
                {#if e}
                  <td><span class="chip-level {FIT_LEVEL[e.fit]}">{app.t(FIT_TEXT[e.fit])}</span></td>
                  <td class="num">{num(e.operating_point?.cd, lang, 4)}</td>
                  <td class="num">{num(e.cl_max?.cl, lang, 2)}</td>
                  <td class="num">{speedKmh(e.performance?.stall_speed, lang)}</td>
                  <td class="num">{sig(e.performance?.lift_to_drag, lang, 3)}</td>
                  <td>{app.t(buildText(c))}</td>
                {:else}
                  <td colspan="6" class="muted">{app.t("cand.failed", { message: c.error ?? "" })}</td>
                {/if}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <p class="muted note">{app.t("results.estimate")}</p>
    </section>
  {/if}
</section>

<style>
  .results {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 12px;
  }
  .head p {
    margin: 0;
  }
  .empty {
    margin: 0;
    padding: 18px;
    max-width: 70ch;
  }
  .callout {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }
  .callout.warning {
    border-color: color-mix(in srgb, var(--warning) 50%, transparent);
    background: color-mix(in srgb, var(--warning) 8%, var(--panel));
  }
  .callout.danger {
    border-color: color-mix(in srgb, var(--danger) 50%, transparent);
    background: color-mix(in srgb, var(--danger) 8%, var(--panel));
  }
  .top {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 10px;
  }
  .cand {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .cand header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .rank {
    /* Dark digits: readable on all three highlight colours. */
    color: #0b1220;
    font-weight: 700;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    flex: none;
  }
  .title {
    flex: 1;
    min-width: 0;
  }
  h3 {
    margin: 0;
    font-size: 14px;
  }
  .tag {
    font-size: 11px;
    color: var(--muted);
  }
  .desc {
    margin: 0;
    font-size: 12px;
    color: var(--muted);
  }
  dl {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px 12px;
    margin: 0;
  }
  dl div {
    display: flex;
    flex-direction: column;
  }
  dt {
    font-size: 11px;
    color: var(--muted);
  }
  dd {
    margin: 0;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .build {
    margin: 0;
    font-size: 12px;
  }
  .actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-top: auto;
  }
  .plot {
    height: 380px;
    display: grid;
  }
  .table-panel {
    padding: 10px 12px;
  }
  .table-panel h3 {
    margin-bottom: 6px;
  }
  .scroll {
    overflow: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-variant-numeric: tabular-nums;
  }
  th,
  td {
    text-align: left;
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }
  th {
    font-weight: 600;
    color: var(--muted);
    font-size: 12px;
  }
  .num {
    text-align: right;
  }
  .note {
    margin: 8px 0 0;
    font-size: 12px;
  }
  .chip-level {
    border-radius: 4px;
    padding: 0 6px;
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
    background: var(--panel-2);
    border: 1px solid var(--border);
  }
  .chip-level.ok {
    color: var(--ok);
  }
  .chip-level.info {
    color: var(--accent);
  }
  .chip-level.warning {
    color: var(--warning);
  }
  .chip-level.danger {
    color: var(--danger);
  }
</style>
