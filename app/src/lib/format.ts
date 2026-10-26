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

/**
 * Compact display of a large number such as a Reynolds number:
 * 1000000 -> "1e6", 350000 -> "3.5e5". Small numbers are shown as is.
 */
export function compactExp(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  if (Math.abs(value) < 1e4) return String(value);
  const exponent = Math.floor(Math.log10(Math.abs(value)));
  const mantissa = Number((value / 10 ** exponent).toPrecision(4));
  return `${mantissa}e${exponent}`;
}

/**
 * Parses a number typed by the user: "1e6", "1 000 000", "1_000_000",
 * "500k", "1.5M", "2,5" (decimal comma). Returns null if it is not a number.
 */
export function parseNumber(text: string): number | null {
  let t = text.trim().replace(/[\s_  ]/g, "");
  if (/^-?\d+,\d+$/.test(t)) t = t.replace(",", ".");
  const m = /^(.*?)([kKmM])$/.exec(t);
  let factor = 1;
  if (m) {
    t = m[1];
    factor = m[2].toLowerCase() === "k" ? 1e3 : 1e6;
  }
  if (t === "" || !/^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/.test(t)) return null;
  const value = Number(t) * factor;
  return Number.isFinite(value) ? value : null;
}
