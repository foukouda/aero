// Application state shared by the components (Svelte 5 runes). The Rust
// backend owns the session; this store mirrors it and caches derived views.

import { Channel } from "@tauri-apps/api/core";
import * as nativeDialogs from "@tauri-apps/plugin-dialog";
import { call, CommandError, commands, errorTitle } from "./api";
import type {
  AnalysisRequest,
  BatchFailure,
  CandidateResult,
  CatalogueEntry,
  Conditions,
  MissionSetup,
  MissionView,
  RecommendEvent,
  RecommendResult,
  AnalysisView,
  BestDesign,
  DesignEdit,
  DesignMethod,
  DesignPreview,
  DesignView,
  Evaluation,
  GeometryView,
  HingeY,
  HistoryEntry,
  Modification,
  ModificationPreview,
  Normalization,
  Objective,
  OptimizationEvent,
  OptimizationSetup,
  PanelingOptions,
  Phase,
  PolarPoint,
  SessionView,
  Settings,
  Shape,
  SweepEvent,
  ViscousPolarPoint,
  ViscousSettings,
} from "./bindings";
import { improvement, objectiveName } from "./optimize";
import { latestRunner } from "./latest";
import { TOUR_STEPS, defaultTourLang, type TourAction, type TourLang } from "./tour";
import { translate, type MessageKey } from "./i18n";
import { applyPreset, sig, type Preset } from "./mission";

export type Tab = "mission" | "geometry" | "analysis" | "polars" | "optimize" | "design";
export type Theme = "light" | "dark";

export interface Toast {
  id: number;
  kind: "error" | "warning" | "info";
  title: string;
  message: string;
}

export interface SweepProgress {
  jobId: string;
  done: number;
  total: number;
  /** Viscous sweep. */
  viscous: boolean;
  /** Points received so far, per airfoil (live plot). */
  live: Record<number, PolarPoint[]>;
  /** Same for a viscous sweep. */
  liveViscous: Record<number, ViscousPolarPoint[]>;
  failures: number;
}

export interface RecommendProgress {
  jobId: string;
  done: number;
  total: number;
}

export interface OptimizationRun {
  jobId: string;
  airfoilId: number;
  objective: Objective;
  maxEvaluations: number;
  evaluations: number;
  phase: Phase;
  /** Number of design variables. */
  dimension: number;
  /** Rms error of the CST fit of the initial airfoil. */
  fitRms: number;
  initialShape: Shape | null;
  initial: Evaluation | null;
  best: BestDesign | null;
  history: HistoryEntry[];
  startedAt: number;
}

/** Viscous settings of a new session (and of projects that predate them). */
export const DEFAULT_VISCOUS: ViscousSettings = {
  enabled: true,
  reynolds: 1e6,
  ncrit: [9, 9],
  xtr: [1, 1],
  variation: "fixed",
  amplification: "original",
};

/** Native file dialogs; replaceable in development builds for automated UI tests. */
export const dialogs = {
  open: nativeDialogs.open,
  save: nativeDialogs.save,
  ask: nativeDialogs.ask,
};

const DAT_FILTERS = [{ name: "Airfoil coordinates", extensions: ["dat", "txt", "cor", "xy"] }];
const PROJECT_FILTERS = [{ name: "Aero project", extensions: ["aero", "json"] }];

function initialTheme(): Theme {
  try {
    const saved = localStorage.getItem("aero-theme");
    if (saved === "light" || saved === "dark") return saved;
  } catch {
    // Storage unavailable: fall back to the system preference.
  }
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function stored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function store(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Not persisted.
  }
}

function initialTourLang(): TourLang {
  const saved = stored("aero-tour-lang");
  if (saved === "en" || saved === "fr") return saved;
  return defaultTourLang(navigator.language);
}

/** The wing chords of a computed aircraft. */
function chords(view: MissionView): { root_chord: number | null; tip_chord: number | null } {
  return { root_chord: view.aircraft.wing.root_chord, tip_chord: view.aircraft.wing.tip_chord };
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** "a (why); b (why); and 3 more" for a toast. */
function failureList(failed: BatchFailure[], shown = 3): string {
  const head = failed.slice(0, shown).map((f) => `${f.item} (${f.message})`);
  const more = failed.length > shown ? `; and ${failed.length - shown} more` : "";
  return head.join("; ") + more;
}

class AppState {
  session = $state<SessionView | null>(null);
  selectedId = $state<number | null>(null);
  tab = $state<Tab>("mission");
  theme = $state<Theme>(initialTheme());
  geometries = $state<Record<number, GeometryView>>({});
  analyses = $state<Record<number, AnalysisView>>({});
  analysisErrors = $state<Record<number, string>>({});
  sweep = $state<SweepProgress | null>(null);
  optimization = $state<OptimizationRun | null>(null);
  /** Preview of an airfoil modification (Geometry tab). */
  modificationPreview = $state<{ id: number; preview: ModificationPreview } | null>(null);
  modificationError = $state<string | null>(null);
  /** Inverse design in progress (Inverse design tab) and its new airfoil. */
  design = $state<DesignView | null>(null);
  designPreview = $state<DesignPreview | null>(null);
  designError = $state<string | null>(null);
  /** Library entry made from the last optimization result. */
  optimizedAirfoilId = $state<number | null>(null);
  toasts = $state<Toast[]>([]);
  dirty = $state(false);
  busy = $state(false);
  /** Current step of the guided tour, `null` when it is closed. */
  tourIndex = $state<number | null>(null);
  tourLang = $state<TourLang>(initialTourLang());
  /** Glossary panel, and the entry to show. */
  glossaryOpen = $state(false);
  glossaryTerm = $state<string | null>(null);
  /** Airfoil catalogue (My aircraft tab). */
  catalogue = $state<CatalogueEntry[]>([]);
  /** The aircraft of the My aircraft tab as computed (sized wing, flight
   * conditions, layout, 3D surfaces), or why it cannot be. */
  missionView = $state<MissionView | null>(null);
  missionError = $state<string | null>(null);
  /** Last airfoil evaluation for the flight, and the one in progress. */
  recommendation = $state<RecommendResult | null>(null);
  recommending = $state<RecommendProgress | null>(null);
  /** Also evaluate the library airfoils. */
  includeLibrary = $state(true);

  #nextToast = 1;

  get airfoils() {
    return this.session?.airfoils ?? [];
  }

  get visible() {
    return this.airfoils.filter((a) => a.visible);
  }

  get selected() {
    return this.airfoils.find((a) => a.id === this.selectedId) ?? null;
  }

  get settings(): Settings | null {
    return this.session?.settings ?? null;
  }

  /** Interface text in the current language. */
  t(key: MessageKey, params: Record<string, string | number> = {}): string {
    return translate(this.tourLang, key, params);
  }

  get mission(): MissionSetup | null {
    return this.session?.mission ?? null;
  }

  get missionConditions(): Conditions | null {
    return this.missionView?.conditions ?? null;
  }

  get viscous(): ViscousSettings {
    return { ...DEFAULT_VISCOUS, ...(this.session?.settings.viscous ?? {}) };
  }

  /** The single-point analysis request of the current settings. */
  analysisRequest(): AnalysisRequest | null {
    const s = this.settings;
    if (!s) return null;
    return {
      mode: s.mode ?? "alpha",
      alpha_deg: s.alpha_deg ?? 0,
      cl: s.cl ?? 0.5,
      mach: s.mach ?? 0,
      viscous: $state.snapshot(this.viscous) as ViscousSettings,
      flap_hinge: s.flap_hinge ? ($state.snapshot(s.flap_hinge) as [number, number]) : null,
    };
  }

  get projectName(): string {
    const path = this.session?.project_path;
    return path ? baseName(path) : "Untitled";
  }

  // ---------------------------------------------------------------- toasts

  notify(kind: Toast["kind"], title: string, message: string) {
    const id = this.#nextToast++;
    this.toasts.push({ id, kind, title, message });
    setTimeout(() => this.dismiss(id), kind === "error" ? 9000 : 5000);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }

  /** Runs an action, reporting failures as toasts. */
  async guard<T>(action: () => Promise<T>): Promise<T | undefined> {
    try {
      return await action();
    } catch (e) {
      if (e instanceof CommandError) this.notify("error", errorTitle(e.kind), e.message);
      else this.notify("error", "Unexpected error", String(e));
      return undefined;
    }
  }

  // ----------------------------------------------------------------- theme

  setTheme(theme: Theme) {
    this.theme = theme;
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem("aero-theme", theme);
    } catch {
      // Not persisted.
    }
  }

  // --------------------------------------------------------------- session

  async init() {
    this.setTheme(this.theme);
    await this.guard(async () => {
      this.applySession(await commands.getSession());
      this.catalogue = await call(commands.getCatalogue());
    });
    // First launch: offer the guided tour.
    if (stored("aero-tour-seen") === null && this.airfoils.length === 0) this.startTour();
  }

  // ------------------------------------------------------------ guided tour

  startTour() {
    this.tourGo(0);
  }

  endTour() {
    this.tourIndex = null;
    store("aero-tour-seen", "1");
  }

  tourGo(index: number) {
    const i = Math.max(0, Math.min(TOUR_STEPS.length - 1, index));
    this.tourIndex = i;
    const tab = TOUR_STEPS[i].tab;
    if (tab) this.tab = tab;
  }

  openGlossary(id: string | null = null) {
    this.glossaryTerm = id;
    this.glossaryOpen = true;
  }

  closeGlossary() {
    this.glossaryOpen = false;
  }

  setTourLang(lang: TourLang) {
    this.tourLang = lang;
    store("aero-tour-lang", lang);
  }

  /** "Do it for me" actions of the tour. */
  async runTourAction(action: TourAction) {
    switch (action) {
      case "generate-naca":
        await this.createNaca("2412", false);
        break;
      case "show-nodes":
        this.updateSettings({ show_nodes: true });
        break;
      case "set-alpha":
        this.updateSettings({ alpha_deg: 5 });
        break;
      case "enable-viscous":
        this.updateViscous({ enabled: true, reynolds: 1e6 });
        break;
      case "run-sweep":
        await this.runSweep();
        break;
    }
  }

  applySession(view: SessionView) {
    this.session = view;
    const ids = new Set(view.airfoils.map((a) => a.id));
    for (const key of Object.keys(this.geometries)) {
      if (!ids.has(Number(key))) delete this.geometries[Number(key)];
    }
    for (const key of Object.keys(this.analyses)) {
      if (!ids.has(Number(key))) delete this.analyses[Number(key)];
    }
    if (this.selectedId === null || !ids.has(this.selectedId)) {
      this.selectedId = view.airfoils[0]?.id ?? null;
    }
    for (const a of view.airfoils) {
      if (!this.geometries[a.id]) void this.loadGeometry(a.id);
    }
    void this.analyzeVisible();
    void this.#computeMission($state.snapshot(view.mission) as MissionSetup);
  }

  async refresh() {
    await this.guard(async () => this.applySession(await commands.getSession()));
  }

  select(id: number) {
    this.selectedId = id;
  }

  // --------------------------------------------------------------- library

  /** Generates one NACA airfoil, or several from a list ("0012, 2412 4415"). */
  async createNaca(designations: string, closedTe: boolean) {
    await this.guard(async () => {
      const r = await call(commands.createNacaSeries(designations, closedTe));
      if (r.added.length) {
        this.dirty = true;
        this.selectedId = r.added[r.added.length - 1].id;
      }
      if (r.added.length > 1) this.notify("info", `${r.added.length} airfoils generated`, r.added.map((a) => a.name).join(", "));
      if (r.failed.length) this.notify("error", "Not generated", failureList(r.failed));
      await this.refresh();
    });
  }

  /** Imports coordinate files and folders; opens project files. */
  async importPaths(paths: string[]) {
    const others: string[] = [];
    for (const path of paths) {
      if (/\.(aero|json)$/i.test(path)) await this.openProjectPath(path);
      else others.push(path);
    }
    if (others.length === 0) return;
    await this.guard(async () => {
      const r = await call(commands.importPaths(others));
      const added = r.imported.map((i) => i.airfoil);
      if (added.length) {
        this.dirty = true;
        this.selectedId = added[added.length - 1].id;
      }
      if (added.length === 1 && r.imported[0].warnings.length) {
        this.notify("warning", `Imported ${added[0].name}`, r.imported[0].warnings.join("; "));
      } else if (added.length > 1) {
        const warned = r.imported.filter((i) => i.warnings.length).length;
        this.notify(
          "info",
          `${added.length} airfoils imported`,
          warned ? `${warned} with warnings (duplicate points removed, reordered…).` : added.map((a) => a.name).join(", "),
        );
      }
      if (r.failed.length) this.notify("error", `${r.failed.length} file(s) not imported`, failureList(r.failed));
    });
    await this.refresh();
  }

  async importDialog() {
    const picked = await dialogs.open({ multiple: true, filters: DAT_FILTERS });
    if (!picked) return;
    await this.importPaths(Array.isArray(picked) ? picked : [picked]);
  }

  /** Imports every coordinate file of a folder. */
  async importFolderDialog() {
    const picked = await dialogs.open({ directory: true, multiple: false });
    if (typeof picked === "string") await this.importPaths([picked]);
  }

  /** Adds a set of classic airfoils to compare. */
  async addExamples() {
    await this.guard(async () => {
      const added = await call(commands.addExamples());
      this.dirty = true;
      this.selectedId = added[0]?.id ?? this.selectedId;
      this.notify("info", `${added.length} example airfoils added`, added.map((a) => a.name).join(", "));
      await this.refresh();
    });
  }

  /** Shows or hides every airfoil. */
  async setAllVisible(visible: boolean) {
    const ids = this.airfoils.filter((a) => a.visible !== visible).map((a) => a.id);
    if (ids.length === 0) return;
    await this.guard(async () => {
      await call(commands.setAirfoilsVisible(ids, visible));
      this.dirty = true;
      await this.refresh();
    });
  }

  /** Deletes the hidden airfoils, after confirmation. */
  async deleteHidden() {
    const hidden = this.airfoils.filter((a) => !a.visible);
    if (hidden.length === 0) return;
    const confirmed = await dialogs.ask(`Delete the ${hidden.length} hidden airfoil(s) and their results?`, {
      title: "Delete hidden airfoils",
      kind: "warning",
    });
    if (!confirmed) return;
    await this.guard(async () => {
      await call(commands.deleteAirfoils(hidden.map((a) => a.id)));
      this.dirty = true;
      await this.refresh();
    });
  }

  async duplicate(id: number) {
    await this.guard(async () => {
      const a = await call(commands.duplicateAirfoil(id));
      this.dirty = true;
      this.selectedId = a.id;
      await this.refresh();
    });
  }

  async rename(id: number, name: string) {
    await this.guard(async () => {
      await call(commands.renameAirfoil(id, name));
      this.dirty = true;
      delete this.geometries[id];
      await this.refresh();
    });
  }

  async remove(id: number) {
    const name = this.airfoils.find((a) => a.id === id)?.name ?? "this airfoil";
    const confirmed = await dialogs.ask(`Delete ${name} and its results?`, {
      title: "Delete airfoil",
      kind: "warning",
    });
    if (!confirmed) return;
    await this.guard(async () => {
      await call(commands.deleteAirfoil(id));
      this.dirty = true;
      await this.refresh();
    });
  }

  async toggleVisible(id: number) {
    const a = this.airfoils.find((x) => x.id === id);
    if (!a) return;
    await this.guard(async () => {
      await call(commands.setAirfoilVisible(id, !a.visible));
      a.visible = !a.visible;
      this.dirty = true;
      void this.analyzeVisible();
    });
  }

  // -------------------------------------------------------------- geometry

  async loadGeometry(id: number) {
    await this.guard(async () => {
      this.geometries[id] = await call(commands.getGeometry(id));
    });
  }

  #paneling = latestRunner(async ({ id, options }: { id: number; options: PanelingOptions }) => {
    await this.guard(async () => {
      this.geometries[id] = await call(commands.setPaneling(id, options));
      this.dirty = true;
      await this.refresh();
    });
  });

  /** Updates the paneling (immediate preview, latest request wins). */
  setPaneling(id: number, options: PanelingOptions) {
    const g = this.geometries[id];
    if (g) g.paneling = options; // controls follow immediately
    void this.#paneling({ id, options });
  }

  // ---------------------------------------------------------- modifications

  #preview = latestRunner(async ({ id, modification }: { id: number; modification: Modification }) => {
    const r = await commands.previewModification(id, modification);
    if (r.status === "ok") {
      this.modificationPreview = { id, preview: r.data };
      this.modificationError = null;
    } else {
      this.modificationPreview = null;
      this.modificationError = r.error.message;
    }
  });

  /** Previews a modification of an airfoil (latest request wins). */
  previewModification(id: number, modification: Modification) {
    void this.#preview({ id, modification });
  }

  clearPreview() {
    this.modificationPreview = null;
    this.modificationError = null;
  }

  /** Adds a modified copy of an airfoil to the library. */
  async applyModification(id: number, modification: Modification, name: string) {
    await this.guard(async () => {
      const a = await call(commands.applyModification(id, modification, name.trim() || null));
      this.dirty = true;
      this.selectedId = a.id;
      await this.refresh();
      this.notify("info", "Airfoil created", a.name);
    });
  }

  /** Adds copies of an airfoil with its flap deflected by each deflection. */
  async flapSeries(id: number, hingeX: number, hingeY: HingeY, deflections: number[]) {
    await this.guard(async () => {
      const r = await call(commands.flapSeries(id, hingeX, hingeY, deflections));
      if (r.added.length) {
        this.dirty = true;
        this.notify("info", `${r.added.length} airfoils created`, r.added.map((a) => a.name).join(", "));
      }
      if (r.failed.length) this.notify("error", "Not created", failureList(r.failed));
      await this.refresh();
    });
  }

  // -------------------------------------------------------- inverse design

  #designQueue: Promise<void> = Promise.resolve();

  /** Starts an inverse design of an airfoil (its current speed as target). */
  startDesign(id: number, method: DesignMethod, alphaDeg: number) {
    this.#designQueue = this.#designQueue.then(async () => {
      const r = await commands.designStart(id, method, alphaDeg);
      if (r.status === "ok") {
        this.design = r.data;
        this.designError = null;
        this.designPreview = null;
        void this.#designPreview(null);
      } else {
        this.design = null;
        this.designPreview = null;
        this.designError = r.error.message;
      }
    });
    return this.#designQueue;
  }

  /** Changes the inverse design; every change is applied, in order. */
  editDesign(edit: DesignEdit) {
    this.#designQueue = this.#designQueue.then(async () => {
      if (!this.design) return;
      const r = await commands.designEdit(edit);
      if (r.status === "ok") {
        this.design = r.data;
        this.designError = null;
        void this.#designPreview(null);
      } else {
        this.designError = r.error.message;
      }
    });
    return this.#designQueue;
  }

  #designPreview = latestRunner(async (_: null) => {
    const r = await commands.designPreview();
    if (r.status === "ok") {
      this.designPreview = r.data;
    } else {
      this.designPreview = null;
      this.designError = r.error.message;
    }
  });

  /** Adds the airfoil of the inverse design to the library. */
  async applyDesign(name: string) {
    await this.#designQueue;
    await this.guard(async () => {
      const a = await call(commands.designApply(name.trim() || null));
      this.dirty = true;
      await this.refresh();
      this.notify("info", "Airfoil created", a.name);
    });
  }

  /** Ends the inverse design. */
  closeDesign() {
    this.#designQueue = this.#designQueue.then(async () => {
      await commands.designClose();
      this.design = null;
      this.designPreview = null;
      this.designError = null;
    });
  }

  async normalize(id: number, mode: Normalization) {
    await this.guard(async () => {
      this.geometries[id] = await call(commands.normalizeAirfoil(id, mode));
      this.dirty = true;
      await this.refresh();
    });
  }

  // -------------------------------------------------------------- settings

  #saveSettings = latestRunner(async (settings: Settings) => {
    await commands.updateSettings(settings);
  });

  /** Changes analysis settings; re-runs the single-point analysis. */
  updateSettings(patch: Partial<Settings>) {
    if (!this.session) return;
    this.session.settings = { ...this.session.settings, ...patch };
    this.dirty = true;
    void this.#saveSettings($state.snapshot(this.session.settings));
    const analysisKeys = ["alpha_deg", "cl", "mode", "mach", "allow_supercritical", "viscous", "flap_hinge"];
    if (analysisKeys.some((k) => k in patch)) {
      void this.analyzeVisible();
    }
  }

  /** Changes the viscous settings. */
  updateViscous(patch: Partial<ViscousSettings>) {
    this.updateSettings({ viscous: { ...$state.snapshot(this.viscous), ...patch } as ViscousSettings });
  }

  // -------------------------------------------------------------- analysis

  #analyze = latestRunner(async (request: AnalysisRequest) => {
    for (const a of this.visible) {
      const r = await commands.analyze(a.id, request);
      if (r.status === "ok") {
        this.analyses[a.id] = r.data;
        delete this.analysisErrors[a.id];
      } else {
        delete this.analyses[a.id];
        this.analysisErrors[a.id] = r.error.message;
      }
    }
  });

  /** Analyzes the visible airfoils at the current operating point. */
  analyzeVisible() {
    const request = this.analysisRequest();
    if (!request) return Promise.resolve();
    return this.#analyze(request);
  }

  // ----------------------------------------------------------------- sweep

  async runSweep() {
    const s = this.settings;
    if (!s || this.sweep) return;
    const ids = this.visible.map((a) => a.id);
    if (ids.length === 0) {
      this.notify("info", "Nothing to analyze", "Show at least one airfoil to run a sweep.");
      return;
    }
    const jobId = crypto.randomUUID();
    const viscous = $state.snapshot(this.viscous) as ViscousSettings;
    const progress: SweepProgress = {
      jobId,
      done: 0,
      total: 0,
      viscous: viscous.enabled,
      live: {},
      liveViscous: {},
      failures: 0,
    };
    this.sweep = progress;
    const channel = new Channel<SweepEvent>();
    channel.onmessage = (event) => {
      const p = this.sweep;
      if (!p || p.jobId !== jobId) return;
      switch (event.type) {
        case "started":
          p.total = event.total;
          break;
        case "progress":
          p.done = Math.max(p.done, event.done);
          if (event.points.length) {
            const current = p.live[event.airfoil_id] ?? [];
            // Replace the array (one reactive update per batch).
            p.live[event.airfoil_id] = current.concat(event.points);
          }
          p.failures += event.failures.length;
          break;
        case "viscous_progress":
          p.done = Math.max(p.done, event.done);
          if (event.points.length) {
            const current = p.liveViscous[event.airfoil_id] ?? [];
            p.liveViscous[event.airfoil_id] = current.concat(event.points);
          }
          p.failures += event.failures.length;
          break;
        case "finished":
          break;
      }
    };
    await this.guard(async () => {
      await call(
        commands.runSweep(
          {
            job_id: jobId,
            airfoil_ids: ids,
            mode: s.mode ?? "alpha",
            sweep: s.sweep,
            cl_sweep: s.cl_sweep ?? { start: 0, end: 1.2, step: 0.1 },
            mach: s.mach,
            viscous,
            flap_hinge: s.flap_hinge ?? null,
          },
          channel,
        ),
      );
      const failures = this.sweep?.failures ?? 0;
      if (failures > 0) {
        this.notify(
          "warning",
          "Some points failed",
          viscous.enabled
            ? `${failures} point(s) did not converge (usually beyond stall) and are not in the polar.`
            : `${failures} point(s) could not be computed (critical Mach number exceeded?).`,
        );
      }
      this.dirty = true;
    });
    this.sweep = null;
    await this.refresh();
  }

  async cancelSweep() {
    if (this.sweep) await commands.cancelSweep(this.sweep.jobId);
  }

  // ---------------------------------------------------------- optimization

  #saveOptimization = latestRunner(async (setup: OptimizationSetup) => {
    await commands.updateOptimizationSetup(setup);
  });

  /** Changes the optimization setup. */
  updateOptimizationSetup(setup: OptimizationSetup) {
    if (!this.session) return;
    this.session.optimization = setup;
    this.dirty = true;
    void this.#saveOptimization($state.snapshot(setup) as OptimizationSetup);
  }

  /** Optimizes an airfoil with the current setup, with live progress. */
  async runOptimization(airfoilId: number) {
    const setup = this.session?.optimization;
    if (!setup || this.optimization || this.sweep) return;
    const jobId = crypto.randomUUID();
    const objective = setup.problem.objective ?? "min_drag";
    this.optimization = {
      jobId,
      airfoilId,
      objective,
      maxEvaluations: setup.options.max_evaluations ?? 0,
      evaluations: 0,
      phase: "initial",
      dimension: 0,
      fitRms: Number.NaN,
      initialShape: null,
      initial: null,
      best: null,
      history: [],
      startedAt: performance.now(),
    };
    const channel = new Channel<OptimizationEvent>();
    channel.onmessage = (event) => {
      const run = this.optimization;
      if (!run || run.jobId !== jobId) return;
      switch (event.type) {
        case "started":
          run.maxEvaluations = event.max_evaluations;
          run.dimension = event.dimension;
          run.fitRms = event.fit_rms_error ?? Number.NaN;
          run.initialShape = event.initial_shape;
          break;
        case "progress":
          run.phase = event.phase;
          run.evaluations = event.evaluations;
          run.history.push(...event.history);
          if (event.best) {
            run.best = event.best;
            if (event.phase === "initial") run.initial = event.best.evaluation;
          }
          break;
        case "finished":
          break;
      }
    };
    await this.guard(async () => {
      const r = await call(
        commands.runOptimization(
          { job_id: jobId, airfoil_id: airfoilId, setup: $state.snapshot(setup) as OptimizationSetup },
          channel,
        ),
      );
      this.dirty = true;
      this.optimizedAirfoilId = null;
      const gain = improvement(objective, r.initial.objective, r.best.objective);
      const summary =
        gain === null
          ? "No feasible design was found."
          : `${objectiveName(objective)} improved by ${gain.toFixed(1)} % in ${r.history.length} evaluations.`;
      this.notify("info", r.stop === "cancelled" ? "Optimization cancelled" : "Optimization finished", summary);
    });
    this.optimization = null;
    await this.refresh();
  }

  async cancelOptimization() {
    if (this.optimization) await commands.cancelOptimization(this.optimization.jobId);
  }

  /** Adds the best design of the last optimization to the library. */
  async addOptimizedAirfoil(): Promise<number | null> {
    const id = await this.guard(async () => {
      const a = await call(commands.addOptimizedAirfoil());
      this.dirty = true;
      this.optimizedAirfoilId = a.id;
      this.selectedId = a.id;
      await this.refresh();
      this.notify("info", "Added to the library", a.name);
      return a.id;
    });
    return id ?? null;
  }

  /**
   * Polars of the initial and optimized airfoils, at the flow conditions of
   * the optimization (the optimized one is added to the library if needed).
   */
  async compareOptimizedPolars() {
    const last = this.session?.last_optimization;
    if (!last || this.sweep) return;
    const added = this.airfoils.some((a) => a.id === this.optimizedAirfoilId) ? this.optimizedAirfoilId : null;
    const id = added ?? (await this.addOptimizedAirfoil());
    if (id === null) return;
    await this.guard(async () => {
      for (const a of this.airfoils) {
        const shown = a.id === id || a.id === last.airfoil_id;
        if (a.visible !== shown) await call(commands.setAirfoilVisible(a.id, shown));
      }
    });
    await this.refresh();
    const v = last.problem.viscous;
    if (v) {
      this.updateSettings({
        mach: v.mach ?? 0,
        viscous: {
          ...($state.snapshot(this.viscous) as ViscousSettings),
          enabled: true,
          reynolds: v.reynolds ?? 1e6,
          ncrit: v.ncrit ?? [9, 9],
          xtr: v.xtr ?? [1, 1],
          variation: v.variation ?? "fixed",
          amplification: v.amplification ?? "original",
        },
      });
    }
    this.tab = "polars";
    await this.runSweep();
  }

  // --------------------------------------------------------------- exports

  async exportDat(id: number) {
    const a = this.airfoils.find((x) => x.id === id);
    const path = await dialogs.save({
      defaultPath: `${(a?.name ?? "airfoil").replace(/[^\w.-]+/g, "_")}.dat`,
      filters: DAT_FILTERS,
    });
    if (!path) return;
    await this.guard(async () => {
      await call(commands.exportDat(id, path));
      this.notify("info", "Exported", baseName(path));
    });
  }

  async exportPolarsCsv() {
    const path = await dialogs.save({ defaultPath: "polars.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    await this.guard(async () => {
      await call(commands.exportPolarsCsv([], path));
      this.notify("info", "Exported", baseName(path));
    });
  }

  async exportCpCsv(id: number) {
    const request = this.analysisRequest();
    if (!request) return;
    const path = await dialogs.save({ defaultPath: "cp.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    await this.guard(async () => {
      await call(commands.exportCpCsv(id, request, path));
      this.notify("info", "Exported", baseName(path));
    });
  }

  async exportBoundaryLayerCsv(id: number) {
    const request = this.analysisRequest();
    if (!request) return;
    const path = await dialogs.save({
      defaultPath: "boundary_layer.csv",
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    await this.guard(async () => {
      await call(
        commands.exportBoundaryLayerCsv(id, request, path),
      );
      this.notify("info", "Exported", baseName(path));
    });
  }

  /** Saves a text file (CSV…) where the user chooses. */
  async saveText(defaultName: string, text: string) {
    const extension = defaultName.split(".").pop() ?? "txt";
    const path = await dialogs.save({
      defaultPath: defaultName,
      filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
    });
    if (!path) return;
    await this.guard(async () => {
      await call(commands.writeFile(path, Array.from(new TextEncoder().encode(text))));
      this.notify("info", "Exported", baseName(path));
    });
  }

  /** Saves plot bytes (PNG or SVG) chosen by a save dialog. */
  async savePlot(name: string, format: "png" | "svg", bytes: () => Promise<Uint8Array>) {
    const path = await dialogs.save({
      defaultPath: `${name}.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (!path) return;
    await this.guard(async () => {
      const data = await bytes();
      await call(commands.writeFile(path, Array.from(data)));
      this.notify("info", "Exported", baseName(path));
    });
  }

  // -------------------------------------------------------------- projects

  async confirmDiscard(): Promise<boolean> {
    if (!this.dirty || this.airfoils.length === 0) return true;
    return dialogs.ask("Discard the unsaved changes of the current project?", {
      title: "Unsaved changes",
      kind: "warning",
    });
  }

  async newProject() {
    if (!(await this.confirmDiscard())) return;
    await this.guard(async () => {
      this.optimizedAirfoilId = null;
      this.geometries = {};
      this.analyses = {};
      this.applySession(await commands.newSession());
      this.dirty = false;
    });
  }

  async openProject() {
    if (!(await this.confirmDiscard())) return;
    const path = await dialogs.open({ multiple: false, filters: PROJECT_FILTERS });
    if (typeof path === "string") await this.openProjectPath(path);
  }

  async openProjectPath(path: string) {
    await this.guard(async () => {
      const view = await call(commands.openProject(path));
      this.optimizedAirfoilId = null;
      this.geometries = {};
      this.analyses = {};
      this.analysisErrors = {};
      this.applySession(view);
      this.dirty = false;
    });
  }

  // ------------------------------------------------------------ my aircraft

  #saveMission = latestRunner(async (setup: MissionSetup) => {
    await commands.updateMissionSetup(setup);
  });

  /** The aircraft as computed by the backend; a sized wing is kept in the
   * setup so that it is saved and shown. */
  #computeMission = latestRunner(async (setup: MissionSetup) => {
    try {
      const view = await call(commands.missionView(setup));
      this.missionView = view;
      this.missionError = null;
      const current = this.session?.mission;
      if (
        current &&
        setup.sizing_cl !== null &&
        (view.aircraft.wing.root_chord !== current.aircraft.wing.root_chord ||
          view.aircraft.wing.tip_chord !== current.aircraft.wing.tip_chord)
      ) {
        this.session!.mission = {
          ...current,
          aircraft: { ...current.aircraft, wing: { ...current.aircraft.wing, ...chords(view) } },
        };
        void this.#saveMission($state.snapshot(this.session!.mission) as MissionSetup);
      }
    } catch (e) {
      this.missionView = null;
      this.missionError = e instanceof Error ? e.message : String(e);
    }
  });

  /** Starts from an example aircraft and its typical flight. */
  async applyPreset(preset: Preset) {
    const setup = this.mission;
    if (!setup) return;
    const aircraft = await commands.aircraftPreset(preset.aircraft);
    this.updateMission(applyPreset($state.snapshot(setup) as MissionSetup, preset, aircraft));
  }

  /** Puts a recommended airfoil on the wing (root and tip). */
  putOnWing(c: CandidateResult) {
    const setup = this.mission;
    if (!setup) return;
    const key = c.source.kind === "library" ? `library:${c.source.id}` : `catalogue:${c.source.id}`;
    const s = $state.snapshot(setup) as MissionSetup;
    s.aircraft.wing.root_airfoil = key;
    s.aircraft.wing.tip_airfoil = key;
    this.updateMission(s);
    this.notify("info", this.t("toast.wing.title"), this.t("toast.wing", { name: c.name }));
  }

  /** Changes the flight of the My aircraft tab. */
  updateMission(setup: MissionSetup) {
    if (!this.session) return;
    this.session.mission = setup;
    this.dirty = true;
    const plain = $state.snapshot(setup) as MissionSetup;
    void this.#saveMission(plain);
    void this.#computeMission(plain);
  }

  /** Evaluates the catalogue (and the library) for the flight. */
  async runRecommendation() {
    const setup = this.mission;
    const view = this.missionView;
    if (!setup || !view || this.recommending) return;
    const jobId = `recommend-${Date.now()}`;
    const catalogueIds = this.catalogue.map((c) => c.id);
    const airfoilIds = this.includeLibrary ? this.airfoils.map((a) => a.id) : [];
    this.recommending = { jobId, done: 0, total: catalogueIds.length + airfoilIds.length };
    const channel = new Channel<RecommendEvent>();
    channel.onmessage = (event) => {
      if (!this.recommending || event.type !== "progress") return;
      this.recommending = { ...this.recommending, done: event.done, total: event.total };
    };
    const plain = $state.snapshot(setup) as MissionSetup;
    await this.guard(async () => {
      this.recommendation = await call(
        commands.recommendAirfoils(
          {
            job_id: jobId,
            flight: plain.flight,
            wing: $state.snapshot(view.wing),
            settings: { ncrit: plain.ncrit },
            model: plain.model,
            catalogue_ids: catalogueIds,
            airfoil_ids: airfoilIds,
          },
          channel,
        ),
      );
    });
    this.recommending = null;
  }

  async cancelRecommendation() {
    if (this.recommending) await commands.cancelRecommendation(this.recommending.jobId);
  }

  /** Reynolds number, lift coefficient and Mach number of the last
   * evaluated flight, rounded as the analysis settings show them. */
  #flightValues(): { reynolds: number; cl: number; mach: number } | null {
    const c = this.recommendation?.conditions;
    if (!c || c.reynolds === null || c.required_cl === null || c.mach === null) return null;
    return {
      reynolds: Number(c.reynolds.toPrecision(3)),
      cl: Number(c.required_cl.toFixed(3)),
      mach: Number(c.mach.toFixed(3)),
    };
  }

  /** Puts a candidate in the library (if needed) and selects it. */
  async #candidateInLibrary(c: CandidateResult): Promise<number | null> {
    if (c.source.kind === "library") {
      this.selectedId = c.source.id;
      return c.source.id;
    }
    const r = await call(commands.addFromCatalogue([c.source.id]));
    const id = r.added[0]?.id ?? null;
    this.dirty = true;
    await this.refresh();
    if (id !== null) this.selectedId = id;
    return id;
  }

  /** Opens a candidate in the Analysis tab at the flight's conditions. */
  async analyseCandidate(c: CandidateResult) {
    const flight = this.#flightValues();
    if (!flight) return;
    await this.guard(async () => {
      if ((await this.#candidateInLibrary(c)) === null) return;
      const { reynolds, cl, mach } = flight;
      const ncrit = this.mission?.ncrit ?? 9;
      this.updateSettings({
        mode: "cl",
        cl,
        mach,
        viscous: {
          ...($state.snapshot(this.viscous) as ViscousSettings),
          enabled: true,
          reynolds,
          ncrit: [ncrit, ncrit],
        },
      });
      this.tab = "analysis";
      this.notify(
        "info",
        this.t("toast.analyse.title"),
        this.t("toast.analyse", {
          name: c.name,
          re: sig(reynolds, this.tourLang),
          cl: cl.toFixed(2),
        }),
      );
    });
  }

  /** Opens a candidate in the Optimize tab: least drag at the flight's lift
   * coefficient and Reynolds number, keeping at least its thickness. */
  async optimizeCandidate(c: CandidateResult) {
    const flight = this.#flightValues();
    const setup = this.session?.optimization;
    if (!flight || !setup) return;
    await this.guard(async () => {
      if ((await this.#candidateInLibrary(c)) === null) return;
      const s = $state.snapshot(setup) as OptimizationSetup;
      const ncrit = this.mission?.ncrit ?? 9;
      const { reynolds, cl, mach } = flight;
      s.problem.points = [{ target: { kind: "cl", value: cl }, weight: 1 }];
      s.problem.objective = "min_drag";
      s.problem.viscous = {
        ...s.problem.viscous,
        reynolds,
        mach,
        ncrit: [ncrit, ncrit],
      };
      // Rounded down to 0.1 % so the airfoil itself satisfies it.
      const thickness = c.evaluation?.buildability.thickness;
      if (thickness) {
        s.problem.constraints = {
          ...s.problem.constraints,
          min_thickness: Math.floor(thickness * 1000) / 1000,
        };
      }
      this.updateOptimizationSetup(s);
      this.tab = "optimize";
      this.notify(
        "info",
        this.t("toast.optimize.title"),
        this.t("toast.optimize", { name: c.name, re: sig(reynolds, this.tourLang), cl: cl.toFixed(2) }),
      );
    });
  }

  async saveProject(saveAs = false) {
    let path = saveAs ? null : (this.session?.project_path ?? null);
    if (!path) {
      path = await dialogs.save({ defaultPath: "project.aero", filters: PROJECT_FILTERS });
      if (!path) return;
    }
    const target = path;
    await this.guard(async () => {
      this.session = await call(commands.saveProject(target));
      this.dirty = false;
      this.notify("info", "Project saved", baseName(target));
    });
  }
}

export const app = new AppState();
