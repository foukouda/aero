<script lang="ts">
  import { tick } from "svelte";
  import { CATEGORY_TITLES, searchGlossary, type GlossaryCategory } from "../lib/glossary";
  import { app } from "../lib/state.svelte";

  let query = $state("");
  let body: HTMLDivElement | undefined = $state();

  const lang = $derived(app.tourLang);
  const groups = $derived.by(() => {
    const found = searchGlossary(query, lang);
    return (Object.keys(CATEGORY_TITLES) as GlossaryCategory[])
      .map((category) => ({ category, entries: found.filter((e) => e.category === category) }))
      .filter((g) => g.entries.length > 0);
  });

  // Show the requested entry when the panel opens.
  $effect(() => {
    const id = app.glossaryTerm;
    if (!app.glossaryOpen) return;
    query = "";
    void tick().then(() => {
      if (id) body?.querySelector(`#glossary-${id}`)?.scrollIntoView({ block: "start" });
      else body?.scrollTo({ top: 0 });
    });
  });

  function onKey(e: KeyboardEvent) {
    if (app.glossaryOpen && e.key === "Escape") app.closeGlossary();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if app.glossaryOpen}
  <div class="backdrop" role="presentation" onclick={() => app.closeGlossary()}>
    <div
      class="dialog panel"
      role="dialog"
      aria-modal="true"
      aria-label={lang === "fr" ? "Glossaire" : "Glossary"}
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <header>
        <h2>{lang === "fr" ? "Glossaire" : "Glossary"}</h2>
        <input
          type="search"
          placeholder={lang === "fr" ? "Chercher (Cd, Reynolds, transition…)" : "Search (Cd, Reynolds, transition…)"}
          bind:value={query}
        />
        <div class="segmented" role="group" aria-label="Language">
          <button class:active={lang === "en"} onclick={() => app.setTourLang("en")}>EN</button>
          <button class:active={lang === "fr"} onclick={() => app.setTourLang("fr")}>FR</button>
        </div>
        <button class="close" aria-label="Close" onclick={() => app.closeGlossary()}>✕</button>
      </header>
      <div class="body" bind:this={body}>
        {#each groups as g (g.category)}
          <h3>{CATEGORY_TITLES[g.category][lang]}</h3>
          {#each g.entries as e (e.id)}
            <article id={`glossary-${e.id}`} class:active={e.id === app.glossaryTerm}>
              <h4>
                {e.text[lang].term}
                {#if e.symbol}<span class="symbol mono">{e.symbol}</span>{/if}
              </h4>
              <p class="short">{e.text[lang].short}</p>
              {#each e.text[lang].long.split("\n\n") as paragraph, i (i)}
                <p>{paragraph}</p>
              {/each}
            </article>
          {/each}
        {:else}
          <p class="muted">{lang === "fr" ? "Aucun résultat." : "No match."}</p>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }
  .dialog {
    width: min(760px, 92vw);
    height: min(820px, 88vh);
    display: flex;
    flex-direction: column;
    padding: 0;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  header h2 {
    margin: 0;
  }
  header input {
    flex: 1;
  }
  .segmented {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .segmented button {
    border: none;
    border-radius: 0;
    padding: 2px 10px;
    background: transparent;
  }
  .segmented button.active {
    background: var(--accent);
    color: white;
  }
  .close {
    padding: 2px 8px;
  }
  .body {
    overflow: auto;
    padding: 4px 18px 18px;
  }
  h3 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    margin: 18px 0 6px;
  }
  article {
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid transparent;
    scroll-margin-top: 8px;
  }
  article.active {
    border-color: var(--accent);
    background: var(--panel-2);
  }
  h4 {
    margin: 0 0 4px;
    font-size: 14px;
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .symbol {
    font-size: 12px;
    color: var(--accent);
  }
  p {
    margin: 4px 0;
    line-height: 1.45;
  }
  .short {
    font-weight: 500;
  }
</style>
