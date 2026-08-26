<script lang="ts">
  import { api } from "../api";
  import { downloads } from "../downloads.svelte";

  const TICK_MS = 500;

  const activeEntries = $derived(
    Object.values(downloads.map).filter((d) => d.phase !== "done"),
  );

  let prevReceived = $state<Record<string, number>>({});
  let prevTime = $state<Record<string, number>>({});
  let speeds = $state<Record<string, number>>({});

  $effect(() => {
    const now = Date.now();
    for (const d of activeEntries) {
      if (!(d.id in prevReceived)) {
        prevReceived[d.id] = d.received;
        prevTime[d.id] = now;
        speeds[d.id] = 0;
      }
    }
  });

  $effect(() => {
    const id = setInterval(() => {
      const now = Date.now();
      const current = downloads.map;
      for (const d of Object.values(current)) {
        if (d.phase === "done") continue;
        const prev = prevReceived[d.id];
        const prevT = prevTime[d.id];
        if (prev !== undefined && prevT !== undefined) {
          const dt = (now - prevT) / 1000;
          if (dt > 0) {
            speeds[d.id] = Math.round((d.received - prev) / dt);
          }
          prevReceived[d.id] = d.received;
          prevTime[d.id] = now;
        }
      }
    }, TICK_MS);
    return () => clearInterval(id);
  });

  function cancel(id: string) {
    api.cancelDownload(id).catch(() => {});
  }

  function tagFromId(id: string): string {
    const idx = id.indexOf(":");
    return idx >= 0 ? id.slice(idx + 1) : id;
  }

  function formatSpeed(bytesPerSec: number): string {
    if (bytesPerSec < 1024) return `${bytesPerSec} B/s`;
    if (bytesPerSec < 1048576) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
    return `${(bytesPerSec / 1048576).toFixed(1)} MB/s`;
  }
</script>

{#if activeEntries.length > 0}
  <div class="dl-info">
    {#each activeEntries as d (d.id)}
      {@const pct = downloads.percent(d.id)}
      {@const speed = speeds[d.id] ?? 0}
      <div class="dl-line">
        <span class="dl-tag">{tagFromId(d.id)}</span>
        <span class="dl-speed">{formatSpeed(speed)}</span>
        <span class="dl-pct">{pct}%</span>
        <button class="btn btn-ghost btn-sm dl-cancel" onclick={() => cancel(d.id)}>Cancel</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .dl-info {
    padding: 0 10px 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .dl-line {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--text-dim);
    line-height: 1;
    padding: 3px 0;
  }

  .dl-tag {
    color: var(--text);
    font-weight: 600;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .dl-speed {
    color: var(--text-faint);
    white-space: nowrap;
  }

  .dl-pct {
    color: var(--text-dim);
    min-width: 28px;
    text-align: right;
    white-space: nowrap;
  }

  .dl-cancel {
    font-size: 10px;
    padding: 1px 4px;
    color: var(--text-faint);
    flex-shrink: 0;
  }

  .dl-cancel:hover {
    color: var(--text);
  }
</style>
