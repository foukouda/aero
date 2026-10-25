<script lang="ts">
  import { app } from "../lib/state.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as toast (toast.id)}
    <div class="toast panel {toast.kind}" role={toast.kind === "error" ? "alert" : "status"}>
      <div class="text">
        <strong>{toast.title}</strong>
        <span>{toast.message}</span>
      </div>
      <button aria-label="Dismiss" onclick={() => app.dismiss(toast.id)}>✕</button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 16px;
    bottom: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 20;
    max-width: 420px;
  }
  .toast {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 12px;
    border-left: 4px solid var(--accent);
  }
  .toast.error {
    border-left-color: var(--danger);
  }
  .toast.warning {
    border-left-color: var(--warning);
  }
  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    word-break: break-word;
  }
  button {
    border: none;
    background: none;
    padding: 0 2px;
    color: var(--muted);
  }
</style>
