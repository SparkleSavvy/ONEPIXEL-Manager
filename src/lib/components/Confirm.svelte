<script lang="ts">
  let {
    open = false,
    title = "Confirm",
    body = "",
    confirmLabel = "Delete",
    onconfirm,
    onclose,
  }: {
    open?: boolean;
    title?: string;
    body?: string;
    confirmLabel?: string;
    onconfirm: () => void;
    onclose: () => void;
  } = $props();
</script>

{#if open}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) onclose();
    }}
  >
    <div class="modal card" role="dialog" aria-modal="true" aria-label={title}>
      <h3>{title}</h3>
      <p class="muted" style="margin-top:8px">{body}</p>
      <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:18px">
        <button class="btn btn-sm" onclick={onclose}>Cancel</button>
        <button
          class="btn btn-sm"
          onclick={() => {
            onconfirm();
            onclose();
          }}
        >
          {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    width: 380px;
    background: var(--surface-2);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.6);
  }
</style>
