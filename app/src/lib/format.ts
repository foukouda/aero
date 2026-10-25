// Number formatting for the interface. Backend floats are typed
// `number | null` (serde writes NaN and infinities as null).

/** Fixed-point formatting, "—" for missing or non-finite values. */
export function fixed(value: number | null | undefined, digits = 4): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  const text = value.toFixed(digits);
  // Avoid "-0.0000".
  return /^-0\.?0*$/.test(text) ? text.slice(1) : text;
}

/** Percentage of chord, e.g. 0.1203 -> "12.03 %". */
export function percent(value: number | null | undefined, digits = 2): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  return `${(value * 100).toFixed(digits)} %`;
}

/** Replaces null entries by NaN (for numeric computations). */
export function numbers(values: (number | null)[]): number[] {
  return values.map((v) => (v === null ? Number.NaN : v));
}
