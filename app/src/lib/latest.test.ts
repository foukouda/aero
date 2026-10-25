import { describe, expect, it } from "vitest";
import { latestRunner } from "./latest";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => (resolve = r));
  return { promise, resolve };
}

describe("latestRunner", () => {
  it("runs one task at a time and only the latest pending argument", async () => {
    const seen: number[] = [];
    const gates: ReturnType<typeof deferred>[] = [];
    const run = latestRunner(async (v: number) => {
      seen.push(v);
      const gate = deferred();
      gates.push(gate);
      await gate.promise;
    });

    const done = run(1);
    run(2);
    run(3);
    run(4);
    // Only the first task started.
    expect(seen).toEqual([1]);
    gates[0].resolve();
    await new Promise((r) => setTimeout(r, 0));
    // 2 and 3 were superseded by 4.
    expect(seen).toEqual([1, 4]);
    gates[1].resolve();
    await done;
    expect(seen).toEqual([1, 4]);
  });

  it("keeps working after a failing task", async () => {
    const seen: number[] = [];
    const run = latestRunner(async (v: number) => {
      seen.push(v);
      if (v === 1) throw new Error("boom");
    });
    await run(1);
    await run(2);
    expect(seen).toEqual([1, 2]);
  });
});
