<script lang="ts">
  // Top view of the wing to scale, next to a familiar object: an A4 sheet
  // for a model, a standing person for a full-size wing.
  import { app } from "../lib/state.svelte";
  import { length } from "../lib/mission";

  interface Props {
    span: number;
    chord: number;
  }

  let { span, chord }: Props = $props();

  const W = 340;
  const H = 120;
  const GAP_PX = 28;

  const drawing = $derived.by(() => {
    if (!(span > 0 && chord > 0)) return null;
    const person = span > 3;
    // Reference object, metres: the sheet lies flat, the person stands.
    const ref = person ? { w: 0.45, h: 1.75 } : { w: 0.297, h: 0.21 };
    const scale = Math.min((W - GAP_PX - 8) / (span + ref.w), (H - 44) / Math.max(chord, ref.h));
    const wing = { w: span * scale, h: chord * scale };
    const r = { w: ref.w * scale, h: ref.h * scale };
    const top = 8;
    const base = top + Math.max(wing.h, r.h);
    return {
      person,
      wing: { x: 4, y: top + (base - top - wing.h) / 2, ...wing },
      ref: { x: 4 + wing.w + GAP_PX, y: base - r.h, ...r },
      base,
    };
  });
</script>

{#if drawing}
  {@const d = drawing}
  <svg viewBox="0 0 {W} {H}" width="100%" role="img" aria-label={app.t("sketch.title")}>
    <rect
      class="wing"
      x={d.wing.x}
      y={d.wing.y}
      width={d.wing.w}
      height={Math.max(d.wing.h, 1)}
      rx={Math.min(4, d.wing.h / 2)}
    />
    <text x={d.wing.x + d.wing.w / 2} y={d.base + 16} text-anchor="middle">{length(span, app.tourLang)}</text>
    <text x={d.wing.x + d.wing.w / 2} y={d.base + 30} text-anchor="middle" class="muted"
      >× {length(chord, app.tourLang)}</text
    >
    {#if d.person}
      <g class="ref">
        <circle cx={d.ref.x + d.ref.w / 2} cy={d.ref.y + d.ref.w * 0.28} r={d.ref.w * 0.26} />
        <rect
          x={d.ref.x + d.ref.w * 0.15}
          y={d.ref.y + d.ref.w * 0.6}
          width={d.ref.w * 0.7}
          height={d.ref.h - d.ref.w * 0.6}
          rx={d.ref.w * 0.2}
        />
      </g>
    {:else}
      <rect class="ref" x={d.ref.x} y={d.ref.y} width={d.ref.w} height={d.ref.h} />
    {/if}
    <text x={d.ref.x + d.ref.w / 2} y={d.base + 16} text-anchor="middle" class="muted"
      >{app.t(d.person ? "sketch.person" : "sketch.a4")}</text
    >
  </svg>
{/if}

<style>
  svg {
    display: block;
    max-width: 340px;
  }
  .wing {
    fill: color-mix(in srgb, var(--accent) 22%, transparent);
    stroke: var(--accent);
    stroke-width: 1.5;
  }
  .ref,
  .ref circle,
  .ref rect {
    fill: color-mix(in srgb, var(--muted) 18%, transparent);
    stroke: var(--muted);
    stroke-width: 1;
  }
  text {
    font-size: 13px;
    fill: var(--text);
  }
  text.muted {
    fill: var(--muted);
  }
</style>
