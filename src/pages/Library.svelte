<script lang="ts">
  import { onMount } from "svelte";

  import { api, formatBytes, formatUnix } from "../lib/api";
  import { downloads } from "../lib/downloads.svelte";
  import { servers } from "../lib/servers.svelte";
  import { toasts } from "../lib/toast.svelte";
  import type { LibrarySnapshot } from "../lib/types";
  import Confirm from "../lib/components/Confirm.svelte";

  let snap = $state<LibrarySnapshot | null>(null);
  let selectedServer = $state<string | null>(null);
  let commandText = $state("");
  let togglingOnline = $state<Record<string, boolean>>({});

  let pendingDeleteVersion = $state<string | null>(null);
  let pendingDeleteServer = $state<string | null>(null);

  const RAM_OPTIONS = [1024, 2048, 3072, 4096, 6144, 8192, 12288, 16384];
  let installingFabric = $state<Record<string, boolean>>({});

  let consoleEl: HTMLDivElement | undefined = $state();

  async function refresh() {
    snap = await api.listLibrary();
    // Keep the global running map in sync with the backend snapshot.
    for (const tag of snap.running) servers.running[tag] = true;
    if (!selectedServer) {
      const firstRunning = Object.keys(servers.running).find((t) => servers.running[t]);
      selectedServer = firstRunning ?? Object.keys(servers.logs)[0] ?? null;
    }
  }

  async function install(tag: string) {
    try {
      const res = await api.installToLauncher(tag);
      toasts.show(res.message);
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function startServer(tag: string) {
    try {
      await api.startServer(tag);
      servers.running[tag] = true;
      servers.pushLog(tag, "[manager] server process started");
      selectedServer = tag;
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function stopServer(tag: string) {
    try {
      await api.stopServer(tag);
      servers.pushLog(tag, "[manager] stop requested…");
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function sendCommand() {
    if (!selectedServer) return;
    const cmd = commandText.trim();
    if (!cmd) return;
    try {
      await api.sendServerCommand(selectedServer, cmd);
      commandText = "";
    } catch (e) {
      toasts.error(String(e));
    }
  }

  function onCommandKey(e: KeyboardEvent) {
    if (e.key === "Enter") sendCommand();
  }

  async function toggleOnline(tag: string) {
    const srv = snap?.servers.find((x) => x.tag === tag);
    if (!srv || togglingOnline[tag]) return;
    const next = !(srv.onlineMode ?? true);
    srv.onlineMode = next;
    togglingOnline[tag] = true;
    try {
      srv.onlineMode = await api.setOnlineMode(tag, next);
      toasts.show(
        `online-mode=${srv.onlineMode} for ${tag}${servers.running[tag] ? " (restart the server to apply)" : ""}`,
      );
    } catch (e) {
      srv.onlineMode = !next;
      toasts.error(String(e));
    } finally {
      togglingOnline[tag] = false;
    }
  }

  async function changeRam(tag: string, value: string) {
    const srv = snap?.servers.find((x) => x.tag === tag);
    if (!srv) return;
    const mb = Number(value);
    if (!Number.isFinite(mb)) return;
    const previous = srv.ramMb;
    srv.ramMb = mb;
    try {
      await api.setServerRam(tag, mb);
      toasts.show(
        `RAM set to ${formatBytes(mb * 1024 * 1024)} for ${tag} (applies on next start)`,
      );
    } catch (e) {
      srv.ramMb = previous;
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

  $effect(() => {
    const lines = selectedServer ? (servers.logs[selectedServer]?.length ?? 0) : 0;
    void lines;
    if (consoleEl) consoleEl.scrollTop = consoleEl.scrollHeight;
  });

  onMount(() => {
    refresh();
    // Refresh snapshot when a server starts/stops or a download finishes.
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

  const serverTagsWithLogs = $derived(Object.keys(servers.logs));
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
              <span class="badge">{b.label} · {formatBytes(b.size)}</span>
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

<span class="section-label">Servers</span>

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
            {#if s.propertiesPath}
              <span
                class="switch-wrap"
                title="Toggles `online-mode` in server.properties (applies on next server start)"
              >
                <span class="small muted">online-mode</span>
                <button
                  type="button"
                  class="switch"
                  class:on={s.onlineMode ?? true}
                  disabled={togglingOnline[s.tag]}
                  aria-label="Toggle online mode for {s.tag}"
                  onclick={() => toggleOnline(s.tag)}
                >
                  <span class="knob"></span>
                </button>
                <span class="mono small faint" style="min-width:32px">
                  {s.onlineMode ?? true ? "true" : "false"}
                </span>
              </span>
            {/if}
            {#if s.hasServerJar}
              <span
                class="switch-wrap"
                title="JVM heap for the managed start script (Aikar's flags, server.jar)"
              >
                <span class="small muted">RAM</span>
                <select
                  class="ram-select mono small"
                  value={String(s.ramMb ?? 6144)}
                  onchange={(e) => changeRam(s.tag, (e.currentTarget as HTMLSelectElement).value)}
                >
                  {#each RAM_OPTIONS as opt (opt)}
                    <option value={String(opt)}>{opt / 1024} GB</option>
                  {/each}
                </select>
              </span>
              <span class="badge" title="Runs via onepixel-start script with Aikar's flags">
                managed · server.jar
              </span>
            {:else if s.script}
              <button
                type="button"
                class="btn btn-sm"
                disabled={installingFabric[s.tag]}
                title="Download the Fabric server launcher for 1.20.1 as server.jar and switch to a managed start script"
                onclick={() => installFabric(s.tag)}
              >
                {installingFabric[s.tag] ? "Installing Fabric…" : "Install Fabric 1.20.1"}
              </button>
            {/if}
          </div>
          <div style="display:flex;gap:8px;flex-shrink:0">
            {#if isRunning}
              <button class="btn btn-sm" onclick={() => stopServer(s.tag)}>Stop</button>
            {:else}
              <button
                class="btn btn-primary btn-sm"
                disabled={!s.script}
                onclick={() => startServer(s.tag)}
              >
                Run server
              </button>
            {/if}
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

{#if selectedServer && servers.logs[selectedServer]}
  <span class="section-label">
    Console ·
    <select
      class="console-picker"
      value={selectedServer}
      onchange={(e) =>
        (selectedServer = (e.currentTarget as HTMLSelectElement).value || null)}
    >
      {#each serverTagsWithLogs as t (t)}
        <option value={t}>{t}{servers.running[t] ? " — running" : ""}</option>
      {/each}
    </select>
  </span>
  <div class="console" bind:this={consoleEl}>
    {#each servers.logs[selectedServer] ?? [] as line, i (i)}
      <span class:sys={line.startsWith("[manager]")}>{line}</span>{"\n"}
    {/each}
  </div>
  {#if servers.running[selectedServer]}
    <div class="cmd-row">
      <input
        class="cmd-input"
        type="text"
        placeholder="Send a command to the server — stop, say hello, whitelist add …"
        bind:value={commandText}
        onkeydown={onCommandKey}
      />
      <button class="btn btn-sm" onclick={sendCommand}>Send</button>
    </div>
  {:else}
    <p class="kbd-note" style="margin-top:8px">Start the server to send commands.</p>
  {/if}
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

<style>
  .console-picker {
    font-family: var(--mono);
    text-transform: none;
    letter-spacing: normal;
    font-size: 11.5px;
    padding: 2px 6px;
    vertical-align: middle;
    margin-left: 6px;
  }
</style>
