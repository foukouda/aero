<script lang="ts">
  import Term from "./Term.svelte";
  import type { CamberThickness, GeometryView, Modification } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import { commands } from "../lib/api";
  import { fixed, percent } from "../lib/format";

  let { geometry }: { geometry: GeometryView } = $props();

  type Kind = "flap" | "thickness" | "high" | "te_gap" | "le_radius" | "interpolate";
  let kind = $state<Kind>("flap");

  // Parameters of each operation (kept when switching between them).
  let hingeX = $state(0.75);
  let hingeMode = $state<"relative" | "absolute">("relative");
  let hingeYValue = $state(0.5);
  let deflection = $state(10);
  let series = $state("0, 5, 10, 15, 20");
  let thicknessMode = $state<"scale" | "set">("scale");
  let thicknessFactor = $state(1);
  let camberFactor = $state(1);
  let thicknessValue = $state<number | null>(null);
  let camberValue = $state<number | null>(null);
  let highThickness = $state<number | null>(null);
  let highCamber = $state<number | null>(null);
  let gap = $state(0.005);
  let gapBlend = $state(0.8);
  let gapSplit = $state(0);
  let leFactor = $state(1.5);
  let leBlend = $state(0.2);
  let otherId = $state<number | null>(null);
  let fraction = $state(0.5);
  let name = $state("");

  // Camber and thickness as XFOIL's TSET/HIGH see them (defaults of the inputs).
  let ct = $state<CamberThickness | null>(null);
  $effect(() => {
    const id = geometry.id;
    void geometry.x.length; // refresh when the geometry changes
    void commands.getCamberThickness(id).then((r) => {
      if (r.status === "ok" && id === geometry.id) {
        ct = r.data;
        thicknessValue = r.data.max_thickness === null ? null : Math.round(r.data.max_thickness * 10000) / 10000;
        camberValue = r.data.max_camber === null ? null : Math.round(r.data.max_camber * 10000) / 10000;
        highThickness = r.data.max_thickness_x === null ? null : Math.round(r.data.max_thickness_x * 1000) / 1000;
        highCamber = r.data.max_camber_x === null ? null : Math.round(r.data.max_camber_x * 1000) / 1000;
      }
    });
  });

  const others = $derived(app.airfoils.filter((a) => a.id !== geometry.id));

  const modification = $derived.by((): Modification | null => {
    switch (kind) {
      case "flap":
        return {
          kind: "flap",
          hinge_x: hingeX,
          hinge_y: hingeMode === "relative" ? { kind: "relative", value: hingeYValue } : { kind: "absolute", value: hingeYValue },
          deflection_deg: deflection,
        };
      case "thickness":
        return thicknessMode === "scale"
          ? { kind: "scale_thickness_camber", thickness: thicknessFactor, camber: camberFactor }
          : { kind: "set_thickness_camber", thickness: thicknessValue, camber: camberValue };
      case "high":
        return { kind: "high_points", thickness_x: highThickness, camber_x: highCamber };
      case "te_gap":
        return { kind: "te_gap", gap, blend: gapBlend, split: gapSplit };
      case "le_radius":
        return { kind: "le_radius", factor: leFactor, blend: leBlend };
      case "interpolate":
        return otherId === null ? null : { kind: "interpolate", other: otherId, fraction };
    }
  });

  // Live preview over the shape plot.
  $effect(() => {
    const m = modification;
    if (m) app.previewModification(geometry.id, $state.snapshot(m) as Modification);
    else app.clearPreview();
  });
  $effect(() => () => app.clearPreview());

  const preview = $derived(app.modificationPreview?.id === geometry.id ? app.modificationPreview : null);

  function deflections(): number[] {
    return series
      .split(/[,;\s]+/)
      .filter(Boolean)
      .map(Number)
      .filter(Number.isFinite);
  }
</script>

<section class="panel modify" data-tour="modify">
  <h2><Term id="modify">Modify</Term></h2>
  <select bind:value={kind} aria-label="Modification">
    <option value="flap">Deflect a flap</option>
    <option value="thickness">Thickness and camber</option>
    <option value="high">Move the high points</option>
    <option value="te_gap">Trailing-edge gap</option>
    <option value="le_radius">Leading-edge radius</option>
    <option value="interpolate">Blend with another airfoil</option>
  </select>

  {#if kind === "flap"}
    <div class="grid">
      <label title="Hinge position along the chord">Hinge x <input type="number" step="0.05" bind:value={hingeX} /></label>
      <label>
        <select bind:value={hingeMode} aria-label="Hinge height">
          <option value="relative">Hinge y/t</option>
          <option value="absolute">Hinge y</option>
        </select>
        <input
          type="number"
          step={hingeMode === "relative" ? 0.1 : 0.005}
          bind:value={hingeYValue}
          title={hingeMode === "relative" ? "0 on the lower surface, 1 on the upper surface, 0.5 halfway" : "Ordinate of the hinge"}
        />
      </label>
      <label title="Positive down (trailing edge goes down, more lift)"><Term id="flap">Deflection °</Term> <input type="number" step="1" bind:value={deflection} /></label>
    </div>
  {:else if kind === "thickness"}
    <div class="segmented">
      <button class:active={thicknessMode === "scale"} onclick={() => (thicknessMode = "scale")}>Scale</button>
      <button class:active={thicknessMode === "set"} onclick={() => (thicknessMode = "set")}>Set</button>
    </div>
    <div class="grid">
      {#if thicknessMode === "scale"}
        <label><Term id="thickness">Thickness</Term> × <input type="number" step="0.05" bind:value={thicknessFactor} /></label>
        <label><Term id="camber">Camber</Term> × <input type="number" step="0.1" bind:value={camberFactor} /></label>
      {:else}
        <label><Term id="thickness">Max t/c</Term> <input type="number" step="0.005" bind:value={thicknessValue} /></label>
        <label><Term id="camber">Max camber</Term> <input type="number" step="0.005" bind:value={camberValue} /></label>
      {/if}
    </div>
  {:else if kind === "high"}
    <div class="grid">
      <label title="Position of the maximum thickness">Max thickness at x <input type="number" step="0.05" bind:value={highThickness} /></label>
      <label title="Position of the maximum camber">Max camber at x <input type="number" step="0.05" bind:value={highCamber} /></label>
    </div>
  {:else if kind === "te_gap"}
    <div class="grid">
      <label><Term id="trailing-edge">TE gap</Term> <input type="number" step="0.001" bind:value={gap} /></label>
      <label title="Distance from the trailing edge over which the change is blended (0 to 1)">Blend <input type="number" step="0.1" min="0" max="1" bind:value={gapBlend} /></label>
      <label title="-1: all on the lower surface, 0: shared, 1: all on the upper surface">Top/bottom split <input type="number" step="0.25" min="-1" max="1" bind:value={gapSplit} /></label>
    </div>
  {:else if kind === "le_radius"}
    <div class="grid">
      <label><Term id="le-radius">Radius</Term> × <input type="number" step="0.1" bind:value={leFactor} /></label>
      <label title="Distance from the leading edge over which the change is blended">Blend <input type="number" step="0.05" bind:value={leBlend} /></label>
    </div>
  {:else}
    <div class="grid">
      <label>
        With
        <select bind:value={otherId} aria-label="Other airfoil">
          <option value={null}>choose…</option>
          {#each others as a (a.id)}
            <option value={a.id}>{a.name}</option>
          {/each}
        </select>
      </label>
      <label title="0 keeps this airfoil, 1 gives the other one; outside 0..1 extrapolates">Fraction <input type="number" step="0.1" bind:value={fraction} /></label>
    </div>
  {/if}

  <p class="muted small">
    {#if app.modificationError}
      <span class="error">{app.modificationError}</span>
    {:else if preview}
      Preview (dashed): t/c {percent(preview.preview.properties.max_thickness)}, camber
      {percent(preview.preview.properties.max_camber)}, TE angle {fixed(preview.preview.properties.te_angle_deg, 1)}°
    {:else if ct}
      Now: t/c {percent(ct.max_thickness)} at x {fixed(ct.max_thickness_x, 3)}, camber {percent(ct.max_camber)} at
      x {fixed(ct.max_camber_x, 3)}
    {/if}
  </p>

  <div class="row">
    <input type="text" placeholder={preview ? `${geometry.name} (${preview.preview.description})` : "Name"} bind:value={name} aria-label="Name of the new airfoil" />
    <button
      class="primary"
      disabled={!modification || !preview}
      title="Add the modified airfoil to the library; the original is kept"
      onclick={() => modification && app.applyModification(geometry.id, $state.snapshot(modification) as Modification, name)}
      >Create</button
    >
  </div>
  {#if kind === "flap"}
    <div class="row">
      <label class="series" title="Creates one airfoil per deflection, ready to be swept and compared">
        Series ° <input type="text" bind:value={series} />
      </label>
      <button
        onclick={() =>
          app.flapSeries(
            geometry.id,
            hingeX,
            hingeMode === "relative" ? { kind: "relative", value: hingeYValue } : { kind: "absolute", value: hingeYValue },
            deflections(),
          )}>Create series</button
      >
    </div>
  {/if}
</section>

<style>
  .modify {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
  }
  h2 {
    margin: 0;
  }
  .grid {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .grid label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    color: var(--muted);
  }
  .grid input {
    width: 80px;
  }
  .row {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .row input[type="text"] {
    flex: 1;
    min-width: 0;
  }
  .series {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    color: var(--muted);
  }
  .series input {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .error {
    color: #dc2626;
  }
  .segmented {
    display: inline-flex;
    align-self: flex-start;
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
</style>
