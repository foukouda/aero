<script lang="ts">
  import type { PlotCurve } from "./Plot.svelte";
  import PlotPanel from "./PlotPanel.svelte";
  import type {
    Constraints,
    Evaluation,
    HistoryEntry,
    Method,
    Objective,
    OptimizationSetup,
    Phase,
    Shape,
    Stop,
  } from "../lib/bindings";
  import { app } from "../lib/state.svelte";
  import { compactExp, fixed, numbers, parseNumber, percent } from "../lib/format";
  import {
    bestSoFar,
    displayObjective,
    feasible,
    improvement,
    liftToDrag,
    objectiveName,
    targetLabel,
  } from "../lib/optimize";

  const setup = $derived(app.session?.optimization ?? null);
  const problem = $derived(setup?.problem ?? null);
  const options = $derived(setup?.options ?? null);
  const constraints = $derived(problem?.constraints ?? {});
  const viscous = $derived(problem?.viscous ?? {});
  const running = $derived(app.optimization !== null);

  // Airfoil to optimize: the selected one unless another is chosen.
  let chosenId = $state<number | null>(null);
  const airfoilId = $derived(
    chosenId !== null && app.airfoils.some((a) => a.id === chosenId)
      ? chosenId
      : (app.selectedId ?? app.airfoils[0]?.id ?? null),
  );
  const airfoilThickness = $derived(
    airfoilId !== null ? (app.geometries[airfoilId]?.properties.max_thickness ?? null) : null,
  );

  /** Applies a change to a copy of the setup. */
  function update(change: (s: OptimizationSetup) => void) {
    if (!setup) return;
    const s = $state.snapshot(setup) as OptimizationSetup;
    change(s);
    app.updateOptimizationSetup(s);
  }

  function value(e: Event): number {
    return Number((e.currentTarget as HTMLInputElement).value);
  }

  function finite(e: Event, set: (x: number) => void) {
    const x = value(e);
    if (Number.isFinite(x)) set(x);
  }

  function addPoint() {
    update((s) => {
      const last = s.problem.points.at(-1);
      const target = last ? { ...last.target, value: (last.target.value ?? 0) + 0.2 } : { kind: "cl" as const, value: 0.5 };
      s.problem.points.push({ target, weight: 1 });
    });
  }

  type ConstraintKey = "min_thickness" | "max_thickness" | "min_cm" | "min_te_angle_deg";

  function toggleConstraint(key: ConstraintKey, on: boolean, initial: number) {
    update((s) => {
      const c: Constraints = s.problem.constraints ?? {};
      c[key] = on ? initial : null;
      s.problem.constraints = c;
    });
  }

  function setConstraint(key: ConstraintKey, x: number) {
    update((s) => {
      const c: Constraints = s.problem.constraints ?? {};
      c[key] = x;
      s.problem.constraints = c;
    });
  }

  const constraintRows: { key: ConstraintKey; label: string; initial: () => number; step: number; title: string }[] = [
    {
      key: "min_thickness",
      label: "Min t/c",
      initial: () => Math.round((airfoilThickness ?? 0.12) * 1000) / 1000,
      step: 0.005,
      title: "Smallest allowed maximum thickness",
    },
    { key: "max_thickness", label: "Max t/c", initial: () => 0.2, step: 0.005, title: "Largest allowed maximum thickness" },
    { key: "min_cm", label: "Min Cm", initial: () => -0.1, step: 0.01, title: "Smallest allowed moment coefficient at every point" },
    {
      key: "min_te_angle_deg",
      label: "Min TE angle °",
      initial: () => 5,
      step: 1,
      title: "Smallest allowed trailing-edge included angle",
    },
  ];

  // -------------------------------------------------------------- results

  interface ResultView {
    objective: Objective;
    airfoilName: string;
    initial: Evaluation | null;
    best: Evaluation | null;
    initialShape: Shape | null;
    bestShape: Shape | null;
    history: HistoryEntry[];
    evaluations: number;
    maxEvaluations: number;
    phase: Phase | null;
    stop: Stop | null;
    elapsed: number | null;
    fitRms: number | null;
  }

  const result = $derived.by((): ResultView | null => {
    const run = app.optimization;
    if (run) {
      return {
        objective: run.objective,
        airfoilName: app.airfoils.find((a) => a.id === run.airfoilId)?.name ?? "",
        initial: run.initial,
        best: run.best?.evaluation ?? null,
        initialShape: run.initialShape,
        bestShape: run.best?.shape ?? null,
        history: run.history,
        evaluations: run.evaluations,
        maxEvaluations: run.maxEvaluations,
        phase: run.phase,
        stop: null,
        elapsed: null,
        fitRms: Number.isFinite(run.fitRms) ? run.fitRms : null,
      };
    }
    const s = app.session?.last_optimization;
    if (!s) return null;
    return {
      objective: s.problem.objective ?? "min_drag",
      airfoilName: s.airfoil_name,
      initial: s.initial,
      best: s.best,
      initialShape: s.initial_shape,
      bestShape: s.best_shape,
      history: s.history,
      evaluations: s.history.length,
      maxEvaluations: s.options.max_evaluations ?? s.history.length,
      phase: null,
      stop: s.stop,
      elapsed: s.elapsed_s,
      fitRms: s.fit_rms_error,
    };
  });

  const gain = $derived(
    result ? improvement(result.objective, result.initial?.objective, result.best?.objective) : null,
  );
  const failed = $derived(result ? result.history.filter((h) => h.objective === null).length : 0);

  const shapeCurves = $derived.by((): PlotCurve[] => {
    if (!result) return [];
    const out: PlotCurve[] = [];
    if (result.initialShape) {
      out.push({
        label: `${result.airfoilName} (CST fit)`,
        color: "#9ca3af",
        x: numbers(result.initialShape.x),
        y: result.initialShape.y,
        dash: [6, 4],
      });
    }
    if (result.bestShape) {
      out.push({ label: "Best design", color: "#2563eb", x: numbers(result.bestShape.x), y: result.bestShape.y });
    }
    return out;
  });

  const historyCurves = $derived.by((): PlotCurve[] => {
    if (!result || result.history.length === 0) return [];
    const h = result.history;
    const x = h.map((e) => e.evaluation);
    // Designs much worse than the initial one would squash the scale.
    const initial = displayObjective(result.objective, result.initial?.objective);
    const readable = (v: number | null) =>
      v !== null &&
      (initial === null ||
        (result.objective === "max_lift_to_drag" ? v >= initial - 0.5 * Math.abs(initial) : v <= 1.5 * initial));
    const shown = (keep: (e: HistoryEntry) => boolean) =>
      h.map((e) => {
        const v = keep(e) ? displayObjective(result.objective, e.objective) : null;
        return readable(v) ? v : null;
      });
    return [
      {
        label: "Feasible",
        color: "#16a34a",
        x,
        y: shown(feasible),
        points: true,
        line: false,
      },
      {
        label: "Infeasible",
        color: "#d97706",
        x,
        y: shown((e) => e.objective !== null && !feasible(e)),
        points: true,
        line: false,
      },
      {
        label: "Best",
        color: "#2563eb",
        x,
        y: bestSoFar(h).map((v) => displayObjective(result.objective, v)),
        width: 2,
      },
    ];
  });

  const phaseLabel: Record<Phase, string> = {
    initial: "initial design",
    global: "global search (CMA-ES)",
    local: "local polish (Nelder–Mead)",
  };
  const stopLabel: Record<Stop, string> = {
    max_evaluations: "budget spent",
    converged: "converged",
    stagnation: "no further improvement",
    numerical_limit: "numerical limit",
    cancelled: "cancelled",
  };

  const progress = $derived(
    result && result.maxEvaluations > 0 ? Math.round((100 * result.evaluations) / result.maxEvaluations) : 0,
  );

  // Elapsed time while running.
  let now = $state(performance.now());
  $effect(() => {
    if (!running) return;
    const timer = setInterval(() => (now = performance.now()), 500);
    return () => clearInterval(timer);
  });
  const elapsed = $derived(
    app.optimization ? (now - app.optimization.startedAt) / 1000 : (result?.elapsed ?? null),
  );

  function signed(x: number | null, digits = 1): string {
    if (x === null) return "—";
    return `${x > 0 ? "+" : ""}${x.toFixed(digits)} %`;
  }

  function change(a: number | null | undefined, b: number | null | undefined): number | null {
    if (a === null || a === undefined || b === null || b === undefined || a === 0) return null;
    return 100 * (b / a - 1);
  }
</script>

<div class="optimize">
  <section class="panel setup" data-tour="optimize-setup">
    {#if setup && problem && options}
      <h2>Problem</h2>
      <label class="field">
        Airfoil
        <select
          value={airfoilId}
          disabled={running}
          onchange={(e) => (chosenId = Number(e.currentTarget.value))}
        >
          {#each app.airfoils as a (a.id)}
            <option value={a.id}>{a.name}</option>
          {/each}
        </select>
      </label>
      {#if airfoilThickness !== null}
        <p class="muted small">Its maximum thickness: {percent(airfoilThickness)}</p>
      {/if}

      <h3>Operating points</h3>
      <table class="points">
        <thead><tr><th></th><th>Value</th><th title="Weight in the objective">Weight</th><th></th></tr></thead>
        <tbody>
          {#each problem.points as p, i (i)}
            <tr>
              <td>
                <select
                  value={p.target.kind}
                  disabled={running}
                  onchange={(e) =>
                    update((s) => {
                      s.problem.points[i].target.kind = e.currentTarget.value as "alpha" | "cl";
                    })}
                >
                  <option value="cl">Cl</option>
                  <option value="alpha">α (°)</option>
                </select>
              </td>
              <td>
                <input
                  type="number"
                  step={p.target.kind === "cl" ? 0.05 : 0.5}
                  value={p.target.value}
                  disabled={running}
                  onchange={(e) => finite(e, (x) => update((s) => (s.problem.points[i].target.value = x)))}
                />
              </td>
              <td>
                <input
                  type="number"
                  min="0.01"
                  step="0.5"
                  value={p.weight}
                  disabled={running}
                  onchange={(e) =>
                    finite(e, (x) => x > 0 && update((s) => (s.problem.points[i].weight = x)))}
                />
              </td>
              <td>
                <button
                  class="icon"
                  title="Remove the point"
                  disabled={running || problem.points.length <= 1}
                  onclick={() => update((s) => s.problem.points.splice(i, 1))}>×</button
                >
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <button class="small-button" disabled={running} onclick={addPoint}>+ Point</button>

      <label class="field">
        Objective
        <select
          value={problem.objective ?? "min_drag"}
          disabled={running}
          onchange={(e) => update((s) => (s.problem.objective = e.currentTarget.value as Objective))}
        >
          <option value="min_drag">Minimum drag (weighted Cd)</option>
          <option value="max_lift_to_drag">Maximum L/D (weighted)</option>
        </select>
      </label>

      <h3>Flow</h3>
      <div class="grid2">
        <label title="Chord Reynolds number, e.g. 1e6, 500k">
          Re
          <input
            type="text"
            inputmode="decimal"
            value={compactExp(viscous.reynolds ?? 1e6)}
            disabled={running}
            onchange={(e) => {
              const x = parseNumber(e.currentTarget.value);
              if (x !== null && x > 0) update((s) => (s.problem.viscous = { ...s.problem.viscous, reynolds: x }));
              else e.currentTarget.value = compactExp(viscous.reynolds ?? 1e6);
            }}
          />
        </label>
        <label>
          Mach
          <input
            type="number"
            min="0"
            max="0.9"
            step="0.05"
            value={viscous.mach ?? 0}
            disabled={running}
            onchange={(e) =>
              finite(e, (x) =>
                update((s) => (s.problem.viscous = { ...s.problem.viscous, mach: Math.max(0, Math.min(0.9, x)) })),
              )}
          />
        </label>
        <label title="Critical amplification ratio (top and bottom)">
          Ncrit
          <input
            type="number"
            min="0.1"
            step="1"
            value={viscous.ncrit?.[0] ?? 9}
            disabled={running}
            onchange={(e) =>
              finite(e, (x) => x > 0 && update((s) => (s.problem.viscous = { ...s.problem.viscous, ncrit: [x, x] })))}
          />
        </label>
        <label title="Forced transition x/c (top and bottom); 1 = free transition">
          Trip x/c
          <input
            type="number"
            min="0"
            max="1"
            step="0.05"
            value={viscous.xtr?.[0] ?? 1}
            disabled={running}
            onchange={(e) =>
              finite(e, (x) =>
                update((s) => (s.problem.viscous = { ...s.problem.viscous, xtr: [Math.min(Math.max(x, 0), 1), Math.min(Math.max(x, 0), 1)] })),
              )}
          />
        </label>
      </div>

      <h3>Constraints</h3>
      {#each constraintRows as row (row.key)}
        {@const current = constraints[row.key] ?? null}
        <div class="constraint" title={row.title}>
          <label class="check">
            <input
              type="checkbox"
              checked={current !== null}
              disabled={running}
              onchange={(e) => toggleConstraint(row.key, e.currentTarget.checked, row.initial())}
            />
            {row.label}
          </label>
          {#if current !== null}
            <input
              type="number"
              step={row.step}
              value={current}
              disabled={running}
              onchange={(e) => finite(e, (x) => setConstraint(row.key, x))}
            />
          {/if}
        </div>
      {/each}
      <div class="stations">
        {#each constraints.stations ?? [] as st, i (i)}
          <div class="constraint">
            <span class="muted">t/c at x =</span>
            <input
              type="number"
              min="0.01"
              max="0.99"
              step="0.05"
              value={st.x}
              disabled={running}
              onchange={(e) =>
                finite(e, (x) => x > 0 && x < 1 && update((s) => s.problem.constraints?.stations && (s.problem.constraints.stations[i].x = x)))}
            />
            <span class="muted">≥</span>
            <input
              type="number"
              min="0.001"
              step="0.005"
              value={st.min}
              disabled={running}
              onchange={(e) =>
                finite(e, (x) => x > 0 && update((s) => s.problem.constraints?.stations && (s.problem.constraints.stations[i].min = x)))}
            />
            <button
              class="icon"
              title="Remove"
              disabled={running}
              onclick={() => update((s) => s.problem.constraints?.stations?.splice(i, 1))}>×</button
            >
          </div>
        {/each}
        <button
          class="small-button"
          disabled={running}
          title="Minimum thickness at a chord station, e.g. for a spar"
          onclick={() =>
            update((s) => {
              const c: Constraints = s.problem.constraints ?? {};
              c.stations = [...(c.stations ?? []), { x: 0.8, min: 0.03 }];
              s.problem.constraints = c;
            })}>+ Thickness at a station</button
        >
      </div>

      <h3>Search</h3>
      <div class="grid1">
        <label title="Bernstein order of the CST shape functions; 2 (n + 1) design variables">
          CST order
          <select
            value={problem.design?.order ?? 6}
            disabled={running}
            onchange={(e) =>
              update((s) => (s.problem.design = { ...s.problem.design, order: Number(e.currentTarget.value) }))}
          >
            {#each [3, 4, 5, 6, 7, 8, 10] as n (n)}
              <option value={n}>{n} ({2 * (n + 1)} variables)</option>
            {/each}
          </select>
        </label>
        <label title="Number of airfoils analyzed, the initial one included">
          Evaluations
          <input
            type="number"
            min="20"
            step="100"
            value={options.max_evaluations ?? 600}
            disabled={running}
            onchange={(e) => finite(e, (x) => x >= 20 && update((s) => (s.options.max_evaluations = Math.round(x))))}
          />
        </label>
        <label>
          Method
          <select
            value={options.method ?? "cma_es_then_nelder_mead"}
            disabled={running}
            onchange={(e) => update((s) => (s.options.method = e.currentTarget.value as Method))}
          >
            <option value="cma_es_then_nelder_mead">CMA-ES + Nelder–Mead</option>
            <option value="cma_es">CMA-ES</option>
            <option value="nelder_mead">Nelder–Mead</option>
          </select>
        </label>
        <label title="Seed of the random sampling: the same seed gives the same result">
          Seed
          <input
            type="number"
            min="0"
            step="1"
            value={options.seed ?? 1}
            disabled={running}
            onchange={(e) => finite(e, (x) => x >= 0 && update((s) => (s.options.seed = Math.round(x))))}
          />
        </label>
      </div>

      <div class="run">
        {#if running}
          <button class="danger" onclick={() => app.cancelOptimization()}>Cancel</button>
        {:else}
          <button
            class="primary"
            disabled={airfoilId === null || app.sweep !== null}
            onclick={() => airfoilId !== null && app.runOptimization(airfoilId)}>Optimize</button
          >
        {/if}
      </div>
      <p class="muted small">
        Every design is analyzed by the viscous solver at each point; designs whose analysis does not converge are
        rejected. The airfoil's paneling is used.
      </p>
    {/if}
  </section>

  <div class="results">
    <section class="panel summary" data-tour="optimize-results">
      {#if result}
        <div class="row">
          <strong>{result.airfoilName}</strong>
          {#if result.initial && result.best}
            <span>
              {objectiveName(result.objective)}
              <span class="mono">{fixed(displayObjective(result.objective, result.initial.objective), result.objective === "min_drag" ? 5 : 2)}</span>
              →
              <span class="mono">{fixed(displayObjective(result.objective, result.best.objective), result.objective === "min_drag" ? 5 : 2)}</span>
              <strong class:good={(gain ?? 0) > 0}>{signed(gain)}</strong>
            </span>
            {#if !result.best.objective || result.best.violation}
              <span class="warn">no feasible design yet</span>
            {/if}
          {/if}
          <span class="muted">
            {result.evaluations} / {result.maxEvaluations} evaluations{failed ? `, ${failed} not converged` : ""}
            {#if elapsed !== null}· {elapsed.toFixed(1)} s{/if}
            {#if result.phase}· {phaseLabel[result.phase]}{/if}
            {#if result.stop}· {stopLabel[result.stop]}{/if}
            {#if result.fitRms !== null}· CST fit rms {result.fitRms.toExponential(1)}{/if}
          </span>
        </div>
        {#if running}
          <div class="progress" role="progressbar" aria-valuenow={progress} aria-valuemin={0} aria-valuemax={100}>
            <div class="bar" style:width={`${progress}%`}></div>
          </div>
        {:else}
          <div class="row">
            <button onclick={() => app.addOptimizedAirfoil()} disabled={app.optimizedAirfoilId !== null}
              >Add to library</button
            >
            <button
              onclick={() => app.compareOptimizedPolars()}
              disabled={app.sweep !== null}
              title="Polars of the initial and optimized airfoils at the conditions of the optimization">Compare polars</button
            >
          </div>
        {/if}
      {:else}
        <p class="muted">
          Define the operating points, the objective and the constraints, then press Optimize. The shape is
          described by CST functions fitted to the airfoil; a global search (CMA-ES, all cores) is followed by a
          local polish (Nelder–Mead).
        </p>
      {/if}
    </section>

    <div class="plots">
      <PlotPanel
        title="Shape"
        fileName="optimized_shape"
        curves={shapeCurves}
        xLabel="x/c"
        yLabel="y/c"
        equal
        emptyMessage="The shapes appear when an optimization starts"
      />
      <PlotPanel
        title={`Convergence — ${result ? objectiveName(result.objective) : "objective"} per evaluation (outliers hidden)`}
        fileName="optimization_history"
        curves={historyCurves}
        xLabel="evaluation"
        yLabel={result ? objectiveName(result.objective) : ""}
        emptyMessage="No evaluation yet"
      />
    </div>

    {#if result?.initial && result.best}
      <section class="panel table-panel">
        <table class="compare">
          <thead>
            <tr>
              <th>Point</th><th></th><th>α</th><th>Cl</th><th>Cd</th><th>L/D</th><th>Cm</th><th>Xtr top</th><th>Xtr bot</th>
            </tr>
          </thead>
          <tbody>
            {#each result.best.points as bp, i (i)}
              {@const ip = result.initial.points[i]}
              {#each [["initial", ip?.values ?? null], ["best", bp.values]] as const as [kind, v] (kind)}
                <tr class:best-row={kind === "best"}>
                  {#if kind === "initial"}<td rowspan="2">{targetLabel(bp.target)}</td>{/if}
                  <td class="muted">{kind}</td>
                  <td>{fixed(v?.alpha_deg, 2)}°</td>
                  <td>{fixed(v?.cl, 4)}</td>
                  <td>
                    {fixed(v?.cd, 5)}
                    {#if kind === "best"}<span class="delta">{signed(change(ip?.values?.cd, v?.cd))}</span>{/if}
                  </td>
                  <td>
                    {fixed(liftToDrag(v), 1)}
                    {#if kind === "best"}<span class="delta">{signed(change(liftToDrag(ip?.values), liftToDrag(v)))}</span>{/if}
                  </td>
                  <td>{fixed(v?.cm, 4)}</td>
                  <td>{fixed(v?.transition[0], 3)}</td>
                  <td>{fixed(v?.transition[1], 3)}</td>
                </tr>
              {/each}
            {/each}
          </tbody>
        </table>
        <table class="compare geometry">
          <thead><tr><th></th><th>t/c</th><th>at x/c</th><th>camber</th><th>at x/c</th><th>TE angle</th></tr></thead>
          <tbody>
            {#each [["initial", result.initial.geometry], ["best", result.best.geometry]] as const as [kind, g] (kind)}
              <tr class:best-row={kind === "best"}>
                <td class="muted">{kind}</td>
                <td>{percent(g?.max_thickness)}</td>
                <td>{fixed(g?.max_thickness_x, 3)}</td>
                <td>{percent(g?.max_camber)}</td>
                <td>{fixed(g?.max_camber_x, 3)}</td>
                <td>{fixed(g?.te_angle_deg, 1)}°</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    {/if}
  </div>
</div>

<style>
  .optimize {
    display: grid;
    grid-template-columns: 340px minmax(0, 1fr);
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .setup {
    overflow: auto;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  h2 {
    font-size: 14px;
    margin: 0 0 4px;
  }
  h3 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    margin: 10px 0 2px;
  }
  .field,
  .grid2 label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    color: var(--muted);
  }
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px 12px;
  }
  .grid2 input {
    width: 90px;
  }
  .grid1 {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .grid1 label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    color: var(--muted);
  }
  .grid1 input,
  .grid1 select {
    width: 190px;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .points {
    width: 100%;
    border-collapse: collapse;
  }
  .points th {
    text-align: left;
    font-weight: 500;
    color: var(--muted);
    font-size: 12px;
  }
  .points input {
    width: 72px;
  }
  .icon {
    padding: 0 6px;
  }
  .small-button {
    align-self: flex-start;
    font-size: 12px;
  }
  .constraint {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 28px;
  }
  .constraint .check {
    min-width: 130px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .constraint input[type="number"] {
    width: 72px;
  }
  .stations {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .run {
    margin-top: 10px;
  }
  .run button {
    width: 100%;
  }
  .results {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    gap: 12px;
    min-height: 0;
  }
  .summary {
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  .good {
    color: #16a34a;
  }
  .warn {
    color: #d97706;
  }
  .progress {
    height: 8px;
    border-radius: 4px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .bar {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  .plots {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    min-height: 0;
  }
  .table-panel {
    padding: 8px 14px;
    display: flex;
    gap: 24px;
    flex-wrap: wrap;
    overflow: auto;
    max-height: 30vh;
  }
  .compare {
    border-collapse: collapse;
    font-variant-numeric: tabular-nums;
  }
  .compare th,
  .compare td {
    padding: 2px 10px;
    text-align: right;
    white-space: nowrap;
  }
  .compare th:first-child,
  .compare td:first-child {
    text-align: left;
  }
  .best-row td {
    font-weight: 600;
  }
  .delta {
    font-weight: 400;
    color: var(--muted);
    font-size: 11px;
    margin-left: 4px;
  }
</style>
