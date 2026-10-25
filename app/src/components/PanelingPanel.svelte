<script lang="ts">
  import type { GeometryView, PanelingOptions } from "../lib/bindings";
  import { app } from "../lib/state.svelte";

  let { geometry }: { geometry: GeometryView } = $props();

  const DEFAULTS: PanelingOptions = {
    n_nodes: 160,
    curvature_bunching: 1,
    te_le_density_ratio: 0.15,
    refined_density_ratio: 0.2,
    top_refinement: [1, 1],
    bottom_refinement: [1, 1],
  };
  const XFOIL_6996: PanelingOptions = { ...DEFAULTS, n_nodes: 240, te_le_density_ratio: 0.3 };

  const p = $derived(geometry.paneling);

  function update(patch: Partial<PanelingOptions>) {
    app.setPaneling(geometry.id, { ...$state.snapshot(p), ...patch } as PanelingOptions);
  }

  function value(e: Event): number {
    return Number((e.currentTarget as HTMLInputElement).value);
  }
</script>

<section class="panel paneling">
  <h2>Paneling <span class="muted">(XFOIL PANGEN)</span></h2>

  <label>
    <span>Nodes <b class="mono">{p.n_nodes}</b></span>
    <input type="range" min="40" max="480" step="10" value={p.n_nodes} oninput={(e) => update({ n_nodes: value(e) })} />
  </label>
  <label>
    <span>Curvature bunching <b class="mono">{(p.curvature_bunching ?? 0).toFixed(2)}</b></span>
    <input type="range" min="0" max="2" step="0.05" value={p.curvature_bunching} oninput={(e) => update({ curvature_bunching: value(e) })} />
  </label>
  <label>
    <span>TE/LE density ratio <b class="mono">{(p.te_le_density_ratio ?? 0).toFixed(2)}</b></span>
    <input type="range" min="0.05" max="1" step="0.05" value={p.te_le_density_ratio} oninput={(e) => update({ te_le_density_ratio: value(e) })} />
  </label>
  <label>
    <span>Refined-area/LE ratio <b class="mono">{(p.refined_density_ratio ?? 0).toFixed(2)}</b></span>
    <input type="range" min="0.05" max="1" step="0.05" value={p.refined_density_ratio} oninput={(e) => update({ refined_density_ratio: value(e) })} />
  </label>
  <div class="zones">
    <span class="muted">Refined zone x/c</span>
    <span>top</span>
    <input type="number" min="0" max="1" step="0.05" value={p.top_refinement?.[0]} onchange={(e) => update({ top_refinement: [value(e), p.top_refinement?.[1] ?? 1] })} />
    <input type="number" min="0" max="1" step="0.05" value={p.top_refinement?.[1]} onchange={(e) => update({ top_refinement: [p.top_refinement?.[0] ?? 1, value(e)] })} />
    <span></span>
    <span>bottom</span>
    <input type="number" min="0" max="1" step="0.05" value={p.bottom_refinement?.[0]} onchange={(e) => update({ bottom_refinement: [value(e), p.bottom_refinement?.[1] ?? 1] })} />
    <input type="number" min="0" max="1" step="0.05" value={p.bottom_refinement?.[1]} onchange={(e) => update({ bottom_refinement: [p.bottom_refinement?.[0] ?? 1, value(e)] })} />
  </div>
  <div class="buttons">
    <button onclick={() => update(DEFAULTS)}>Defaults (160)</button>
    <button onclick={() => update(XFOIL_6996)}>XFOIL 6.996 (240)</button>
  </div>
</section>

<style>
  .paneling {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  label span {
    display: flex;
    justify-content: space-between;
    color: var(--muted);
  }
  label b {
    color: var(--text);
    font-weight: 500;
  }
  input[type="range"] {
    width: 100%;
    accent-color: var(--accent);
  }
  .zones {
    display: grid;
    grid-template-columns: auto auto 1fr 1fr;
    gap: 4px 6px;
    align-items: center;
  }
  .zones > .muted {
    grid-column: 1 / -1;
  }
  .zones input {
    width: 100%;
  }
  .buttons {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
</style>
