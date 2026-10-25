<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { TOUR_STEPS, placeCard, type Box } from "../lib/tour";

  const total = TOUR_STEPS.length;
  const index = $derived(app.tourIndex);
  const step = $derived(index === null ? null : TOUR_STEPS[index]);
  const text = $derived(step ? step.text[app.tourLang] : null);
  const missingAirfoil = $derived(!!step?.needsAirfoil && app.airfoils.length === 0);
  const ui = $derived(
    app.tourLang === "fr"
      ? { step: "Étape", back: "Précédent", next: "Suivant", finish: "Terminer", start: "Commencer la visite", skip: "Passer", close: "Fermer la visite", needs: "Crée d'abord un profil pour voir cette partie.", create: "Créer un NACA 2412" }
      : { step: "Step", back: "Back", next: "Next", finish: "Finish", start: "Start the tour", skip: "Skip", close: "Close the tour", needs: "Create an airfoil first to see this part.", create: "Create a NACA 2412" },
  );

  let target = $state<Box | null>(null);
  let cardSize = $state({ width: 380, height: 240 });
  let view = $state({ width: window.innerWidth, height: window.innerHeight });
  let card: HTMLDivElement | undefined = $state();
  let busy = $state(false);

  // Follow the highlighted element (layout changes, tab switches, resizes).
  $effect(() => {
    const current = step;
    if (!current) return;
    let frame = 0;
    let scrolled = false;
    const tick = () => {
      const el = current.target
        ? document.querySelector<HTMLElement>(`[data-tour="${current.target}"]`)
        : null;
      if (el && !scrolled) {
        el.scrollIntoView({ block: "nearest", inline: "nearest" });
        scrolled = true;
      }
      const r = el?.getBoundingClientRect();
      const next = r && r.width > 0 ? { left: r.left, top: r.top, width: r.width, height: r.height } : null;
      if (JSON.stringify(next) !== JSON.stringify(target)) target = next;
      if (card) {
        const c = card.getBoundingClientRect();
        if (c.width !== cardSize.width || c.height !== cardSize.height) {
          cardSize = { width: c.width, height: c.height };
        }
      }
      if (window.innerWidth !== view.width || window.innerHeight !== view.height) {
        view = { width: window.innerWidth, height: window.innerHeight };
      }
      frame = requestAnimationFrame(tick);
    };
    tick();
    return () => cancelAnimationFrame(frame);
  });

  const position = $derived(placeCard(target, cardSize, view));

  function go(delta: number) {
    if (index === null) return;
    if (index + delta >= total) app.endTour();
    else app.tourGo(index + delta);
  }

  async function act() {
    if (!step?.action) return;
    busy = true;
    try {
      await app.runTourAction(step.action);
    } finally {
      busy = false;
    }
  }

  async function createAirfoil() {
    busy = true;
    try {
      await app.runTourAction("generate-naca");
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (index === null) return;
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (e.key === "Escape") {
      app.endTour();
    } else if (tag !== "INPUT" && tag !== "TEXTAREA" && tag !== "SELECT") {
      if (e.key === "ArrowRight") go(1);
      else if (e.key === "ArrowLeft") go(-1);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if step && text && index !== null}
  {#if target}
    <div
      class="spotlight"
      style:left={`${target.left - 6}px`}
      style:top={`${target.top - 6}px`}
      style:width={`${target.width + 12}px`}
      style:height={`${target.height + 12}px`}
    ></div>
  {:else}
    <div class="dim"></div>
  {/if}

  <div
    class="card panel"
    bind:this={card}
    role="dialog"
    aria-modal="false"
    aria-labelledby="tour-title"
    style:left={`${position.left}px`}
    style:top={`${position.top}px`}
  >
    <header>
      <span class="muted step">{ui.step} {index + 1} / {total}</span>
      <div class="lang" role="group" aria-label="Language">
        <button class:active={app.tourLang === "en"} onclick={() => app.setTourLang("en")}>EN</button>
        <button class:active={app.tourLang === "fr"} onclick={() => app.setTourLang("fr")}>FR</button>
      </div>
      <button class="close" aria-label={ui.close} title={ui.close} onclick={() => app.endTour()}>✕</button>
    </header>

    <h3 id="tour-title">{text.title}</h3>
    <p>{text.body}</p>

    {#if missingAirfoil}
      <div class="notice">
        <span>{ui.needs}</span>
        <button disabled={busy} onclick={createAirfoil}>{ui.create}</button>
      </div>
    {:else if step.action && text.action}
      <button class="action" disabled={busy} onclick={act}>▶ {text.action}</button>
    {/if}

    <div class="progress" aria-hidden="true">
      {#each TOUR_STEPS as s, i (s.id)}
        <button class="dot" class:done={i < index} class:current={i === index} tabindex="-1" onclick={() => app.tourGo(i)} aria-label={`${ui.step} ${i + 1}`}></button>
      {/each}
    </div>

    <footer>
      {#if index === 0}
        <button onclick={() => app.endTour()}>{ui.skip}</button>
        <button class="primary" onclick={() => go(1)}>{ui.start}</button>
      {:else}
        <button onclick={() => go(-1)}>{ui.back}</button>
        <button class="primary" onclick={() => go(1)}>{index === total - 1 ? ui.finish : ui.next}</button>
      {/if}
    </footer>
  </div>
{/if}

<style>
  .spotlight {
    position: fixed;
    z-index: 40;
    border-radius: 10px;
    box-shadow: 0 0 0 9999px rgb(0 0 0 / 0.45);
    outline: 2px solid var(--accent);
    pointer-events: none;
    transition:
      left 0.2s,
      top 0.2s,
      width 0.2s,
      height 0.2s;
  }
  .dim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgb(0 0 0 / 0.45);
    pointer-events: none;
  }
  .card {
    position: fixed;
    z-index: 41;
    width: 380px;
    max-width: calc(100vw - 24px);
    padding: 14px 16px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    transition:
      left 0.2s,
      top 0.2s;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .step {
    flex: 1;
    font-size: 12px;
  }
  .lang {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .lang button {
    border: none;
    border-radius: 0;
    padding: 1px 7px;
    font-size: 11px;
    background: transparent;
  }
  .lang button.active {
    background: var(--accent);
    color: var(--accent-text);
  }
  .close {
    border: none;
    background: none;
    color: var(--muted);
    padding: 0 4px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .action {
    align-self: flex-start;
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
  .notice {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--panel-2);
    color: var(--warning);
  }
  .notice button {
    align-self: flex-start;
  }
  .progress {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
    margin-top: 2px;
  }
  .dot {
    width: 8px;
    height: 8px;
    padding: 0;
    border-radius: 50%;
    border: none;
    background: var(--border);
  }
  .dot.done {
    background: color-mix(in srgb, var(--accent) 45%, var(--border));
  }
  .dot.current {
    background: var(--accent);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
</style>
