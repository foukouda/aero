<script lang="ts">
  // Gallery of random aircraft: a grid of pictures from the same angle,
  // each a card to open in a large 3D view (previous, next, use it).
  import { tick } from "svelte";
  import AircraftViewer from "./AircraftViewer.svelte";
  import type { RandomAircraftView } from "../lib/bindings";
  import type { MessageKey } from "../lib/i18n";
  import { app } from "../lib/state.svelte";
  import { length, num, sig } from "../lib/mission";
  import { renderThumbnails } from "../lib/scene3d";

  const lang = $derived(app.tourLang);
  const items = $derived(app.gallery?.items ?? []);
  let thumbs = $state<string[]>([]);
  let zoom = $state<number | null>(null);
  let count = $state(app.galleryCount);
  let dialog: HTMLDivElement | undefined = $state();

  $effect(() => {
    // Pictures for each new draw (one WebGL context for all of them).
    const list = items;
    thumbs = [];
    zoom = null;
    if (!list.length) return;
    void tick().then(() => {
      thumbs = renderThumbnails(
        list.map((i) => i.parts),
        360,
        220,
      );
    });
  });

  $effect(() => {
    if (!app.galleryOpen) return;
    count = app.galleryCount;
    void tick().then(() => dialog?.focus());
  });

  function airfoilName(key: string): string {
    const [kind, id] = key.split(":");
    if (kind === "catalogue") return app.catalogue.find((e) => e.id === id)?.name ?? key;
    if (kind === "naca") return `NACA ${id}`;
    return key;
  }

  /** The wing's airfoils, root first, without repeats. */
  function wingAirfoils(item: RandomAircraftView): string {
    const names = item.aircraft.wing.sections.map((s) => airfoilName(s.airfoil));
    return [...new Set(names)].join(" · ");
  }

  function configLabel(item: RandomAircraftView): string {
    return app.t(`config.${item.configuration}` as MessageKey);
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      if (zoom !== null) zoom = null;
      else app.galleryOpen = false;
    } else if (zoom !== null && e.key === "ArrowRight") {
      zoom = (zoom + 1) % items.length;
    } else if (zoom !== null && e.key === "ArrowLeft") {
      zoom = (zoom - 1 + items.length) % items.length;
    }
  }
</script>

{#if app.galleryOpen}
  <div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && (app.galleryOpen = false)}>
    <div
      class="panel gallery"
      role="dialog"
      aria-modal="true"
      aria-label={app.t("gallery.title")}
      tabindex="-1"
      bind:this={dialog}
      onkeydown={keydown}
    >
      <header>
        <div>
          <h1>{app.t("gallery.title")}</h1>
          <p class="muted">{app.t("gallery.intro")}</p>
        </div>
        <div class="controls">
          <label>
            {app.t("gallery.count")}
            <input type="number" min="1" max="60" step="1" bind:value={count} />
          </label>
          <button class="primary" disabled={app.galleryBusy} onclick={() => app.drawGallery(count)}
            >{app.t(items.length ? "gallery.again" : "gallery.generate")}</button
          >
          <button class="close" aria-label={app.t("gallery.close")} onclick={() => (app.galleryOpen = false)}>✕</button>
        </div>
      </header>

      {#if app.galleryBusy}
        <p class="status muted" role="status">{app.t("gallery.drawing", { count: app.galleryCount })}</p>
      {:else if zoom !== null && items[zoom]}
        {@const item = items[zoom]}
        {@const l = item.layout}
        <div class="zoom">
          <div class="big">
            <AircraftViewer
              parts={item.parts}
              layout={item.layout}
              wingZ={item.aircraft.wing.z ?? 0}
              selected={null}
              onselect={() => {}}
            />
          </div>
          <aside>
            <p class="title">
              <strong>{app.t("gallery.number", { n: zoom + 1 })}</strong>
              <span class="chip">{configLabel(item)}</span>
            </p>
            <dl>
              <dt>{app.t("surface.span")}</dt>
              <dd>{length(l.wing.span, lang)}</dd>
              <dt>{app.t("gallery.area")}</dt>
              <dd>{sig(l.wing.area, lang, 3)} m²</dd>
              <dt>{app.t("gallery.arLabel")}</dt>
              <dd>{num(l.wing.aspect_ratio, lang, 1)}</dd>
              <dt>{app.t("gallery.sectionsLabel")}</dt>
              <dd>{item.aircraft.wing.sections.length}</dd>
            </dl>
            <p class="airfoils">{wingAirfoils(item)}</p>
            {#if l.tail_volume !== null}<p class="muted">{app.t("gallery.tail", { value: num(l.tail_volume, lang, 2) })}</p>{/if}
            <div class="nav">
              <button onclick={() => (zoom = (zoom! - 1 + items.length) % items.length)}>← {app.t("gallery.previous")}</button>
              <button onclick={() => (zoom = (zoom! + 1) % items.length)}>{app.t("gallery.next")} →</button>
            </div>
            <button class="primary use" onclick={() => app.useRandomAircraft(zoom!)}>{app.t("gallery.use")}</button>
            <button onclick={() => (zoom = null)}>← {app.t("gallery.back")}</button>
          </aside>
        </div>
      {:else}
        <ul class="grid">
          {#each items as item, i (i)}
            <li>
              <button class="card" onclick={() => (zoom = i)}>
                {#if thumbs[i]}
                  <img src={thumbs[i]} alt="" width="360" height="220" />
                {:else}
                  <span class="placeholder"></span>
                {/if}
                <span class="caption">
                  <span class="line">
                    <strong>{app.t("gallery.number", { n: i + 1 })}</strong>
                    <span class="chip">{configLabel(item)}</span>
                  </span>
                  <span class="muted line">
                    {length(item.layout.wing.span, lang)} · {app.t("gallery.ar", {
                      value: num(item.layout.wing.aspect_ratio, lang, 1),
                    })} · {app.t("gallery.sections", { n: item.aircraft.wing.sections.length })}
                  </span>
                  <span class="airfoils">{wingAirfoils(item)}</span>
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    /* Flex, not grid: a grid row would grow with the cards and push the
     * panel off screen. */
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, #000 45%, transparent);
    padding: 24px;
  }
  .gallery {
    width: min(1320px, 100%);
    height: min(92vh, calc(100vh - 48px));
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
    outline: none;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
  }
  h1 {
    font-size: 18px;
    margin: 0;
  }
  header p {
    margin: 4px 0 0;
    max-width: 70ch;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  .controls label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .controls input {
    width: 64px;
  }
  .close {
    width: 32px;
  }
  .status {
    margin: 40px auto;
  }
  .grid {
    list-style: none;
    margin: 0;
    padding: 2px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 12px;
    overflow: auto;
  }
  .card {
    width: 100%;
    padding: 0;
    display: flex;
    flex-direction: column;
    text-align: left;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--panel-2);
    transition:
      transform 120ms ease,
      box-shadow 120ms ease;
  }
  .card:hover {
    transform: translateY(-2px);
    box-shadow: var(--shadow);
  }
  .card img,
  .placeholder {
    display: block;
    width: 100%;
    height: auto;
    aspect-ratio: 360 / 220;
    background: linear-gradient(to bottom, var(--panel), var(--panel-2));
  }
  .caption {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px 10px;
    background: var(--panel);
    border-top: 1px solid var(--border);
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 6px;
    font-size: 12px;
  }
  .chip {
    border-radius: 999px;
    padding: 0 8px;
    font-size: 11px;
    border: 1px solid var(--border);
    background: var(--panel-2);
  }
  .airfoils {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .zoom {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 260px;
    gap: 12px;
  }
  .big {
    position: relative;
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: linear-gradient(to bottom, var(--panel), var(--panel-2));
  }
  aside {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .title {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin: 0;
  }
  dl {
    display: grid;
    grid-template-columns: auto auto;
    gap: 4px 12px;
    margin: 0;
  }
  dt {
    color: var(--muted);
    font-size: 12px;
  }
  dd {
    margin: 0;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  aside .airfoils {
    white-space: normal;
    font-size: 12px;
  }
  aside p {
    margin: 0;
  }
  .nav {
    display: flex;
    gap: 6px;
  }
  .nav button {
    flex: 1;
  }
  .use {
    padding: 8px;
    font-weight: 600;
  }
</style>
