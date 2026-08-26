<script lang="ts">
  import { downloads } from "../downloads.svelte";
  import { formatBytes } from "../api";

  let {
    id,
    oncancel,
  }: {
    id: string;
    oncancel?: () => void;
  } = $props();

  const state = $derived(downloads.state(id));
  const percent = $derived(downloads.percent(id));
  const label = $derived(
    state
      ? state.phase === "extracting"
        ? `Extracting ${percent}%`
        : `${formatBytes(state.received)} / ${formatBytes(state.total)}`
      : "",
  );
</script>

<div class="progress-row">
  <div class="progress">
    <div class="progress-fill" style:width="{state?.phase === 'done' ? 100 : percent}%"></div>
  </div>
  <span class="progress-num">{label}</span>
  {#if oncancel && (downloads.active(id) || state?.phase === "downloading")}
    <button class="btn btn-ghost btn-sm" onclick={oncancel}>Cancel</button>
  {/if}
</div>
