<script lang="ts">
  // A labelled number input with a unit; the value is stored in SI units
  // and shown multiplied by `scale` (e.g. metres shown in centimetres).
  import type { Snippet } from "svelte";

  interface Props {
    label: Snippet | string;
    value: number | null | undefined;
    unit?: string;
    /** Displayed value = value × scale. */
    scale?: number;
    step?: number;
    min?: number;
    max?: number;
    /** Decimals shown. */
    digits?: number;
    /** Accept zero and negative values. */
    signed?: boolean;
    onchange: (value: number) => void;
  }

  let { label, value, unit = "", scale = 1, step = 1, min, max, digits = 2, signed = false, onchange }: Props =
    $props();

  const shown = $derived(
    value === null || value === undefined ? "" : Number((value * scale).toFixed(digits)),
  );

  function input(e: Event) {
    const v = (e.currentTarget as HTMLInputElement).valueAsNumber;
    if (!Number.isFinite(v) || (!signed && v <= 0)) return;
    if (min !== undefined && v < min) return;
    if (max !== undefined && v > max) return;
    onchange(v / scale);
  }
</script>

<label class="row">
  <span class="label">{#if typeof label === "string"}{label}{:else}{@render label()}{/if}</span>
  <span class="field">
    <input type="number" {step} {min} {max} value={shown} oninput={input} />
    <span class="unit">{unit}</span>
  </span>
</label>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .label {
    min-width: 0;
  }
  .field {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
  }
  input {
    width: 84px;
    text-align: right;
  }
  .unit {
    color: var(--muted);
    min-width: 26px;
  }
</style>
