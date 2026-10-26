<script lang="ts">
  import type { Snippet } from "svelte";
  import { glossaryEntry } from "../lib/glossary";
  import { app } from "../lib/state.svelte";

  interface Props {
    /** Glossary entry. */
    id: string;
    children: Snippet;
  }

  let { id, children }: Props = $props();

  const tip = $derived.by(() => {
    const entry = glossaryEntry(id);
    if (!entry) return "";
    const more = app.tourLang === "fr" ? "Clic : en savoir plus" : "Click for more";
    return `${entry.text[app.tourLang].short}\n${more}`;
  });

  function open(e: Event) {
    // Inside a label, do not also activate the labelled control.
    e.preventDefault();
    e.stopPropagation();
    app.openGlossary(id);
  }
</script>

<span
  class="term"
  role="button"
  tabindex="0"
  title={tip}
  onclick={open}
  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && open(e)}>{@render children()}</span
>

<style>
  .term {
    text-decoration: underline dotted;
    text-decoration-color: var(--muted);
    text-underline-offset: 3px;
    cursor: help;
  }
  .term:hover,
  .term:focus-visible {
    color: var(--accent);
    outline: none;
  }
</style>
