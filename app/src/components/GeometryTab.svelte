<script lang="ts">
  import type { PlotCurve } from "./Plot.svelte";
  import PlotPanel from "./PlotPanel.svelte";
  import PropertiesPanel from "./PropertiesPanel.svelte";
  import PanelingPanel from "./PanelingPanel.svelte";
  import ModifyPanel from "./ModifyPanel.svelte";
  import { app } from "../lib/state.svelte";
  import { minIndex, splitSurfaces } from "../lib/plot/scales";

  const showNodes = $derived(app.settings?.show_nodes ?? false);

  const curves = $derived.by(() => {
    const out: PlotCurve[] = [];
    for (const a of app.visible) {
      const g = app.geometries[a.id];
      if (!g) continue;
      const { upper, lower } = splitSurfaces(g.x, g.y, minIndex(g.x));
      out.push({ label: g.name, color: g.color, x: upper.x, y: upper.y });
      out.push({ label: `${g.name} lower`, color: g.color, x: lower.x, y: lower.y, hideInLegend: true });
      if (showNodes) {
        const nodes = splitSurfaces(g.panel_x, g.panel_y, minIndex(g.panel_x));
        out.push({
          label: `${g.name} nodes`,
          color: g.color,
          x: nodes.upper.x,
          y: nodes.upper.y,
          points: true,
          line: false,
          hideInLegend: true,
        });
        out.push({
          label: `${g.name} lower nodes`,
          color: g.color,
          x: nodes.lower.x,
          y: nodes.lower.y,
          points: true,
          line: false,
          hideInLegend: true,
        });
      }
    }
    const p = app.modificationPreview;
    if (p && app.selectedId === p.id) {
      const x = p.preview.x.map((v) => v ?? Number.NaN);
      const y = p.preview.y.map((v) => v ?? Number.NaN);
      const { upper, lower } = splitSurfaces(x, y, minIndex(x));
      out.push({ label: `Preview: ${p.preview.description}`, color: "#d946ef", x: upper.x, y: upper.y, dash: [5, 4] });
      out.push({ label: "preview lower", color: "#d946ef", x: lower.x, y: lower.y, dash: [5, 4], hideInLegend: true });
    }
    return out;
  });

  const geometry = $derived(app.selectedId !== null ? app.geometries[app.selectedId] : undefined);
</script>

<div class="geometry">
  <div class="view" data-tour="shape">
    <PlotPanel
      title="Airfoil shape"
      fileName="airfoils"
      {curves}
      xLabel="x"
      yLabel="y"
      equal
      emptyMessage="Generate or import an airfoil to see its shape"
    />
    <label class="nodes">
      <input
        type="checkbox"
        checked={showNodes}
        onchange={(e) => app.updateSettings({ show_nodes: e.currentTarget.checked })}
      />
      Show panel nodes
    </label>
  </div>
  <div class="side">
    {#if geometry}
      <PropertiesPanel {geometry} />
      <ModifyPanel {geometry} />
      <PanelingPanel {geometry} />
    {:else}
      <section class="panel placeholder muted">Select an airfoil in the library.</section>
    {/if}
  </div>
</div>

<style>
  .geometry {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .view {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .view :global(.plot-panel) {
    flex: 1;
  }
  .nodes {
    position: absolute;
    top: 8px;
    left: 140px;
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow: auto;
    min-height: 0;
  }
  .placeholder {
    padding: 16px;
  }
</style>
