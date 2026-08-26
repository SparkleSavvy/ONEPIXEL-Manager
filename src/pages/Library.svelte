<script lang="ts">
  import { onMount } from "svelte";

  import { api, formatBytes, formatUnix } from "../lib/api";
  import { downloads } from "../lib/downloads.svelte";
  import { servers } from "../lib/servers.svelte";
  import { toasts } from "../lib/toast.svelte";
  import type { LibrarySnapshot } from "../lib/types";
  import Confirm from "../lib/components/Confirm.svelte";

  let snap = $state<LibrarySnapshot | null>(null);

  let pendingDeleteVersion = $state<string | null>(null);
  let pendingDeleteServer = $state<string | null>(null);

  let installingFabric = $state<Record<string, boolean>>({});

  async function refresh() {
    snap = await api.listLibrary();
    for (const tag of snap.running) servers.running[tag] = true;
  }

  async function install(tag: string) {
    try {
      const res = await api.installToLauncher(tag);
      toasts.show(res.message);
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function installFabric(tag: string) {
    if (installingFabric[tag]) return;
    installingFabric[tag] = true;
    try {
      const msg = await api.installFabricServer(tag);
      toasts.show(msg);
      await refresh();
    } catch (e) {
      toasts.error(String(e));
    } finally {
      installingFabric[tag] = false;
    }
  }

  function openFolder(dir: string) {
    api.revealPath(dir).catch((e) => toasts.error(String(e)));
  }

  async function deleteVersion(tag: string) {
    try {
      await api.deleteVersion(tag);
      await refresh();
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function deleteServer(tag: string) {
    try {
      await api.deleteServer(tag);
      delete servers.logs[tag];
      await refresh();
    } catch (e) {
      toasts.error(String(e));
    }
  }

  onMount(() => {
    refresh();
    const offStatus = servers.onChange(refresh);
    const offDone = downloads.onDone(() => refresh());
    return () => {
      offStatus();
      offDone();
    };
  });

  const versionFileBadges = (files: { kind: string | null; name: string; size: number }[]) =>
    files.map((f) => ({
      label:
        f.kind === "client_pack"
          ? "mrpack"
          : f.kind === "full_zip"
            ? "zip"
            : f.name,
      size: f.size,
    }));
</script>

<div class="page-head">
  <div>
    <h1 class="page-title">Library</h1>
    <p class="page-subtitle">Downloaded versions and server packs stored on this machine.</p>
  </div>
  <button class="btn btn-sm" onclick={refresh}>Refresh</button>
</div>

<span class="section-label" style="margin-top:0">Modpack versions</span>

{#if !snap?.versions.length}
  <div class="empty-state">
    Nothing here yet — download a version from the Versions page.
  </div>
{:else}
  <div class="stack">
    {#each snap.versions as v (v.tag)}
      <article class="card">
        <div class="row">
          <div style="display:flex;align-items:center;gap:12px;min-width:0;flex-wrap:wrap">
            <span class="mono" style="font-size:14.5px;font-weight:600">{v.tag}</span>
            {#each versionFileBadges(v.files) as b (b.label)}
              <span class="badge">{b.label} \u00b7 {formatBytes(b.size)}</span>
            {/each}
            {#if v.installedAt}
              <span class="faint small">{formatUnix(v.installedAt)}</span>
            {/if}
          </div>
          <div style="display:flex;gap:8px;flex-shrink:0">
            <button class="btn btn-primary btn-sm" onclick={() => install(v.tag)}>
              Install to launcher
            </button>
            <button class="btn btn-sm" onclick={() => openFolder(v.dir)}>Open folder</button>
            <button
              class="btn btn-sm btn-danger"
              onclick={() => (pendingDeleteVersion = v.tag)}
              title={v.files.map((f) => f.name).join(", ")}
            >
              Delete
            </button>
          </div>
        </div>
      </article>
    {/each}
  </div>
{/if}

<span class="section-label">Server packs</span>

{#if !snap?.servers.length}
  <div class="empty-state">
    No server packs installed. Releases that ship a server pack can be downloaded from the Versions page.
  </div>
{:else}
  <div class="stack">
    {#each snap.servers as s (s.tag)}
      {@const isRunning = !!servers.running[s.tag]}
      <article class="card">
        <div class="row">
          <div style="display:flex;align-items:center;gap:12px;flex-wrap:wrap">
            <span class="mono" style="font-size:14.5px;font-weight:600">{s.tag}</span>
            <span style="display:inline-flex;align-items:center;gap:7px" class="small muted">
              <span class="dot" class:on={isRunning}></span>
              {isRunning ? "Running" : "Stopped"}
            </span>
            {#if s.script}
              <span class="badge mono">{s.script.split(/[\\/]/).pop()}</span>
            {:else}
              <span class="badge">no start script found</span>
            {/if}
            {#if s.hasServerJar}
              <span class="badge" title="Runs via onepixel-start script with Aikar's flags">
                managed \u00b7 server.jar
              </span>
            {:else if s.script}
              <button
                type="button"
                class="btn btn-sm"
                disabled={installingFabric[s.tag]}
                title="Download the Fabric server launcher for 1.20.1 as server.jar and switch to a managed start script"
                onclick={() => installFabric(s.tag)}
              >
                {installingFabric[s.tag] ? "Installing Fabric\u2026" : "Install Fabric 1.20.1"}
              </button>
            {/if}
          </div>
          <div style="display:flex;gap:8px;flex-shrink:0">
            <button class="btn btn-sm" onclick={() => openFolder(s.dir)}>Open folder</button>
            <button class="btn btn-sm btn-danger" onclick={() => (pendingDeleteServer = s.tag)}>
              Delete
            </button>
          </div>
        </div>
      </article>
    {/each}
  </div>
{/if}

<Confirm
  open={pendingDeleteVersion !== null}
  title="Delete version"
  body={`Remove the downloaded files for ${pendingDeleteVersion ?? ""} from disk? This cannot be undone.`}
  confirmLabel="Delete"
  onconfirm={() => pendingDeleteVersion && deleteVersion(pendingDeleteVersion)}
  onclose={() => (pendingDeleteVersion = null)}
/>

<Confirm
  open={pendingDeleteServer !== null}
  title="Delete server pack"
  body={`Remove the extracted server ${pendingDeleteServer ?? ""} and all of its world data? This cannot be undone.`}
  confirmLabel="Delete"
  onconfirm={() => pendingDeleteServer && deleteServer(pendingDeleteServer)}
  onclose={() => (pendingDeleteServer = null)}
/>
