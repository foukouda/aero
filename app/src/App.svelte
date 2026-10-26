<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import Library from "./components/Library.svelte";
  import GeometryTab from "./components/GeometryTab.svelte";
  import AnalysisTab from "./components/AnalysisTab.svelte";
  import PolarsTab from "./components/PolarsTab.svelte";
  import OptimizeTab from "./components/OptimizeTab.svelte";
  import DesignTab from "./components/DesignTab.svelte";
  import Toasts from "./components/Toasts.svelte";
  import Tour from "./components/Tour.svelte";
  import Glossary from "./components/Glossary.svelte";
  import { app, dialogs, type Tab } from "./lib/state.svelte";

  let dragging = $state(false);

  const tabs: { id: Tab; label: string }[] = [
    { id: "geometry", label: "Geometry" },
    { id: "analysis", label: "Analysis" },
    { id: "polars", label: "Polars" },
    { id: "optimize", label: "Optimize" },
    { id: "design", label: "Inverse design" },
  ];

  onMount(() => {
    void app.init();
    // Development builds expose the store for automated UI checks.
    if (import.meta.env.DEV) Object.assign(window, { __aero: { app, dialogs } });
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          dragging = true;
        } else if (payload.type === "leave") {
          dragging = false;
        } else if (payload.type === "drop") {
          dragging = false;
          void app.importPaths(payload.paths);
        }
      })
      .then((fn) => (unlisten = fn))
      .catch(() => {
        // Not running inside Tauri (plain browser): no native drag and drop.
      });
    return () => unlisten?.();
  });

  function onKey(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey)) return;
    const key = e.key.toLowerCase();
    if (key === "s") {
      e.preventDefault();
      void app.saveProject(e.shiftKey);
    } else if (key === "o") {
      e.preventDefault();
      void app.openProject();
    } else if (key === "i") {
      e.preventDefault();
      void app.importDialog();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="shell">
  <header class="topbar">
    <div class="brand">
      <img src="/aero.svg" alt="" width="22" height="22" />
      <span class="name">Aero</span>
      <span class="project muted" title={app.session?.project_path ?? "Not saved yet"}>
        {app.projectName}{app.dirty ? " •" : ""}
      </span>
    </div>
    <nav class="tabs" aria-label="Views" data-tour="tabs">
      {#each tabs as tab (tab.id)}
        <button class:active={app.tab === tab.id} onclick={() => (app.tab = tab.id)}>{tab.label}</button>
      {/each}
    </nav>
    <div class="actions">
      <button class="tour" onclick={() => app.startTour()} title="Guided tour of the application">
        ? Tour
      </button>
      <button
        data-tour="glossary"
        onclick={() => app.openGlossary()}
        title="What Cl, Cd, Re, transition… mean (also: click any dotted label)">Glossary</button
      >
      <span class="project-actions" data-tour="project">
        <button onclick={() => app.newProject()} title="New project">New</button>
      <button onclick={() => app.openProject()} title="Open project (Ctrl+O)">Open…</button>
      <button onclick={() => app.saveProject()} title="Save project (Ctrl+S)">Save</button>
      <button onclick={() => app.saveProject(true)} title="Save project as (Ctrl+Shift+S)">Save as…</button>
        <button
          class="theme"
          onclick={() => app.setTheme(app.theme === "dark" ? "light" : "dark")}
          title="Toggle light/dark theme"
          aria-label="Toggle theme">{app.theme === "dark" ? "☀" : "☾"}</button
        >
      </span>
    </div>
  </header>

  <div class="content">
    <Library />
    <main>
      {#if app.tab === "geometry"}
        <GeometryTab />
      {:else if app.tab === "analysis"}
        <AnalysisTab />
      {:else if app.tab === "polars"}
        <PolarsTab />
      {:else if app.tab === "optimize"}
        <OptimizeTab />
      {:else}
        <DesignTab />
      {/if}
    </main>
  </div>

  {#if dragging}
    <div class="drop">
      <div class="panel">Drop airfoil files (.dat) or a project to open them</div>
    </div>
  {/if}
  <Toasts />
  <Tour />
  <Glossary />
</div>

<style>
  .shell {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    height: 100%;
  }
  .topbar {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 12px;
    padding: 8px 14px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .name {
    font-weight: 700;
    font-size: 14px;
  }
  .project {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tabs {
    display: flex;
    gap: 2px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 2px;
  }
  .tabs button {
    border: none;
    background: none;
    padding: 4px 16px;
  }
  .tabs button.active {
    background: var(--panel);
    box-shadow: var(--shadow);
    font-weight: 600;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
  .project-actions {
    display: flex;
    gap: 6px;
  }
  .tour {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
  .theme {
    width: 32px;
    padding: 4px 0;
  }
  .content {
    display: grid;
    grid-template-columns: 290px minmax(0, 1fr);
    gap: 12px;
    padding: 12px;
    min-height: 0;
  }
  main {
    min-width: 0;
    min-height: 0;
  }
  .drop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 3px dashed var(--accent);
    z-index: 30;
    pointer-events: none;
  }
  .drop .panel {
    padding: 16px 24px;
    font-size: 15px;
  }
</style>
