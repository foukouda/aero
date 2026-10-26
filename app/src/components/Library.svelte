<script lang="ts">
  import Term from "./Term.svelte";
  import { app } from "../lib/state.svelte";

  let designation = $state("2412");
  let closedTe = $state(false);
  let editing = $state<number | null>(null);
  let editName = $state("");

  async function generate(event: SubmitEvent) {
    event.preventDefault();
    await app.createNaca(designation, closedTe);
  }

  function startRename(id: number, name: string) {
    editing = id;
    editName = name;
  }

  async function commitRename() {
    if (editing === null) return;
    const id = editing;
    editing = null;
    const current = app.airfoils.find((a) => a.id === id)?.name;
    if (editName.trim() && editName !== current) await app.rename(id, editName);
  }

  function originLabel(origin: (typeof app.airfoils)[number]["origin"]): string {
    switch (origin.kind) {
      case "naca":
        return origin.closed_te ? "NACA, closed TE" : "NACA";
      case "imported":
        return origin.file_name;
      case "duplicate":
        return `copy of ${origin.of}`;
      case "optimized":
        return `optimized from ${origin.of}`;
    }
  }

  function focus(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

<aside class="library panel">
  <form class="naca" onsubmit={generate} data-tour="naca">
    <h2>New <Term id="naca">NACA</Term> airfoil</h2>
    <div class="row">
      <input
        type="text"
        bind:value={designation}
        placeholder="2412 or 23012"
        aria-label="NACA designation"
        maxlength="10"
      />
      <button class="primary" type="submit">Generate</button>
    </div>
    <label class="check">
      <input type="checkbox" bind:checked={closedTe} />
      Closed (sharp) trailing edge
    </label>
  </form>

  <div class="import" data-tour="import">
    <button onclick={() => app.importDialog()}>Import .dat…</button>
    <span class="muted hint">or drop files on the window</span>
  </div>

  <h2 class="title">Airfoils</h2>
  {#if app.airfoils.length === 0}
    <p class="muted empty">No airfoil yet. Generate a NACA section or import a coordinate file
      (Selig or Lednicer format).</p>
  {/if}
  <ul data-tour="airfoil-list">
    {#each app.airfoils as airfoil (airfoil.id)}
      <li class:selected={airfoil.id === app.selectedId}>
        <input
          type="checkbox"
          checked={airfoil.visible}
          title="Show in plots"
          onchange={() => app.toggleVisible(airfoil.id)}
        />
        <span class="swatch" style:background={airfoil.color}></span>
        {#if editing === airfoil.id}
          <input
            class="rename"
            type="text"
            bind:value={editName}
            use:focus
            onblur={commitRename}
            onkeydown={(e) => {
              if (e.key === "Enter") commitRename();
              if (e.key === "Escape") editing = null;
            }}
          />
        {:else}
          <button
            class="name"
            onclick={() => app.select(airfoil.id)}
            ondblclick={() => startRename(airfoil.id, airfoil.name)}
            title="Double-click to rename"
          >
            <span>{airfoil.name}</span>
            <small class="muted">{originLabel(airfoil.origin)} · {airfoil.n_points} pts</small>
          </button>
        {/if}
        <div class="row-actions">
          <button title="Rename" onclick={() => startRename(airfoil.id, airfoil.name)}>✎</button>
          <button title="Duplicate" onclick={() => app.duplicate(airfoil.id)}>⧉</button>
          <button title="Export .dat" onclick={() => app.exportDat(airfoil.id)}>⤓</button>
          <button class="danger" title="Delete" onclick={() => app.remove(airfoil.id)}>✕</button>
        </div>
      </li>
    {/each}
  </ul>
</aside>

<style>
  .library {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    overflow: auto;
    min-height: 0;
  }
  .naca {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .row input {
    flex: 1;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
  }
  .import {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border);
  }
  .hint {
    font-size: 11px;
  }
  .title {
    margin-top: 2px;
  }
  .empty {
    margin: 0;
    font-size: 12px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border-radius: 6px;
    border: 1px solid transparent;
  }
  li.selected {
    background: var(--panel-2);
    border-color: var(--border);
  }
  .swatch {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    flex: none;
  }
  .name {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    border: none;
    background: none;
    padding: 0;
    text-align: left;
  }
  .name span,
  .name small {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename {
    flex: 1;
  }
  .row-actions {
    display: flex;
    gap: 2px;
    opacity: 0;
  }
  li:hover .row-actions,
  li.selected .row-actions,
  li:focus-within .row-actions {
    opacity: 1;
  }
  .row-actions button {
    padding: 0 5px;
    font-size: 12px;
    border-color: transparent;
    background: transparent;
  }
</style>
