<script lang="ts">
  // "How it flies": the 3D analysis of the whole aircraft (aero-mission
  // `analyze`) — balance, static margin, glide and power with the parts of
  // the drag, the stall and the lift along the wing.
  import Term from "./Term.svelte";
  import PlotPanel from "./PlotPanel.svelte";
  import type { PlotCurve } from "./Plot.svelte";
  import type { AnalysisWarning, StripResult } from "../lib/bindings";
  import type { MessageKey } from "../lib/i18n";
  import { app } from "../lib/state.svelte";
  import { TOP_COLORS, dragShares, length, num, power, readMargin, readStallStart, sig, speedKmh } from "../lib/mission";

  const lang = $derived(app.tourLang);
  const result = $derived(app.flightAnalysis);
  const busy = $derived(app.analysingFlight);
  const colors = $derived(TOP_COLORS[app.theme]);
  const mac = $derived(app.missionView?.layout.wing.mac ?? 0);

  const WARNINGS: Record<AnalysisWarning, MessageKey> = {
    cannot_trim: "flies.warn.cannot_trim",
    wing_stalled: "flies.warn.wing_stalled",
    tail_stalled: "flies.warn.tail_stalled",
    missing_polars: "flies.warn.missing_polars",
  };
  const LEVEL_ICON = { ok: "✓", info: "i", warning: "!", danger: "✕" } as const;

  const margin = $derived(
    result && result.stability.static_margin !== null ? readMargin(result.stability.static_margin, mac) : null,
  );
  const stallStart = $derived(result?.stall ? readStallStart(result.stall) : null);
  const shares = $derived(result ? dragShares(result.drag) : []);
  const DRAG_COLORS = $derived([colors[0], colors[1], colors[2], "#9ca3af"]);

  /** Right-half strips of a part, root to tip. */
  function half(strips: StripResult[], part: string): StripResult[] {
    return strips
      .filter((s) => s.part === part && !s.left)
      .sort((a, b) => (a.span_fraction ?? 0) - (b.span_fraction ?? 0));
  }

  const curves = $derived.by((): PlotCurve[] => {
    if (!result) return [];
    const wing = half(result.strips, "wing");
    const tail = half(result.strips, "tail");
    const x = (s: StripResult[]) => s.map((p) => p.span_fraction ?? 0);
    const out: PlotCurve[] = [
      { label: app.t("flies.chart.cl"), color: colors[0], x: x(wing), y: wing.map((p) => p.cl), width: 2 },
      {
        label: app.t("flies.chart.max"),
        color: colors[0],
        x: x(wing),
        y: wing.map((p) => p.cl_max),
        dash: [6, 4],
        width: 1.5,
      },
    ];
    if (tail.length) {
      out.push({ label: app.t("flies.chart.tail"), color: colors[1], x: x(tail), y: tail.map((p) => p.cl), width: 1.5 });
    }
    return out;
  });

  function deg(v: number | null | undefined): string {
    return `${num(v, lang, 1)}°`;
  }
</script>

<section class="flies" aria-labelledby="flies-title">
  <header class="head">
    <div>
      <h2 id="flies-title">{app.t("flies.title")}</h2>
      <p class="muted">{app.t("flies.intro")}</p>
    </div>
    {#if busy}
      <button onclick={() => app.cancelFlightAnalysis()}>{app.t("flies.cancel")}</button>
    {:else}
      <button class="primary run" disabled={!app.missionView} onclick={() => app.runFlightAnalysis()}
        >{app.t(result ? "flies.again" : "flies.run")}</button
      >
    {/if}
  </header>

  {#if busy}
    <div class="progress" role="status">
      {#if busy.phase === "polars" && busy.total > 0}
        <progress max={busy.total} value={busy.done}></progress>
        <span>{app.t("flies.polars", { done: busy.done, total: busy.total })}</span>
      {:else}
        <progress></progress>
        <span>{app.t("flies.solving")}</span>
      {/if}
    </div>
  {/if}

  {#if !result}
    {#if !busy}<p class="panel empty">{app.t("flies.empty")}</p>{/if}
  {:else}
    {#if app.flightAnalysisStale}
      <p class="callout info">{app.t("flies.stale")}</p>
    {/if}
    {#each result.warnings as w (w)}
      <p class="callout danger">{app.t(WARNINGS[w])}</p>
    {/each}

    <div class="cards">
      <article class="panel card {result.trim.trimmed ? 'ok' : 'danger'}">
        <header><Term id="trim">{app.t("flies.trim")}</Term></header>
        <p class="value">{deg(result.trim.alpha_deg)}</p>
        <p>
          {app.t(result.trim.trimmed ? "flies.trim.ok" : "flies.trim.no", {
            alpha: deg(result.trim.alpha_deg),
            elevator: deg(result.trim.elevator_deg),
          })}
        </p>
      </article>

      {#if margin}
        <article class="panel card {margin.level}">
          <header>
            <Term id="static-margin">{app.t("flies.margin")}</Term>
            <span class="chip-level {margin.level}" aria-hidden="true">{LEVEL_ICON[margin.level]}</span>
          </header>
          <p class="value">{sig((result.stability.static_margin ?? 0) * 100, lang, 3)} %</p>
          <p>{app.t(margin.text, { move: length(margin.move, lang) })}</p>
          <p class="muted small">
            {app.t("flies.margin.points", {
              np: length(result.stability.neutral_point, lang),
              cg: length(result.stability.center_of_gravity, lang),
            })}
            {app.t("flies.margin.fuselage")}
          </p>
          <p class="small">
            {app.t((result.stability.cn_beta ?? 0) > 0 ? "flies.yaw.ok" : "flies.yaw.no")}
            {app.t((result.stability.cl_beta ?? 0) < 0 ? "flies.roll.ok" : "flies.roll.no")}
          </p>
        </article>
      {/if}

      <article class="panel card">
        <header><Term id="ld">{app.t("flies.performance")}</Term></header>
        <p class="value">{sig(result.drag.lift_to_drag, lang, 3)}</p>
        <p>
          {app.t("flies.glide", {
            ld: sig(result.drag.lift_to_drag, lang, 3),
            distance: length(result.drag.lift_to_drag, lang),
          })}
        </p>
        <p>
          {app.t("flies.power", {
            power: power(result.drag.power, lang),
            drag: `${sig(result.drag.drag, lang, 3)} N`,
          })}
        </p>
        <div class="bar" role="img" aria-label={shares.map((s) => `${app.t(s.key)} ${sig(s.share * 100, lang, 2)} %`).join(", ")}>
          {#each shares as s, i (s.key)}
            {#if s.share > 0}
              <span style:width="{s.share * 100}%" style:background={DRAG_COLORS[i]}></span>
            {/if}
          {/each}
        </div>
        <ul class="legend">
          {#each shares as s, i (s.key)}
            <li><span class="swatch" style:background={DRAG_COLORS[i]}></span>{app.t(s.key)} {sig(s.share * 100, lang, 2)} %</li>
          {/each}
        </ul>
      </article>

      <article class="panel card {stallStart?.level ?? 'info'}">
        <header><Term id="stall">{app.t("flies.stall")}</Term></header>
        {#if result.stall}
          <p class="value">{speedKmh(result.stall.speed, lang)}</p>
          <p>{app.t("flies.stall.speed", { speed: speedKmh(result.stall.speed, lang), cl: num(result.stall.cl_max, lang, 2) })}</p>
          {#if stallStart}<p>{app.t(stallStart.text)}</p>{/if}
        {:else}
          <p>{app.t("flies.stall.none")}</p>
        {/if}
      </article>
    </div>

    <div class="plot">
      <PlotPanel
        title={app.t("flies.chart")}
        fileName="lift-along-the-wing"
        {curves}
        xLabel={app.t("flies.chart.x")}
        yLabel="cl"
      />
    </div>
    <div class="foot">
      <label class="toggle">
        <input type="checkbox" bind:checked={app.colourByLift} />
        {app.t("flies.colour")}
      </label>
      <span class="muted small">{app.t("flies.vortices", { n: result.vortices ?? 0 })}</span>
    </div>
  {/if}
</section>

<style>
  .flies {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
  }
  .head h2 {
    margin: 0;
    font-size: 16px;
  }
  .head p {
    margin: 4px 0 0;
    max-width: 75ch;
  }
  .run {
    flex: none;
    background: var(--accent);
    color: var(--accent-text, #fff);
    font-weight: 600;
  }
  .progress {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .progress progress {
    width: 200px;
  }
  .empty {
    padding: 12px;
    margin: 0;
  }
  .callout {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }
  .callout.info {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 7%, var(--panel));
  }
  .callout.danger {
    border-color: color-mix(in srgb, var(--danger) 50%, transparent);
    background: color-mix(in srgb, var(--danger) 8%, var(--panel));
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 10px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
  }
  .card header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-weight: 600;
  }
  .card p {
    margin: 0;
  }
  .value {
    font-size: 22px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .small {
    font-size: 12px;
  }
  .bar {
    display: flex;
    height: 10px;
    border-radius: 4px;
    overflow: hidden;
    gap: 2px;
    background: var(--panel-2);
  }
  .bar span {
    height: 100%;
  }
  .legend {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 2px 10px;
    font-size: 12px;
  }
  .legend li {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .swatch {
    width: 10px;
    height: 10px;
    border-radius: 2px;
  }
  .plot {
    height: 340px;
    display: grid;
  }
  .foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 6px;
  }
</style>
