<script lang="ts">
  // Editor of one surface (wing, canard, tail or fin) as a list of sections:
  // the surface's position and incidence, its span, and for each section its
  // position, chord, offset and sweep, height and dihedral, twist and
  // airfoil; sections can be inserted, removed and added at the tip.
  import NumberField from "./NumberField.svelte";
  import type { SectionParams, SurfaceParams } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import {
    addTipSection,
    insertSection,
    panelAngle,
    removeSection,
    setAllAirfoils,
    setPanelAngle,
    setSection,
    setSurfaceSpan,
    surfaceSpan,
    type Axis,
    type SurfaceKind,
  } from "../lib/aircraft";

  interface Props {
    kind: SurfaceKind;
    surface: SurfaceParams;
    /** Chords computed elsewhere (wing sized for a lift coefficient). */
    chordsLocked?: boolean;
    onchange: (surface: SurfaceParams) => void;
  }

  let { kind, surface, chordsLocked = false, onchange }: Props = $props();

  const fin = $derived(kind === "fin");
  const span = $derived(surfaceSpan(surface, kind));
  // Lengths in centimetres for models, metres for full-size aircraft.
  const big = $derived(span > 3);
  const length = $derived(big ? { unit: "m", scale: 1, digits: 3, step: 0.05 } : { unit: "cm", scale: 100, digits: 1, step: 1 });
  const commonAirfoil = $derived(
    surface.sections.every((s) => s.airfoil === surface.sections[0]?.airfoil) ? surface.sections[0]?.airfoil : "",
  );

  function sections(next: SectionParams[]) {
    onchange({ ...surface, sections: next });
  }

  function angle(i: number, axis: Axis, deg: number) {
    sections(setPanelAngle(surface.sections, i, axis, deg));
  }

  /** Airfoil choices: catalogue, library, and the current key otherwise. */
  function choices(current: string) {
    const catalogue = app.catalogue.map((e) => ({ key: `catalogue:${e.id}`, name: e.name }));
    const library = app.airfoils.map((a) => ({ key: `library:${a.id}`, name: a.name }));
    const known = current === "" || [...catalogue, ...library].some((o) => o.key === current);
    const extra = known ? [] : [{ key: current, name: current.replace(/^naca:/, "NACA ") }];
    return { catalogue, library, extra };
  }
</script>

{#snippet airfoilSelect(value: string, onpick: (key: string) => void, several = false)}
  {@const c = choices(value)}
  <select {value} onchange={(e) => onpick((e.currentTarget as HTMLSelectElement).value)}>
    {#if several}<option value="" disabled>{app.t("surface.several")}</option>{/if}
    {#each c.extra as o (o.key)}<option value={o.key}>{o.name}</option>{/each}
    <optgroup label={app.t("airfoil.catalogue")}>
      {#each c.catalogue as o (o.key)}<option value={o.key}>{o.name}</option>{/each}
    </optgroup>
    {#if c.library.length}
      <optgroup label={app.t("airfoil.library")}>
        {#each c.library as o (o.key)}<option value={o.key}>{o.name}</option>{/each}
      </optgroup>
    {/if}
  </select>
{/snippet}

<div class="editor">
  <NumberField
    label={app.t("field.x")}
    value={surface.x}
    unit="m"
    signed
    step={0.01}
    digits={3}
    onchange={(v) => onchange({ ...surface, x: v })}
  />
  <NumberField
    label={app.t("field.z")}
    value={surface.z}
    unit={length.unit}
    scale={length.scale}
    digits={length.digits}
    signed
    onchange={(v) => onchange({ ...surface, z: v })}
  />
  {#if !fin}
    <NumberField
      label={app.t("field.incidence")}
      value={surface.incidence_deg}
      unit="°"
      signed
      min={-15}
      max={15}
      digits={1}
      onchange={(v) => onchange({ ...surface, incidence_deg: v })}
    />
  {/if}
  <NumberField
    label={app.t(fin ? "surface.height" : "surface.span")}
    value={span}
    unit="m"
    step={0.05}
    digits={3}
    onchange={(v) => onchange(setSurfaceSpan(surface, kind, v))}
  />
  <p class="muted hint">{app.t("surface.spanHint")}</p>
  <label class="row">
    <span>{app.t("surface.airfoils")}</span>
    {@render airfoilSelect(commonAirfoil ?? "", (key) => onchange(setAllAirfoils(surface, key)), commonAirfoil === "")}
  </label>

  <ol class="sections">
    {#each surface.sections as s, i (i)}
      <li class="section">
        <header>
          <strong>{i === 0 ? app.t(fin ? "section.rootFin" : "section.root") : app.t("section.title", { n: i + 1 })}</strong>
          {#if i > 0}
            <span class="actions">
              <button class="link" title={app.t("section.insert")} onclick={() => sections(insertSection(surface.sections, i))}
                >＋ {app.t("section.insert")}</button
              >
              {#if surface.sections.length > 2}
                <button class="link danger" onclick={() => sections(removeSection(surface.sections, i))}
                  >✕ {app.t("section.remove")}</button
                >
              {/if}
            </span>
          {/if}
        </header>
        <div class="grid">
          {#if i > 0}
            <NumberField
              stacked
              label={app.t(fin ? "section.yFin" : "section.y")}
              value={s.y}
              unit={length.unit}
              scale={length.scale}
              digits={length.digits}
              step={length.step}
              onchange={(v) => sections(setSection(surface.sections, i, { y: v }))}
            />
          {/if}
          <NumberField
            stacked
            label={app.t("section.chord")}
            value={s.chord}
            unit={length.unit}
            scale={length.scale}
            digits={length.digits}
            step={length.step}
            disabled={chordsLocked}
            onchange={(v) => sections(setSection(surface.sections, i, { chord: v }))}
          />
          {#if !fin}
            <NumberField
              stacked
              label={app.t("section.twist")}
              value={s.twist_deg}
              unit="°"
              signed
              min={-20}
              max={20}
              digits={1}
              step={0.5}
              onchange={(v) => sections(setSection(surface.sections, i, { twist_deg: v }))}
            />
          {/if}
          {#if i > 0}
            <NumberField
              stacked
              label={app.t("section.offset")}
              value={s.x}
              unit={length.unit}
              scale={length.scale}
              digits={length.digits}
              step={length.step}
              signed
              onchange={(v) => sections(setSection(surface.sections, i, { x: v }))}
            />
            <NumberField
              stacked
              label={app.t("section.sweep")}
              value={panelAngle(surface.sections, i, "x")}
              unit="°"
              signed
              min={-70}
              max={70}
              digits={1}
              onchange={(v) => angle(i, "x", v)}
            />
            {#if !fin}
              <NumberField
                stacked
                label={app.t("section.height")}
                value={s.z}
                unit={length.unit}
                scale={length.scale}
                digits={length.digits}
                step={length.step}
                signed
                onchange={(v) => sections(setSection(surface.sections, i, { z: v }))}
              />
              <NumberField
                stacked
                label={app.t("section.dihedral")}
                value={panelAngle(surface.sections, i, "z")}
                unit="°"
                signed
                min={-70}
                max={70}
                digits={1}
                onchange={(v) => angle(i, "z", v)}
              />
            {/if}
          {/if}
        </div>
        <label class="row airfoil">
          <span class="muted">{app.t("field.airfoil")}</span>
          {@render airfoilSelect(s.airfoil, (key) => sections(setSection(surface.sections, i, { airfoil: key })))}
        </label>
      </li>
    {/each}
  </ol>
  <button class="add" onclick={() => sections(addTipSection(surface.sections))}>{app.t("section.add")}</button>
  <p class="muted hint">{app.t("section.hint")}</p>
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .row select {
    max-width: 180px;
    min-width: 0;
  }
  .hint {
    margin: -4px 0 0;
    font-size: 12px;
  }
  .sections {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .section {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px;
    background: var(--panel-2);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .section header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 6px;
    font-size: 12px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .link {
    border: none;
    background: none;
    padding: 0;
    font-size: 11px;
    color: var(--accent);
  }
  .link.danger {
    color: var(--danger);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 6px;
  }
  .airfoil {
    font-size: 12px;
  }
  .add {
    align-self: flex-start;
  }
</style>
