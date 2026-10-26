<script lang="ts">
  // "My aircraft": the flight in everyday units, the aircraft built part by
  // part and shown in 3D, what its numbers mean, and the recommended
  // airfoils (MissionResults).
  import Term from "./Term.svelte";
  import NumberField from "./NumberField.svelte";
  import AircraftViewer from "./AircraftViewer.svelte";
  import SurfaceEditor from "./SurfaceEditor.svelte";
  import MissionResults from "./MissionResults.svelte";
  import type { MissionSetup, PartKind, SurfaceParams } from "../lib/bindings";
  import { defaultSurface } from "../lib/aircraft";
  import { app } from "../lib/state.svelte";
  import type { MessageKey } from "../lib/i18n";
  import {
    GOALS,
    PRESETS,
    TURBULENCE,
    length,
    num,
    readCl,
    readFinVolume,
    readMach,
    readReynolds,
    readTailVolume,
    readWingLoading,
    sig,
    wingLoadingGdm2,
    type Level,
  } from "../lib/mission";

  type Part = "wing" | "canard" | "tail" | "fin";

  const setup = $derived(app.mission);
  const view = $derived(app.missionView);
  const c = $derived(app.missionConditions);
  const lang = $derived(app.tourLang);
  let massUnit = $state<"g" | "kg">("g");
  let unitChosen = false;
  /** Panel opened by a click in the 3D view (or by hand). */
  let openPart = $state<PartKind | "cg" | null>("wing");

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

  /** Replaces a surface. */
  function setSurface(part: Part, surface: SurfaceParams) {
    update((s) => {
      if (part === "wing") s.aircraft.wing = surface;
      else s.aircraft[part] = surface;
    });
  }

  /** Adds a part proportional to the wing, or removes it. */
  function toggle(part: "canard" | "tail" | "fin" | "fuselage", on: boolean) {
    update((s) => {
      if (!on) {
        s.aircraft[part] = null;
      } else if (part === "fuselage") {
        const span = 2 * (s.aircraft.wing.sections[s.aircraft.wing.sections.length - 1]?.y ?? 0.5);
        s.aircraft.fuselage = { length: 0.8 * span, diameter: 0.08 * span };
      } else {
        s.aircraft[part] = defaultSurface(part, s.aircraft.wing);
      }
    });
  }

  function read(e: Event, signed = false): number | null {
    const v = (e.currentTarget as HTMLInputElement).valueAsNumber;
    return Number.isFinite(v) && (signed || v > 0) ? v : null;
  }

  function round(v: number, digits: number): number {
    return Number(v.toFixed(digits));
  }

  function select(kind: PartKind) {
    openPart = kind;
    queueMicrotask(() =>
      document.getElementById(`part-${kind}`)?.scrollIntoView({ block: "nearest", behavior: "smooth" }),
    );
  }

  function opened(kind: PartKind | "cg") {
    return (e: Event) => {
      if ((e.currentTarget as HTMLDetailsElement).open) openPart = kind;
    };
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
  const tailReading = $derived(view ? readTailVolume(view.layout.tail_volume) : null);
  const finReading = $derived(
    view ? readFinVolume(view.layout.fin_volume, view.aircraft.tail === null && view.aircraft.canard === null) : null,
  );
  const seaLevelDensity = 1.225;
</script>


{#if setup}
  {@const aircraft = setup.aircraft}
  <div class="mission">
    <section class="panel form" aria-labelledby="mission-title">
      <h1 id="mission-title">{app.t("mission.title")}</h1>
      <p class="muted intro">{app.t("mission.intro")}</p>

      <div class="presets" role="group" aria-label={app.t("aircraft.presets")}>
        <span class="muted">{app.t("aircraft.presets")}</span>
        {#each PRESETS as p (p.aircraft)}
          <button class="chip" onclick={() => app.applyPreset(p)}>{app.t(p.label)}</button>
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

      <h2 class="section-title">{app.t("aircraft.title")}</h2>

      <details id="part-wing" class="part" open={openPart === "wing"} ontoggle={opened("wing")}>
        <summary>{app.t("part.wing")}</summary>
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
        {#if setup.sizing_cl !== null}
          <NumberField
            label={app.t("mission.sizingCl")}
            value={setup.sizing_cl}
            step={0.05}
            max={2.5}
            onchange={(v) => update((s) => (s.sizing_cl = v))}
          />
          <p class="muted hint">{app.t("mission.sizingHint")}</p>
          {#if view}
            <p class="sized">
              {app.t("mission.sizedWing", {
                root: length(view.aircraft.wing.sections[0]?.chord, lang),
                tip: length(view.aircraft.wing.sections[view.aircraft.wing.sections.length - 1]?.chord, lang),
                area: `${sig(view.layout.wing.area, lang, 3)} m²`,
              })}
            </p>
          {/if}
        {/if}
        <SurfaceEditor
          kind="wing"
          surface={setup.sizing_cl !== null && view ? view.aircraft.wing : aircraft.wing}
          chordsLocked={setup.sizing_cl !== null}
          onchange={(w) => setSurface("wing", w)}
        />
      </details>

      <details id="part-canard" class="part" open={openPart === "canard"} ontoggle={opened("canard")}>
        <summary>{app.t("part.canard")}</summary>
        <label class="check">
          <input
            type="checkbox"
            checked={aircraft.canard !== null}
            onchange={(e) => toggle("canard", (e.currentTarget as HTMLInputElement).checked)}
          />
          {app.t("part.canard.toggle")}
        </label>
        {#if aircraft.canard}
          <SurfaceEditor kind="canard" surface={aircraft.canard} onchange={(c) => setSurface("canard", c)} />
        {/if}
      </details>

      <details id="part-tail" class="part" open={openPart === "tail"} ontoggle={opened("tail")}>
        <summary>{app.t("part.tail")}</summary>
        <label class="check">
          <input
            type="checkbox"
            checked={aircraft.tail !== null}
            onchange={(e) => toggle("tail", (e.currentTarget as HTMLInputElement).checked)}
          />
          {app.t("part.tail.toggle")}
        </label>
        {#if aircraft.tail}
          <SurfaceEditor kind="tail" surface={aircraft.tail} onchange={(t) => setSurface("tail", t)} />
        {/if}
      </details>

      <details id="part-fin" class="part" open={openPart === "fin"} ontoggle={opened("fin")}>
        <summary>{app.t("part.fin")}</summary>
        <label class="check">
          <input
            type="checkbox"
            checked={aircraft.fin !== null}
            onchange={(e) => toggle("fin", (e.currentTarget as HTMLInputElement).checked)}
          />
          {app.t("part.fin.toggle")}
        </label>
        {#if aircraft.fin}
          <SurfaceEditor kind="fin" surface={aircraft.fin} onchange={(f) => setSurface("fin", f)} />
        {/if}
      </details>

      <details id="part-fuselage" class="part" open={openPart === "fuselage"} ontoggle={opened("fuselage")}>
        <summary>{app.t("part.fuselage")}</summary>
        <label class="check">
          <input
            type="checkbox"
            checked={aircraft.fuselage !== null}
            onchange={(e) => toggle("fuselage", (e.currentTarget as HTMLInputElement).checked)}
          />
          {app.t("part.fuselage.toggle")}
        </label>
        {#if aircraft.fuselage}
          <NumberField
            label={app.t("field.length")}
            value={aircraft.fuselage.length}
            unit="m"
            step={0.05}
            digits={3}
            onchange={(v) =>
              update((s) => {
                if (s.aircraft.fuselage) s.aircraft.fuselage.length = v;
              })}
          />
          <NumberField
            label={app.t("field.diameter")}
            value={aircraft.fuselage.diameter}
            unit="cm"
            scale={100}
            digits={1}
            onchange={(v) =>
              update((s) => {
                if (s.aircraft.fuselage) s.aircraft.fuselage.diameter = v;
              })}
          />
        {/if}
      </details>

      <details id="part-cg" class="part" open={openPart === "cg"} ontoggle={opened("cg")}>
        <summary>{app.t("part.cg")}</summary>
        <NumberField
          label={app.t("field.cg")}
          value={aircraft.cg_mac_fraction}
          unit="%"
          scale={100}
          digits={0}
          signed
          min={-50}
          max={100}
          onchange={(v) => update((s) => (s.aircraft.cg_mac_fraction = v))}
        />
        <p class="muted hint">{app.t("cg.hint")}</p>
      </details>

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
        <NumberField
          label={app.t("mission.temperature")}
          value={setup.flight.temperature_offset ?? 0}
          signed
          min={-60}
          max={60}
          step={5}
          digits={0}
          onchange={(v) => update((s) => (s.flight.temperature_offset = v))}
        />
        <p class="muted hint">{app.t("mission.temperatureHint")}</p>
        <NumberField
          label={app.t("mission.spanEfficiency")}
          value={setup.model.span_efficiency}
          step={0.05}
          max={1}
          onchange={(v) => update((s) => (s.model.span_efficiency = v))}
        />
        <NumberField
          label={app.t("mission.clMaxFactor")}
          value={setup.model.cl_max_factor}
          step={0.05}
          max={1}
          onchange={(v) => update((s) => (s.model.cl_max_factor = v))}
        />
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
      <section class="panel viewer-panel" aria-label={app.t("aircraft.title")}>
        {#if view}
          <AircraftViewer
            parts={view.parts}
            layout={view.layout}
            wingZ={view.aircraft.wing.z ?? 0}
            selected={openPart === "cg" ? null : openPart}
            onselect={select}
          />
        {/if}
        {#if app.missionError}
          <p class="callout danger overlay">{app.t("mission.flightError", { message: app.missionError })}</p>
        {/if}
      </section>

      <section class="meaning" aria-labelledby="meaning-title">
        <h2 id="meaning-title">{app.t("card.title")}</h2>
        {#if c && view}
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
            {#if tailReading && finReading}
              {@const worst = [tailReading, finReading].some((r) => r.level === "warning") ? "warning" : "ok"}
              <article class="panel card {worst}">
                <header><Term id="tail-volume">{app.t("card.tails")}</Term></header>
                {#if view.layout.canard_volume !== null}
                  <p>{app.t("canard.text", { value: num(view.layout.canard_volume, lang, 2) })}</p>
                {/if}
                {#if view.layout.tail_volume !== null || view.layout.canard_volume === null}
                  <p>{app.t(tailReading.text, { value: num(view.layout.tail_volume, lang, 2) })}</p>
                {/if}
                <p>{app.t(finReading.text, { value: num(view.layout.fin_volume, lang, 3) })}</p>
              </article>
            {/if}
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
    gap: 12px;
  }
  h1 {
    font-size: 18px;
    margin: 0;
    letter-spacing: -0.01em;
  }
  .section-title {
    font-size: 14px;
    margin: 4px 0 -4px;
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
  .part {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 6px 12px;
  }
  .part[open] {
    padding-bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .part summary {
    cursor: pointer;
    font-weight: 600;
    padding: 2px 0;
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
  .row select {
    max-width: 190px;
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
  .viewer-panel {
    position: relative;
    height: 48vh;
    min-height: 320px;
    flex: none;
    background: linear-gradient(to bottom, var(--panel), var(--panel-2));
  }
  .overlay {
    position: absolute;
    left: 12px;
    right: 12px;
    top: 12px;
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
