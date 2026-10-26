<script lang="ts">
  import Term from "./Term.svelte";
  import type { GeometryView } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import { fixed, percent } from "../lib/format";

  let { geometry }: { geometry: GeometryView } = $props();
  const p = $derived(geometry.properties);
</script>

<section class="panel props" data-tour="properties">
  <h2>{geometry.name}</h2>
  <dl class="mono">
    <dt><Term id="thickness">Max thickness</Term></dt>
    <dd>{percent(p.max_thickness)} <span class="muted">at x/c {fixed(p.max_thickness_x, 3)}</span></dd>
    <dt><Term id="camber">Max camber</Term></dt>
    <dd>{percent(p.max_camber)} <span class="muted">at x/c {fixed(p.max_camber_x, 3)}</span></dd>
    <dt><Term id="le-radius">LE radius</Term></dt>
    <dd>{percent(p.le_radius, 3)}</dd>
    <dt><Term id="trailing-edge">TE angle</Term></dt>
    <dd>{fixed(p.te_angle_deg, 2)}°</dd>
    <dt><Term id="trailing-edge">TE thickness</Term></dt>
    <dd>{percent(p.te_thickness, 3)}</dd>
    <dt>Area</dt>
    <dd>{fixed(p.area, 5)} c²</dd>
    <dt><Term id="chord">Chord</Term></dt>
    <dd>{fixed(p.chord, 5)}</dd>
    <dt>Trailing edge</dt>
    <dd>{geometry.sharp_trailing_edge ? "sharp" : "blunt"}</dd>
  </dl>
  {#if !geometry.normalized}
    <p class="warning">
      The chord is not 1 or the leading edge is not at the origin: coefficients are referred to a
      unit length in airfoil coordinates, as in XFOIL.
    </p>
  {/if}
  <div class="buttons">
    <button
      title="Translate the leading edge to the origin and scale the chord to 1 (XFOIL NORM)"
      onclick={() => app.normalize(geometry.id, "Xfoil")}>Normalize</button
    >
    <button
      title="Normalize and rotate so that the trailing edge is at (1, 0)"
      onclick={() => app.normalize(geometry.id, "ChordAligned")}>Normalize + derotate</button
    >
  </div>
</section>

<style>
  .props {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    margin: 0;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    text-align: right;
  }
  .warning {
    margin: 0;
    font-size: 12px;
    color: var(--warning);
  }
  .buttons {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
</style>
