<script lang="ts">
  import Term from "./Term.svelte";
  import type { PlotCurve } from "./Plot.svelte";
  import PlotPanel from "./PlotPanel.svelte";
  import type { BoundaryLayer } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import { numbers } from "../lib/format";

  type Quantity = "dstar" | "theta" | "h" | "cf" | "n" | "ue";

  const QUANTITIES: { key: Quantity; label: string; axis: string; wake: boolean }[] = [
    { key: "dstar", label: "Displacement thickness δ*", axis: "δ*/c", wake: true },
    { key: "theta", label: "Momentum thickness θ", axis: "θ/c", wake: true },
    { key: "h", label: "Shape factor H", axis: "H = δ*/θ", wake: true },
    { key: "cf", label: "Skin friction Cf", axis: "Cf", wake: false },
    { key: "n", label: "Amplification N", axis: "N", wake: false },
    { key: "ue", label: "Edge velocity Ue/V∞", axis: "Ue/V∞", wake: true },
  ];

  let quantity = $state<Quantity>("dstar");
  const q = $derived(QUANTITIES.find((x) => x.key === quantity) ?? QUANTITIES[0]);

  function values(b: BoundaryLayer): (number | null)[] {
    switch (quantity) {
      case "dstar":
        return b.dstar;
      case "theta":
        return b.theta;
      case "h":
        return b.h;
      case "cf":
        return b.cf;
      case "n":
        return b.amplification;
      case "ue":
        return b.ue;
    }
  }

  const curves = $derived.by(() => {
    const out: PlotCurve[] = [];
    for (const a of app.visible) {
      const v = app.analyses[a.id]?.viscous;
      if (!v) continue;
      out.push({ label: a.name, color: a.color, x: numbers(v.top.x), y: values(v.top) });
      out.push({
        label: `${a.name} bottom`,
        color: a.color,
        x: numbers(v.bottom.x),
        y: values(v.bottom),
        dash: [6, 4],
        hideInLegend: true,
      });
      if (q.wake) {
        out.push({
          label: `${a.name} wake`,
          color: a.color,
          x: numbers(v.wake.x),
          y: values(v.wake),
          dash: [2, 3],
          hideInLegend: true,
        });
      }
    }
    return out;
  });
</script>

<div class="bl" data-tour="bl-plot">
  <label class="select">
    <span class="muted"><Term id="boundary-layer">Boundary layer</Term></span>
    <select bind:value={quantity} aria-label="Boundary-layer quantity">
      {#each QUANTITIES as item (item.key)}
        <option value={item.key}>{item.label}</option>
      {/each}
    </select>
  </label>
  <PlotPanel
    title={`${q.label} — top solid, bottom dashed${q.wake ? ", wake dotted" : ""}`}
    fileName={`bl_${quantity}`}
    {curves}
    xLabel="x"
    yLabel={q.axis}
    emptyMessage="Enable the viscous analysis to see the boundary layer"
  />
</div>

<style>
  .bl {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
  }
  .bl :global(.plot-panel) {
    flex: 1;
  }
  .select {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 0 6px;
  }
</style>
