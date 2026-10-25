// Application state shared by the components (Svelte 5 runes). The Rust
// backend owns the session; this store mirrors it and caches derived views.

import { Channel } from "@tauri-apps/api/core";
import * as nativeDialogs from "@tauri-apps/plugin-dialog";
import { call, CommandError, commands, errorTitle } from "./api";
import type {
  AnalysisView,
  GeometryView,
  Normalization,
  PanelingOptions,
  PolarPoint,
  SessionView,
  Settings,
  SweepEvent,
} from "./bindings";
import { latestRunner } from "./latest";

export type Tab = "geometry" | "analysis" | "polars";
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
  /** Points received so far, per airfoil (live plot). */
  live: Record<number, PolarPoint[]>;
  failures: number;
}

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

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

class AppState {
  session = $state<SessionView | null>(null);
  selectedId = $state<number | null>(null);
  tab = $state<Tab>("geometry");
  theme = $state<Theme>(initialTheme());
  geometries = $state<Record<number, GeometryView>>({});
  analyses = $state<Record<number, AnalysisView>>({});
  analysisErrors = $state<Record<number, string>>({});
  sweep = $state<SweepProgress | null>(null);
  toasts = $state<Toast[]>([]);
  dirty = $state(false);
  busy = $state(false);

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
    });
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
  }

  async refresh() {
    await this.guard(async () => this.applySession(await commands.getSession()));
  }

  select(id: number) {
    this.selectedId = id;
  }

  // --------------------------------------------------------------- library

  async createNaca(designation: string, closedTe: boolean) {
    await this.guard(async () => {
      const a = await call(commands.createNaca(designation, closedTe));
      this.dirty = true;
      this.selectedId = a.id;
      await this.refresh();
    });
  }

  async importPaths(paths: string[]) {
    for (const path of paths) {
      if (/\.(aero|json)$/i.test(path)) {
        await this.openProjectPath(path);
        continue;
      }
      await this.guard(async () => {
        const r = await call(commands.importAirfoil(path));
        this.dirty = true;
        this.selectedId = r.airfoil.id;
        if (r.warnings.length) {
          this.notify("warning", `Imported ${r.airfoil.name}`, r.warnings.join("; "));
        }
      });
    }
    await this.refresh();
  }

  async importDialog() {
    const picked = await dialogs.open({ multiple: true, filters: DAT_FILTERS });
    if (!picked) return;
    await this.importPaths(Array.isArray(picked) ? picked : [picked]);
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
    if ("alpha_deg" in patch || "mach" in patch || "allow_supercritical" in patch) {
      void this.analyzeVisible();
    }
  }

  // -------------------------------------------------------------- analysis

  #analyze = latestRunner(async (request: { alpha: number; mach: number }) => {
    for (const a of this.visible) {
      const r = await commands.analyze(a.id, request.alpha, request.mach);
      if (r.status === "ok") {
        this.analyses[a.id] = r.data;
        delete this.analysisErrors[a.id];
      } else {
        delete this.analyses[a.id];
        this.analysisErrors[a.id] = r.error.message;
      }
    }
  });

  /** Analyzes the visible airfoils at the current angle of attack. */
  analyzeVisible() {
    const s = this.settings;
    if (!s) return Promise.resolve();
    return this.#analyze({ alpha: s.alpha_deg ?? 0, mach: s.mach ?? 0 });
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
    const progress: SweepProgress = { jobId, done: 0, total: 0, live: {}, failures: 0 };
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
          p.done = event.done;
          if (event.points.length) {
            const current = p.live[event.airfoil_id] ?? [];
            // Replace the array (one reactive update per batch).
            p.live[event.airfoil_id] = current.concat(event.points);
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
          { job_id: jobId, airfoil_ids: ids, sweep: s.sweep, mach: s.mach },
          channel,
        ),
      );
      const failures = this.sweep?.failures ?? 0;
      if (failures > 0) {
        this.notify(
          "warning",
          "Some points failed",
          `${failures} point(s) could not be computed (critical Mach number exceeded?).`,
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
    const s = this.settings;
    if (!s) return;
    const path = await dialogs.save({ defaultPath: "cp.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    await this.guard(async () => {
      await call(commands.exportCpCsv(id, s.alpha_deg, s.mach, path));
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
      this.geometries = {};
      this.analyses = {};
      this.analysisErrors = {};
      this.applySession(view);
      this.dirty = false;
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
