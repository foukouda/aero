import { describe, expect, it } from "vitest";
import { compactExp, fixed, parseNumber, percent } from "./format";

describe("format", () => {
  it("formats fixed and percent values", () => {
    expect(fixed(0.12345)).toBe("0.1235");
    expect(fixed(-0.00001)).toBe("0.0000");
    expect(fixed(null)).toBe("—");
    expect(percent(0.1203)).toBe("12.03 %");
  });

  it("shows large numbers compactly", () => {
    expect(compactExp(1e6)).toBe("1e6");
    expect(compactExp(350000)).toBe("3.5e5");
    expect(compactExp(2.5e6)).toBe("2.5e6");
    expect(compactExp(123456)).toBe("1.235e5");
    expect(compactExp(9000)).toBe("9000");
    expect(compactExp(null)).toBe("—");
  });

  it("parses user input", () => {
    expect(parseNumber("1e6")).toBe(1e6);
    expect(parseNumber(" 1 000 000 ")).toBe(1e6);
    expect(parseNumber("1_000_000")).toBe(1e6);
    expect(parseNumber("500k")).toBe(5e5);
    expect(parseNumber("1.5M")).toBe(1.5e6);
    expect(parseNumber("2,5")).toBe(2.5);
    expect(parseNumber("0.1")).toBe(0.1);
    expect(parseNumber("3.5e5")).toBe(3.5e5);
    expect(parseNumber("")).toBeNull();
    expect(parseNumber("abc")).toBeNull();
    expect(parseNumber("1e6x")).toBeNull();
  });

  it("round-trips compact numbers", () => {
    for (const v of [1e5, 2e5, 3.5e5, 1e6, 6e6, 1e7]) expect(parseNumber(compactExp(v))).toBe(v);
  });
});
