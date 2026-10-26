<script lang="ts">
  import { untrack } from "svelte";
  import type { PlotCurve } from "./Plot.svelte";
  import PlotPanel from "./PlotPanel.svelte";
  import Term from "./Term.svelte";
  import type { DesignEdit, DesignMethod } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import { fixed, percent } from "../lib/format";
  import { splitSpeed, strokeEdit } from "../lib/inverse";
  import { minIndex, splitSurfaces } from "../lib/plot/scales";

  // Numbers come as `number | null` from the backend (null for NaN).
  const nums = (a: (number | null)[]) => a.map((v) => v ?? Number.NaN);
  const num = (v: number | null) => v ?? Number.NaN;

  let method = $state<DesignMethod>("mixed");
  let drawing = $state(true);
  let name = $state("");

  const geometry = $derived(app.selectedId !== null ? app.geometries[app.selectedId] : undefined);
  const design = $derived(app.design && geometry && app.design.id === geometry.id ? app.design : null);
  const preview = $derived(design ? app.designPreview : null);

  // (Re)start the design when the airfoil, its paneling or the method change.
  $effect(() => {
    const g = geometry;
    const m = method;
    if (!g) return;
    void g.panel_x;
    untrack(() => {
      const alpha = app.design?.id === g.id ? num(app.design.alpha_deg) : (app.settings?.alpha_deg ?? 2);
      void app.startDesign(g.id, m, alpha);
    });
  });
  $effect(() => () => app.closeDesign());

  const ORIGINAL = "#9ca3af";
  const TARGET = "#2563eb";
  const SEGMENT = "#d946ef";
  const RESULT = "#16a34a";

  const speedCurves = $derived.by(() => {
    const out: PlotCurve[] = [];
    if (!design) return out;
    const pair = (
      label: string,
      color: string,
      x: (number | null)[],
      speed: (number | null)[],
      extra: Partial<PlotCurve> = {},
    ) => {
      const { upper, lower } = splitSpeed(nums(x), nums(speed));
      out.push({ label, color, x: upper.x, y: upper.q, ...extra });
      out.push({ label: `${label} lower`, color, x: lower.x, y: lower.q, hideInLegend: true, ...extra });
    };
    pair("Original airfoil", ORIGINAL, design.current_x, design.current_speed);
    pair("Target", TARGET, design.target_x, design.target, { width: 2 });
    if (design.segment) {
      const [a, b] = design.segment;
      const x = nums(design.target_x.slice(a, b + 1));
      const q = nums(design.target.slice(a, b + 1)).map(Math.abs);
      out.push({ label: "Reshaped segment", color: SEGMENT, x, y: q, width: 3 });
    }
    if (preview?.speed) pair("New airfoil", RESULT, preview.x, preview.speed, { dash: [5, 4] });
    return out;
  });

  const shapeCurves = $derived.by(() => {
    const out: PlotCurve[] = [];
    if (!geometry) return out;
    const shape = (
      label: string,
      color: string,
      xs: (number | null)[],
      ys: (number | null)[],
      extra: Partial<PlotCurve> = {},
    ) => {
      const x = nums(xs);
      const { upper, lower } = splitSurfaces(x, nums(ys), minIndex(x));
      out.push({ label, color, x: upper.x, y: upper.y, ...extra });
      out.push({ label: `${label} lower`, color, x: lower.x, y: lower.y, hideInLegend: true, ...extra });
    };
    shape(geometry.name, geometry.color, geometry.x, geometry.y);
    if (preview) shape("New airfoil", SEGMENT, preview.x, preview.y, { dash: [5, 4] });
    return out;
  });

  function onDraw(points: [number, number][]) {
    if (!design) return;
    const e = strokeEdit(nums(design.target_x), nums(design.target), points);
    if (e) void app.editDesign({ kind: "target", first: e.first, speed: e.speed });
  }

  function edit(e: DesignEdit) {
    void app.editDesign(e);
  }

  /** Value of a number input, or null if it is not a finite number. */
  function numberOf(e: Event): number | null {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    return Number.isFinite(v) ? v : null;
  }
</script>

<div class="design">
  <div class="plots">
    <PlotPanel
      title="Surface speed q/V∞"
      fileName="inverse-speed"
      curves={speedCurves}
      xLabel="x"
      yLabel="q/V∞"
      draw={drawing && design !== null}
      {onDraw}
      tour="design-speed"
      emptyMessage="Select an airfoil to redesign it from its surface speed"
    />
    <PlotPanel
      title="Shape"
      fileName="inverse-shape"
      curves={shapeCurves}
      xLabel="x"
      yLabel="y"
      equal
      emptyMessage="Select an airfoil in the library"
    />
  </div>

  <div class="side">
    <section class="panel controls" data-tour="design">
      <h2><Term id="inverse-design">Inverse design</Term></h2>
      {#if !geometry}
        <p class="muted">Select an airfoil in the library.</p>
      {:else}
        <div class="segmented" role="group" aria-label="Method">
          <button
            class:active={method === "mixed"}
            title="Reshape the part of the surface where the target changes (XFOIL QDES)"
            onclick={() => (method = "mixed")}>Mixed</button
          >
          <button
            class:active={method === "full"}
            title="Generate the whole airfoil from the target (XFOIL MDES)"
            onclick={() => (method = "full")}>Full</button
          >
        </div>
        <p class="muted small">
          {#if method === "mixed"}
            Only the segment where you change the <Term id="target-speed">target speed</Term> is reshaped; the rest of
            the airfoil is kept.
          {:else}
            The whole airfoil is generated from the <Term id="target-speed">target speed</Term> by conformal mapping.
          {/if}
        </p>

        {#if design}
          <div class="grid">
            <label>
              <span><Term id="alpha">α</Term> (°)</span>
              <input
                type="number"
                step="0.5"
                value={Number(num(design.alpha_deg).toFixed(3))}
                disabled={design.by_lift}
                title={design.by_lift ? "Solved for the lift coefficient" : "Angle of attack of the design"}
                onchange={(e) => {
                  const v = numberOf(e);
                  if (v !== null) edit({ kind: "alpha", alpha_deg: v });
                }}
              />
            </label>
            {#if method === "full"}
              <label>
                <span><Term id="cl">Cl</Term> of the target</span>
                <input
                  type="number"
                  step="0.05"
                  value={design.cl === null ? "" : Number(design.cl.toFixed(4))}
                  title="Set a lift coefficient: the angle is solved for"
                  onchange={(e) => {
                    const v = numberOf(e);
                    if (v !== null) edit({ kind: "lift", cl: v });
                  }}
                />
              </label>
            {/if}
          </div>

          <div class="row">
            <button
              class:active={drawing}
              title="Drag over the speed plot near the upper or lower curve to draw a new target there"
              onclick={() => (drawing = !drawing)}>✎ Draw</button
            >
            <button
              disabled={method === "mixed" && !design.segment}
              title={method === "mixed" ? "Smooth the target on the reshaped segment" : "Smooth the whole target"}
              onclick={() => edit({ kind: "smooth" })}>Smooth</button
            >
            {#if method === "full"}
              <button title="Hanning filter of the mapping: removes wiggles" onclick={() => edit({ kind: "filter" })}
                >Filter</button
              >
            {/if}
            <button title="Back to the speed of the original airfoil" onclick={() => edit({ kind: "reset" })}>Reset</button>
          </div>

          {#if method === "mixed"}
            <label class="check" title="Also keeps the curvature of the speed at both ends of the segment (XFOIL CPXX)">
              <input
                type="checkbox"
                checked={design.curvature_constraints}
                onchange={(e) => edit({ kind: "curvature", on: e.currentTarget.checked })}
              />
              Smooth speed at the segment ends
            </label>
          {:else}
            <label class="check">
              <input
                type="checkbox"
                checked={design.symmetric}
                onchange={(e) => edit({ kind: "symmetric", on: e.currentTarget.checked })}
              />
              Symmetric airfoil
            </label>
            <div class="grid">
              <label>
                <span><Term id="trailing-edge">TE gap</Term></span>
                <input
                  type="number"
                  step="0.001"
                  value={Number(num(design.te_gap[1]).toFixed(5))}
                  title="Trailing-edge thickness of the new airfoil (fraction of the chord)"
                  onchange={(e) => {
                    const v = numberOf(e);
                    if (v !== null && design) edit({ kind: "te_gap", dx: num(design.te_gap[0]), dy: v });
                  }}
                />
              </label>
              <label>
                <span>TE angle (°)</span>
                <input
                  type="number"
                  step="1"
                  value={Number(num(design.te_angle_deg).toFixed(2))}
                  title="Trailing-edge angle of the new airfoil"
                  onchange={(e) => {
                    const v = numberOf(e);
                    if (v !== null) edit({ kind: "te_angle", angle_deg: v });
                  }}
                />
              </label>
            </div>
          {/if}
        {/if}

        <p class="muted small status">
          {#if app.designError}
            <span class="error">{app.designError}</span>
          {:else if preview}
            New airfoil (dashed): t/c {percent(preview.properties.max_thickness)}, camber
            {percent(preview.properties.max_camber)}{preview.converged
              ? `, converged in ${preview.iterations} iterations`
              : " — not converged, the result may be inaccurate"}.
          {:else if design && method === "mixed"}
            With ✎ Draw on, drag over the speed plot near the upper or lower curve: the target takes the drawn speed
            there, and that segment of the surface is reshaped to reach it.
          {/if}
        </p>

        <div class="row">
          <input
            type="text"
            placeholder={preview ? `${geometry.name} (${preview.description})` : "Name"}
            bind:value={name}
            aria-label="Name of the new airfoil"
          />
          <button
            class="primary"
            disabled={!preview}
            title="Add the new airfoil to the library; the original is kept"
            onclick={() => app.applyDesign(name)}>Create</button
          >
        </div>
        {#if design?.cl !== null && design?.cl !== undefined}
          <p class="muted small">Target: Cl {fixed(design.cl, 4)} at α {fixed(design.alpha_deg, 2)}°</p>
        {/if}
      {/if}
    </section>
  </div>
</div>

<style>
  .design {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .plots {
    display: grid;
    grid-template-rows: minmax(0, 3fr) minmax(0, 2fr);
    gap: 12px;
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow: auto;
    min-height: 0;
  }
  .controls {
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
    width: 90px;
  }
  .row {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .row input[type="text"] {
    flex: 1;
    min-width: 0;
  }
  .row button.active {
    background: var(--accent);
    color: white;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
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
    padding: 2px 12px;
    background: transparent;
  }
  .segmented button.active {
    background: var(--accent);
    color: white;
  }
</style>
