<script lang="ts">
  import { onMount } from "svelte";

  import { api, formatBytes, formatDate } from "../lib/api";
  import { downloads } from "../lib/downloads.svelte";
  import { toasts } from "../lib/toast.svelte";
  import type { AssetInfo, DownloadKind, ReleaseInfo } from "../lib/types";

  let releases = $state<ReleaseInfo[]>([]);
  let loading = $state(true);
  let errorText = $state<string | null>(null);
  let expanded = $state<Record<string, boolean>>({});
  let installedIds = $state<Record<string, boolean>>({});

  const TICK_MS = 500;
  let prevReceived = $state<Record<string, number>>({});
  let prevTime = $state<Record<string, number>>({});
  let speeds = $state<Record<string, number>>({});

  const idFor = (kind: DownloadKind, tag: string) => `${kind}:${tag}`;

  function assetOf(r: ReleaseInfo, kind: DownloadKind): AssetInfo | undefined {
    const map: Record<DownloadKind, string> = {
      client: "client_pack",
      server: "server_pack",
      zip: "full_zip",
    };
    return r.assets.find((a) => a.kind === map[kind]);
  }

  async function load() {
    loading = true;
    errorText = null;
    try {
      releases = await api.fetchReleases();
    } catch (e) {
      errorText = String(e);
    } finally {
      loading = false;
    }
  }

  async function start(kind: DownloadKind, tag: string) {
    try {
      await api.startDownload(kind, tag);
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function cancel(id: string) {
    try {
      await api.cancelDownload(id);
    } catch (e) {
      toasts.error(String(e));
    }
  }

  function bodyPreview(body: string | null): string {
    if (!body) return "";
    return body.replace(/^>.*$/gm, "").trim();
  }

  function formatSpeed(bytesPerSec: number): string {
    if (bytesPerSec < 1024) return `${bytesPerSec} B/s`;
    if (bytesPerSec < 1048576) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
    return `${(bytesPerSec / 1048576).toFixed(1)} MB/s`;
  }

  $effect(() => {
    const now = Date.now();
    for (const d of Object.values(downloads.map)) {
      if (d.phase === "done") continue;
      if (!(d.id in prevReceived)) {
        prevReceived[d.id] = d.received;
        prevTime[d.id] = now;
        speeds[d.id] = 0;
      }
    }
  });

  $effect(() => {
    const interval = setInterval(() => {
      const now = Date.now();
      for (const d of Object.values(downloads.map)) {
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
    return () => clearInterval(interval);
  });

  onMount(() => {
    load();
    return downloads.onDone((id, res) => {
      if (!res.ok) {
        if (res.message && res.message !== "cancelled") {
          toasts.error(`Download failed: ${res.message}`);
        }
        return;
      }
      const [kind, tag] = id.split(":");
      if (!installedIds[id]) {
        installedIds[id] = true;
        toasts.show(
          kind === "server"
            ? `Server pack ${tag} extracted to library`
            : `${tag} saved to library`,
        );
      }
    });
  });
</script>

<div class="page-head">
  <div>
    <h1 class="page-title">Versions</h1>
    <p class="page-subtitle">Every published build of the ONEPIXEL modpack.</p>
  </div>
  <button class="btn btn-sm" onclick={load} disabled={loading}>Refresh</button>
</div>

{#if loading}
  <div class="empty-state">Loading releases...</div>
{:else if errorText}
  <div class="empty-state">
    <p>{errorText}</p>
    <button class="btn btn-sm" style="margin-top:12px" onclick={load}>Retry</button>
  </div>
{:else if releases.length === 0}
  <div class="empty-state">No modpack releases found in the repository yet.</div>
{:else}
  <div class="stack">
    {#each releases as r (r.tag)}
      {@const client = assetOf(r, "client")}
      {@const server = assetOf(r, "server")}
      {@const zip = assetOf(r, "zip")}
      {@const body = bodyPreview(r.body)}
      <article class="card version-row">
        <div class="version-header">
          <div class="version-meta">
            <span class="mono" style="font-weight:600">{r.tag}</span>
            <span class="muted">{r.name}</span>
            <span class="faint small">{formatDate(r.publishedAt)}</span>
          </div>
          <div class="version-assets">
            {#if client}
              {#if downloads.active(idFor("client", r.tag))}
                {@const state = downloads.state(idFor("client", r.tag))}
                {@const pct = downloads.percent(idFor("client", r.tag))}
                {@const speed = speeds[idFor("client", r.tag)] ?? 0}
                <div class="dl-inline">
                  <div class="dl-progress">
                    <div class="dl-progress-fill" style:width="{state?.phase === 'done' ? 100 : pct}%"></div>
                  </div>
                  <span class="dl-label">{pct}%</span>
                  <span class="dl-speed">{formatSpeed(speed)}</span>
                  <button class="btn btn-ghost btn-sm dl-cancel" onclick={() => cancel(idFor("client", r.tag))}>Cancel</button>
                </div>
              {:else}
                <button class="btn btn-primary btn-sm" onclick={() => start("client", r.tag)}>
                  Download mrpack
                  <span class="faint" style="color:inherit;opacity:.65">{formatBytes(client.size)}</span>
                </button>
              {/if}
            {/if}
            {#if server}
              {#if downloads.active(idFor("server", r.tag))}
                {@const pct = downloads.percent(idFor("server", r.tag))}
                {@const speed = speeds[idFor("server", r.tag)] ?? 0}
                <div class="dl-inline">
                  <div class="dl-progress">
                    <div class="dl-progress-fill" style:width="{pct}%"></div>
                  </div>
                  <span class="dl-label">{pct}%</span>
                  <span class="dl-speed">{formatSpeed(speed)}</span>
                  <button class="btn btn-ghost btn-sm dl-cancel" onclick={() => cancel(idFor("server", r.tag))}>Cancel</button>
                </div>
              {:else}
                <button class="btn btn-sm" onclick={() => start("server", r.tag)}>
                  Server pack
                  <span class="faint" style="opacity:.65">{formatBytes(server.size)}</span>
                </button>
              {/if}
            {/if}
            {#if zip}
              {#if downloads.active(idFor("zip", r.tag))}
                {@const pct = downloads.percent(idFor("zip", r.tag))}
                {@const speed = speeds[idFor("zip", r.tag)] ?? 0}
                <div class="dl-inline">
                  <div class="dl-progress">
                    <div class="dl-progress-fill" style:width="{pct}%"></div>
                  </div>
                  <span class="dl-label">{pct}%</span>
                  <span class="dl-speed">{formatSpeed(speed)}</span>
                  <button class="btn btn-ghost btn-sm dl-cancel" onclick={() => cancel(idFor("zip", r.tag))}>Cancel</button>
                </div>
              {:else}
                <button class="btn btn-sm" onclick={() => start("zip", r.tag)}>
                  Full ZIP
                  <span class="faint" style="opacity:.65">{formatBytes(zip.size)}</span>
                </button>
              {/if}
            {/if}
          </div>
        </div>
        {#if body}
          {#if expanded[r.tag]}
            <pre class="release-body">{body}</pre>
            <button
              class="btn btn-ghost btn-sm"
              style="padding-left:0;margin-top:2px"
              onclick={() => (expanded[r.tag] = false)}
            >
              Show less
            </button>
          {:else if body.length > 140}
            <button
              class="btn btn-ghost btn-sm"
              style="padding-left:0;margin-top:4px"
              onclick={() => (expanded[r.tag] = true)}
            >
              Show release notes
            </button>
          {:else}
            <p class="release-body" style="margin-top:4px">{body}</p>
          {/if}
        {/if}
      </article>
    {/each}
  </div>
{/if}

<style>
  .version-row {
    padding: 12px 16px;
  }

  .version-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }

  .version-meta {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
    flex-wrap: wrap;
  }

  .version-assets {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .dl-inline {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .dl-progress {
    width: 80px;
    height: 3px;
    background: var(--surface-3);
    border-radius: 2px;
    overflow: hidden;
  }

  .dl-progress-fill {
    height: 100%;
    background: var(--text);
    border-radius: 2px;
    transition: width 0.3s linear;
  }

  .dl-label {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--text-dim);
    min-width: 30px;
    text-align: right;
  }

  .dl-speed {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--text-faint);
    min-width: 60px;
    white-space: nowrap;
  }

  .dl-cancel {
    font-size: 10.5px;
    color: var(--text-faint);
  }

  .dl-cancel:hover {
    color: var(--text);
  }

  .release-body {
    font-size: 12.5px;
    color: var(--text-dim);
    line-height: 1.5;
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
