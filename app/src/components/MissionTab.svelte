<script lang="ts">
  // "My aircraft": the flight in everyday units, what its numbers mean,
  // and the recommended airfoils (MissionResults).
  import Term from "./Term.svelte";
  import WingSketch from "./WingSketch.svelte";
  import MissionResults from "./MissionResults.svelte";
  import type { MissionSetup } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import type { MessageKey } from "../lib/i18n";
  import {
    GOALS,
    PRESETS,
    TURBULENCE,
    applyPreset,
    length,
    num,
    readCl,
    readMach,
    readReynolds,
    readWingLoading,
    sig,
    wingLoadingGdm2,
    type Level,
  } from "../lib/mission";

  const setup = $derived(app.mission);
  const c = $derived(app.missionConditions);
  const lang = $derived(app.tourLang);
  let massUnit = $state<"g" | "kg">("g");
  let unitChosen = false;

  $effect(() => {
    // Grams for models, kilograms from 1 kg, until the user picks one.
    if (setup && !unitChosen) massUnit = (setup.flight.mass ?? 0) < 1 ? "g" : "kg";
  });

  /** Applies a change to a copy of the setup. */
  function update(change: (s: MissionSetup) => void) {
    if (!setup) return;
    const s = $state.snapshot(setup) as MissionSetup;
    change(s);
    app.updateMission(s);
  }

  /** The typed number, if finite and positive (or any finite with `signed`). */
  function read(e: Event, signed = false): number | null {
    const v = (e.currentTarget as HTMLInputElement).valueAsNumber;
    return Number.isFinite(v) && (signed || v > 0) ? v : null;
  }

  function round(v: number, digits: number): number {
    return Number(v.toFixed(digits));
  }

  const LEVEL_ICON: Record<Level, string> = { ok: "✓", info: "i", warning: "!", danger: "✕" };
  const LEVEL_TEXT: Record<Level, MessageKey> = {
    ok: "level.ok",
    info: "level.info",
    warning: "level.warning",
    danger: "level.danger",
  };

  const clReading = $derived(c?.required_cl != null ? readCl(c.required_cl) : null);
  const reReading = $derived(c?.reynolds != null ? readReynolds(c.reynolds) : null);
  const machReading = $derived(c?.mach != null ? readMach(c.mach) : null);
  const loading = $derived(c ? wingLoadingGdm2(c) : Number.NaN);
  const seaLevelDensity = 1.225;
</script>

{#if setup}
  <div class="mission">
    <section class="panel form" aria-labelledby="mission-title">
      <h1 id="mission-title">{app.t("mission.title")}</h1>
      <p class="muted intro">{app.t("mission.intro")}</p>

      <div class="presets" role="group" aria-label={app.t("mission.presets")}>
        <span class="muted">{app.t("mission.presets")}</span>
        {#each PRESETS as p (p.id)}
          <button class="chip" onclick={() => update((s) => Object.assign(s, applyPreset(s, p)))}
            >{app.t(p.label)}</button
          >
        {/each}
      </div>

      <div class="sentence">
        <label>
          <span>{app.t("mission.speed")}</span>
          <span class="field">
            <input
              type="number"
              min="1"
              step="1"
              value={round((setup.flight.speed ?? 0) * 3.6, 1)}
              oninput={(e) => {
                const v = read(e);
                if (v !== null) update((s) => (s.flight.speed = v / 3.6));
              }}
            />
            <span class="unit">km/h</span>
          </span>
        </label>
        <label>
          <span>{app.t("mission.altitude")}</span>
          <span class="field">
            <input
              type="number"
              min="-1000"
              max="20000"
              step="10"
              value={round(setup.flight.altitude ?? 0, 0)}
              oninput={(e) => {
                const v = read(e, true);
                if (v !== null) update((s) => (s.flight.altitude = v));
              }}
            />
            <span class="unit">m</span>
          </span>
        </label>
        <label>
          <span>{app.t("mission.mass")}</span>
          <span class="field">
            <input
              type="number"
              min="0"
              step={massUnit === "g" ? 10 : 0.1}
              value={massUnit === "g" ? round((setup.flight.mass ?? 0) * 1000, 0) : round(setup.flight.mass ?? 0, 3)}
              oninput={(e) => {
                const v = read(e);
                if (v !== null) update((s) => (s.flight.mass = massUnit === "g" ? v / 1000 : v));
              }}
            />
            <select
              class="unit"
              value={massUnit}
              aria-label="unit"
              onchange={(e) => {
                unitChosen = true;
                massUnit = (e.currentTarget as HTMLSelectElement).value as "g" | "kg";
              }}
            >
              <option value="g">g</option>
              <option value="kg">kg</option>
            </select>
          </span>
        </label>
      </div>

      <fieldset class="wing">
        <legend>{app.t("mission.wing")}</legend>
        <div class="segmented" role="radiogroup">
          <button
            role="radio"
            aria-checked={setup.sizing_cl === null}
            class:active={setup.sizing_cl === null}
            onclick={() => update((s) => (s.sizing_cl = null))}>{app.t("mission.wingKnown")}</button
          >
          <button
            role="radio"
            aria-checked={setup.sizing_cl !== null}
            class:active={setup.sizing_cl !== null}
            onclick={() => update((s) => (s.sizing_cl = s.sizing_cl ?? 0.6))}>{app.t("mission.wingSized")}</button
          >
        </div>
        <label class="row">
          <span>{app.t("mission.span")}</span>
          <span class="field">
            <input
              type="number"
              min="0.05"
              step="0.05"
              value={round(setup.wing.span ?? 0, 3)}
              oninput={(e) => {
                const v = read(e);
                if (v !== null) update((s) => (s.wing.span = v));
              }}
            />
            <span class="unit">m</span>
          </span>
        </label>
        {#if setup.sizing_cl === null}
          <label class="row">
            <span>{app.t("mission.chord")}</span>
            <span class="field">
              <input
                type="number"
                min="1"
                step="1"
                value={round((setup.wing.chord ?? 0) * 100, 1)}
                oninput={(e) => {
                  const v = read(e);
                  if (v !== null) update((s) => (s.wing.chord = v / 100));
                }}
              />
              <span class="unit">cm</span>
            </span>
          </label>
        {:else}
          <label class="row">
            <span><Term id="cl">{app.t("mission.sizingCl")}</Term></span>
            <span class="field">
              <input
                type="number"
                min="0.1"
                max="2"
                step="0.05"
                value={setup.sizing_cl}
                oninput={(e) => {
                  const v = read(e);
                  if (v !== null) update((s) => (s.sizing_cl = Math.min(v, 2.5)));
                }}
              />
            </span>
          </label>
          <p class="muted hint">{app.t("mission.sizingHint")}</p>
          <p class="sized">
            {app.t("mission.sizedWing", {
              chord: length(setup.wing.chord, lang),
              area: `${sig((setup.wing.span ?? 0) * (setup.wing.chord ?? 0), lang, 3)} m²`,
            })}
          </p>
        {/if}
      </fieldset>

      <fieldset>
        <legend>{app.t("mission.goal")}</legend>
        <div class="goals" role="radiogroup">
          {#each GOALS as g (g.goal)}
            <button
              role="radio"
              aria-checked={setup.goal === g.goal}
              class="goal"
              class:active={setup.goal === g.goal}
              onclick={() => update((s) => (s.goal = g.goal))}
            >
              <strong>{app.t(g.label)}</strong>
              <span class="muted">{app.t(g.hint)}</span>
            </button>
          {/each}
        </div>
      </fieldset>

      <details class="advanced">
        <summary>{app.t("mission.advanced")}</summary>
        <label class="row">
          <span><Term id="ncrit">{app.t("mission.air")}</Term></span>
          <select
            value={setup.ncrit}
            onchange={(e) => {
              const v = Number((e.currentTarget as HTMLSelectElement).value);
              update((s) => (s.ncrit = v));
            }}
          >
            {#each TURBULENCE as t (t.ncrit)}
              <option value={t.ncrit}>{app.t(t.label)}</option>
            {/each}
          </select>
        </label>
        <label class="row">
          <span>{app.t("mission.temperature")}</span>
          <input
            type="number"
            min="-60"
            max="60"
            step="5"
            value={setup.flight.temperature_offset ?? 0}
            oninput={(e) => {
              const v = read(e, true);
              if (v !== null) update((s) => (s.flight.temperature_offset = v));
            }}
          />
        </label>
        <p class="muted hint">{app.t("mission.temperatureHint")}</p>
        <label class="row">
          <span>{app.t("mission.spanEfficiency")}</span>
          <input
            type="number"
            min="0.5"
            max="1"
            step="0.05"
            value={setup.model.span_efficiency}
            oninput={(e) => {
              const v = read(e);
              if (v !== null && v <= 1) update((s) => (s.model.span_efficiency = v));
            }}
          />
        </label>
        <label class="row">
          <span>{app.t("mission.clMaxFactor")}</span>
          <input
            type="number"
            min="0.5"
            max="1"
            step="0.05"
            value={setup.model.cl_max_factor}
            oninput={(e) => {
              const v = read(e);
              if (v !== null && v <= 1) update((s) => (s.model.cl_max_factor = v));
            }}
          />
        </label>
      </details>

      {#if app.airfoils.length > 0}
        <label class="check">
          <input type="checkbox" bind:checked={app.includeLibrary} />
          {app.t("mission.includeLibrary", { count: app.airfoils.length })}
        </label>
      {/if}

      {#if app.recommending}
        {@const r = app.recommending}
        <div class="progress" role="status">
          <progress max={r.total} value={r.done}></progress>
          <span>{app.t("mission.finding", { done: r.done, total: r.total })}</span>
          <button onclick={() => app.cancelRecommendation()}>{app.t("mission.stop")}</button>
        </div>
      {:else}
        <button class="primary find" disabled={!c} onclick={() => app.runRecommendation()}
          >{app.t("mission.find")}</button
        >
      {/if}
    </section>

    <div class="right">
      <section class="meaning" aria-labelledby="meaning-title">
        <h2 id="meaning-title">{app.t("card.title")}</h2>
        {#if app.missionError}
          <p class="callout danger">{app.t("mission.flightError", { message: app.missionError })}</p>
        {:else if c}
          <div class="cards">
            {#if clReading}
              <article class="panel card {clReading.level}">
                <header>
                  <Term id="cl">{app.t("card.cl")}</Term>
                  <span class="chip-level {clReading.level}"
                    ><span aria-hidden="true">{LEVEL_ICON[clReading.level]}</span>
                    {app.t(LEVEL_TEXT[clReading.level])}</span
                  >
                </header>
                <p class="value">{num(c.required_cl, lang, 2)}</p>
                <p>{app.t(clReading.text)}</p>
              </article>
            {/if}
            {#if reReading}
              <article class="panel card {reReading.level}">
                <header>
                  <Term id="reynolds">{app.t("card.re")}</Term>
                  <span class="chip-level {reReading.level}"
                    ><span aria-hidden="true">{LEVEL_ICON[reReading.level]}</span>
                    {app.t(LEVEL_TEXT[reReading.level])}</span
                  >
                </header>
                <p class="value">{sig(c.reynolds, lang, 3)}</p>
                <p>{app.t(reReading.text)}</p>
              </article>
            {/if}
            <article class="panel card">
              <header><Term id="wing-loading">{app.t("card.loading")}</Term></header>
              <p class="value">{sig(loading, lang, 3)} g/dm²</p>
              <p>{app.t(readWingLoading(loading))}</p>
            </article>
            <article class="panel card">
              <header>
                <Term id="atmosphere">{app.t("card.air", { altitude: `${sig(setup.flight.altitude, lang, 4)} m` })}</Term>
              </header>
              <p class="value">{num(c.atmosphere.density, lang, 3)} kg/m³</p>
              <p>
                {app.t("card.air.text", {
                  temperature: `${num((c.atmosphere.temperature ?? Number.NaN) - 273.15, lang, 1)} °C`,
                  percent: `${sig(((c.atmosphere.density ?? Number.NaN) / seaLevelDensity) * 100, lang, 3)} %`,
                })}
              </p>
            </article>
            {#if machReading && (c.mach ?? 0) >= 0.2}
              <article class="panel card {machReading.level}">
                <header><Term id="mach">{app.t("card.mach")}</Term></header>
                <p class="value">{num(c.mach, lang, 2)}</p>
                <p>{app.t(machReading.text)}</p>
              </article>
            {/if}
            <article class="panel card sketch">
              <header>{app.t("sketch.title")}</header>
              <WingSketch span={setup.wing.span ?? 0} chord={setup.wing.chord ?? 0} />
            </article>
          </div>
        {/if}
      </section>

      <MissionResults />
    </div>
  </div>
{/if}

<style>
  .mission {
    display: grid;
    grid-template-columns: minmax(320px, 380px) minmax(0, 1fr);
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .form {
    padding: 16px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  h1 {
    font-size: 18px;
    margin: 0;
    letter-spacing: -0.01em;
  }
  .intro {
    margin: -6px 0 0;
    font-size: 13px;
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .chip {
    border-radius: 999px;
    padding: 2px 10px;
    font-size: 12px;
  }
  .sentence {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 15px;
  }
  .sentence label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .field {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .field input {
    width: 90px;
    font-size: 15px;
    text-align: right;
  }
  .unit {
    color: var(--muted);
    min-width: 34px;
  }
  select.unit {
    color: var(--text);
  }
  fieldset {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px 12px 12px;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  legend {
    font-weight: 600;
    padding: 0 4px;
  }
  .segmented {
    display: flex;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 2px;
  }
  .segmented button {
    flex: 1;
    border: none;
    background: none;
  }
  .segmented button.active {
    background: var(--panel);
    box-shadow: var(--shadow);
    font-weight: 600;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .row input {
    width: 90px;
    text-align: right;
  }
  .hint {
    margin: -4px 0 0;
    font-size: 12px;
  }
  .sized {
    margin: 0;
    font-weight: 600;
  }
  .goals {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .goal {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    text-align: left;
    padding: 8px 10px;
  }
  .goal.active {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, var(--panel));
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .goal span {
    font-size: 12px;
  }
  .advanced summary {
    cursor: pointer;
    color: var(--muted);
  }
  .advanced[open] {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .find,
  .progress {
    /* The main action stays in view while the form scrolls. */
    position: sticky;
    bottom: -16px;
    margin: auto -16px -16px;
    padding: 12px 16px 16px;
    background: var(--panel);
    border-top: 1px solid var(--border);
  }
  .find {
    background: var(--accent);
    color: var(--accent-text);
    border-radius: 0 0 var(--radius) var(--radius);
    font-size: 15px;
    font-weight: 600;
  }
  .progress {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 6px 10px;
    align-items: center;
  }
  .progress progress {
    grid-column: 1 / -1;
    width: 100%;
  }
  .right {
    min-width: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-right: 2px;
  }
  .meaning h2 {
    margin: 2px 0 8px;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 10px;
  }
  .card {
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    border-left: 4px solid var(--border);
  }
  .card.ok {
    border-left-color: var(--ok);
  }
  .card.info {
    border-left-color: var(--accent);
  }
  .card.warning {
    border-left-color: var(--warning);
  }
  .card.danger {
    border-left-color: var(--danger);
  }
  .card.sketch {
    grid-column: span 2;
  }
  .card header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    font-size: 12px;
  }
  .card p {
    margin: 0;
  }
  .value {
    font-size: 22px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.01em;
  }
  .chip-level {
    border-radius: 4px;
    padding: 0 6px;
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
    color: var(--text);
    background: var(--panel-2);
    border: 1px solid var(--border);
  }
  .chip-level.ok {
    color: var(--ok);
    border-color: color-mix(in srgb, var(--ok) 40%, transparent);
  }
  .chip-level.info {
    color: var(--accent);
  }
  .chip-level.warning {
    color: var(--warning);
    border-color: color-mix(in srgb, var(--warning) 40%, transparent);
  }
  .chip-level.danger {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 40%, transparent);
  }
  .callout {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }
  .callout.danger {
    border-color: color-mix(in srgb, var(--danger) 50%, transparent);
    background: color-mix(in srgb, var(--danger) 8%, var(--panel));
  }
</style>
