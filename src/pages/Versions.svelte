<script lang="ts">
  import { onMount } from "svelte";

  import { api, formatBytes, formatDate } from "../lib/api";
  import { downloads } from "../lib/downloads.svelte";
  import { toasts } from "../lib/toast.svelte";
  import type { AssetInfo, DownloadKind, ReleaseInfo } from "../lib/types";
  import Progress from "../lib/components/Progress.svelte";

  let releases = $state<ReleaseInfo[]>([]);
  let loading = $state(true);
  let errorText = $state<string | null>(null);
  let expanded = $state<Record<string, boolean>>({});
  let installedIds = $state<Record<string, boolean>>({});

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

  const busyCount = $derived(
    Object.values(downloads.map).filter((d) => d.phase !== "done").length,
  );
</script>

<div class="page-head">
  <div>
    <h1 class="page-title">Versions</h1>
    <p class="page-subtitle">Every published build of the ONEPIXEL modpack.</p>
  </div>
  <div style="display:flex;align-items:center;gap:10px">
    {#if busyCount > 0}
      <span class="faint small">{busyCount} active</span>
    {/if}
    <button class="btn btn-sm" onclick={load} disabled={loading}>Refresh</button>
  </div>
</div>

{#if loading}
  <div class="empty-state">Loading releases…</div>
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
      <article class="card">
        <div class="row">
          <div style="min-width:0">
            <div style="display:flex;align-items:center;gap:10px;flex-wrap:wrap">
              <span class="mono" style="font-size:15px;font-weight:600">{r.tag}</span>
              <span class="muted">{r.name}</span>
              <span class="faint small">{formatDate(r.publishedAt)}</span>
            </div>
            {#if body}
              <p
                class="release-body"
                style:margin-top="8px"
                style:max-width="640px"
                style:white-space={expanded[r.tag] ? "pre-wrap" : "nowrap"}
                style:overflow="hidden"
                style:text-overflow={expanded[r.tag] ? "unset" : "ellipsis"}
              >
                {body}
              </p>
              {#if body.length > 140}
                <button
                  class="btn btn-ghost btn-sm"
                  style="padding-left:0;margin-top:2px"
                  onclick={() => (expanded[r.tag] = !expanded[r.tag])}
                >
                  {expanded[r.tag] ? "Show less" : "Show more"}
                </button>
              {/if}
            {/if}
          </div>

          <div style="display:flex;flex-direction:column;gap:8px;align-items:flex-end;flex-shrink:0">
            {#if client}
              {#if downloads.active(idFor("client", r.tag))}
                <div style="width:230px">
                  <Progress id={idFor("client", r.tag)} oncancel={() => cancel(idFor("client", r.tag))} />
                </div>
              {:else}
                <button
                  class="btn btn-primary btn-sm"
                  onclick={() => start("client", r.tag)}
                >
                  {installedIds[idFor("client", r.tag)] ? "Download again" : "Download mrpack"}
                  <span class="faint" style="color:inherit;opacity:.65">{formatBytes(client.size)}</span>
                </button>
              {/if}
            {/if}

            <div style="display:flex;gap:8px">
              {#if server}
                {#if downloads.active(idFor("server", r.tag))}
                  <div style="width:200px"><Progress id={idFor("server", r.tag)} /></div>
                {:else}
                  <button class="btn btn-sm" onclick={() => start("server", r.tag)}>
                    Server pack · {formatBytes(server.size)}
                  </button>
                {/if}
              {/if}
              {#if zip}
                {#if downloads.active(idFor("zip", r.tag))}
                  <div style="width:200px"><Progress id={idFor("zip", r.tag)} /></div>
                {:else}
                  <button class="btn btn-sm" onclick={() => start("zip", r.tag)}>
                    Full ZIP · {formatBytes(zip.size)}
                  </button>
                {/if}
              {/if}
            </div>
          </div>
        </div>
      </article>
    {/each}
  </div>
{/if}
