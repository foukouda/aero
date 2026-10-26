<script lang="ts">
  // Small drawing of an airfoil outline (recommendation cards).
  interface Props {
    x: (number | null)[];
    y: (number | null)[];
    color: string;
    width?: number;
  }

  let { x, y, color, width = 150 }: Props = $props();

  const view = $derived.by(() => {
    const pts = x
      .map((xi, i) => [xi, y[i]] as const)
      .filter((p): p is readonly [number, number] => p[0] !== null && p[1] !== null);
    if (pts.length < 3) return null;
    const xs = pts.map((p) => p[0]);
    const ys = pts.map((p) => p[1]);
    const x0 = Math.min(...xs);
    const x1 = Math.max(...xs);
    const y0 = Math.min(...ys);
    const y1 = Math.max(...ys);
    const pad = 0.04 * (x1 - x0);
    const points = pts.map(([xi, yi]) => `${xi},${-yi}`).join(" ");
    return { box: `${x0 - pad} ${-y1 - pad} ${x1 - x0 + 2 * pad} ${y1 - y0 + 2 * pad}`, points };
  });
</script>

{#if view}
  <svg viewBox={view.box} {width} preserveAspectRatio="xMidYMid meet" role="img" aria-hidden="true">
    <polygon
      points={view.points}
      fill={color}
      fill-opacity="0.15"
      stroke={color}
      stroke-width="1.5"
      vector-effect="non-scaling-stroke"
      stroke-linejoin="round"
    />
  </svg>
{/if}
