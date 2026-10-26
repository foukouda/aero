<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { compactExp, parseNumber } from "../lib/format";

  interface Props {
    /** Disables the inputs (while a sweep runs). */
    disabled?: boolean;
    /** Anchor for the guided tour. */
    tour?: string;
  }

  let { disabled = false, tour }: Props = $props();

  const v = $derived(app.viscous);
  const off = $derived(disabled || !v.enabled);

  /** Applies a parsed value, or restores the field when it is invalid. */
  function apply(e: Event, valid: (x: number) => boolean, set: (x: number) => void, shown: string) {
    const input = e.currentTarget as HTMLInputElement;
    const x = parseNumber(input.value);
    if (x !== null && valid(x)) set(x);
    else input.value = shown;
  }

  function pair(values: [number | null, number | null], side: 0 | 1, x: number): [number, number] {
    const out: [number, number] = [values[0] ?? 0, values[1] ?? 0];
    out[side] = x;
    return out;
  }
</script>

<div class="viscous" data-tour={tour}>
  <label class="check" title="Boundary layer, transition and drag (XFOIL viscous analysis)">
    <input
      type="checkbox"
      checked={v.enabled}
      {disabled}
      onchange={(e) => app.updateViscous({ enabled: e.currentTarget.checked })}
    />
    Viscous
  </label>
  <label title="Chord Reynolds number, e.g. 1e6, 500k, 3 000 000">
    Re
    <input
      class="re"
      type="text"
      inputmode="decimal"
      value={compactExp(v.reynolds)}
      disabled={off}
      onchange={(e) =>
        apply(e, (x) => x > 0, (x) => app.updateViscous({ reynolds: x }), compactExp(v.reynolds))}
    />
  </label>
  <label title="Critical amplification ratio of the eᴺ transition criterion (top / bottom); 9 = average wind tunnel">
    Ncrit
    {#each [0, 1] as const as side (side)}
      <input
        class="small"
        type="text"
        inputmode="decimal"
        aria-label={side === 0 ? "Ncrit top" : "Ncrit bottom"}
        value={v.ncrit[side]}
        disabled={off}
        onchange={(e) =>
          apply(
            e,
            (x) => x > 0 && x < 50,
            (x) => app.updateViscous({ ncrit: pair(v.ncrit, side, x) }),
            String(v.ncrit[side]),
          )}
      />
    {/each}
  </label>
  <label title="Forced transition (trip) x/c on the top / bottom surface; 1 = free transition">
    Trip x/c
    {#each [0, 1] as const as side (side)}
      <input
        class="small"
        type="text"
        inputmode="decimal"
        aria-label={side === 0 ? "Trip top" : "Trip bottom"}
        value={v.xtr[side]}
        disabled={off}
        onchange={(e) =>
          apply(
            e,
            (x) => x >= 0,
            (x) => app.updateViscous({ xtr: pair(v.xtr, side, Math.min(x, 1)) }),
            String(v.xtr[side]),
          )}
      />
    {/each}
  </label>
</div>

<style>
  .viscous {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  label {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--muted);
  }
  .check {
    color: var(--text);
    font-weight: 500;
  }
  .re {
    width: 70px;
  }
  .small {
    width: 42px;
  }
</style>
