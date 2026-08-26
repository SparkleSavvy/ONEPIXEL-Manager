<script lang="ts">
  import { onMount } from "svelte";

  import { api, formatBytes } from "../lib/api";
  import { downloads } from "../lib/downloads.svelte";
  import { servers } from "../lib/servers.svelte";
  import { toasts } from "../lib/toast.svelte";
  import type { LibrarySnapshot } from "../lib/types";
  import Confirm from "../lib/components/Confirm.svelte";

  let snap = $state<LibrarySnapshot | null>(null);
  let selectedTag = $state<string | null>(null);
  let commandText = $state("");
  let togglingOnline = $state<Record<string, boolean>>({});
  let pendingDelete = $state<string | null>(null);
  let installingFabric = $state<Record<string, boolean>>({});

  const RAM_OPTIONS = [1024, 2048, 3072, 4096, 6144, 8192, 12288, 16384];

  let consoleEl: HTMLDivElement | undefined = $state();

  const selected = $derived(
    snap?.servers.find((s) => s.tag === selectedTag) ?? null,
  );

  const isRunning = $derived(
    selectedTag ? !!servers.running[selectedTag] : false,
  );

  const serverTags = $derived(snap?.servers.map((s) => s.tag) ?? []);

  async function refresh() {
    snap = await api.listLibrary();
    for (const tag of snap.running) servers.running[tag] = true;
    if (!selectedTag) {
      selectedTag =
        Object.keys(servers.running).find((t) => servers.running[t]) ??
        snap.servers[0]?.tag ??
        null;
    }
  }

  async function startServer(tag: string) {
    try {
      await api.startServer(tag);
      servers.running[tag] = true;
      servers.pushLog(tag, "[manager] server process started");
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function stopServer(tag: string) {
    try {
      await api.stopServer(tag);
      servers.pushLog(tag, "[manager] stop requested\u2026");
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function sendCommand() {
    if (!selectedTag) return;
    const cmd = commandText.trim();
    if (!cmd) return;
    try {
      await api.sendServerCommand(selectedTag, cmd);
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

  async function deleteServer(tag: string) {
    try {
      await api.deleteServer(tag);
      delete servers.logs[tag];
      if (selectedTag === tag) selectedTag = null;
      await refresh();
    } catch (e) {
      toasts.error(String(e));
    }
  }

  $effect(() => {
    const lines = selectedTag ? (servers.logs[selectedTag]?.length ?? 0) : 0;
    void lines;
    if (consoleEl) consoleEl.scrollTop = consoleEl.scrollHeight;
  });

  onMount(() => {
    refresh();
    const offStatus = servers.onChange(refresh);
    const offDone = downloads.onDone(() => refresh());
    return () => {
      offStatus();
      offDone();
    };
  });
</script>

<div class="page-head">
  <div>
    <h1 class="page-title">Server</h1>
    <p class="page-subtitle">
      Start, stop and monitor your Minecraft server.
    </p>
  </div>
  <button class="btn btn-sm" onclick={refresh}>Refresh</button>
</div>

{#if !snap?.servers.length}
  <div class="empty-state">
    No server packs installed. Download a release with a server pack from the
    Versions page, then install it in Library.
  </div>
{:else}
  {#if serverTags.length > 1}
    <span class="section-label" style="margin-top:0">Select server</span>
    <div class="ctrl-row">
      <select
        class="cmd-input mono"
        value={selectedTag ?? ""}
        onchange={(e) =>
          (selectedTag = (e.currentTarget as HTMLSelectElement).value || null)}
      >
        {#each serverTags as t (t)}
          <option value={t}>
            {t}{servers.running[t] ? " \u2014 running" : ""}
          </option>
        {/each}
      </select>
    </div>
  {/if}

  {#if selected}
    <span class="section-label" style="margin-top:0">
      {selected.tag}
    </span>

    <article class="card" style="padding:16px">
      <div
        style="display:flex;align-items:center;gap:16px;flex-wrap:wrap;margin-bottom:14px"
      >
        <span style="display:inline-flex;align-items:center;gap:7px" class="small muted">
          <span class="dot" class:on={isRunning}></span>
          {isRunning ? "Running" : "Stopped"}
        </span>
        {#if selected.script}
          <span class="badge mono">{selected.script.split(/[\\/]/).pop()}</span>
        {:else}
          <span class="badge">no start script found</span>
        {/if}
        {#if selected.hasServerJar}
          <span class="badge" title="Runs via onepixel-start script with Aikar's flags">
            managed \u00b7 server.jar
          </span>
        {:else if selected.script}
          <button
            type="button"
            class="btn btn-sm"
            disabled={installingFabric[selected.tag]}
            title="Download the Fabric server launcher for 1.20.1 as server.jar and switch to a managed start script"
            onclick={() => installFabric(selected.tag)}
          >
            {installingFabric[selected.tag] ? "Installing Fabric\u2026" : "Install Fabric 1.20.1"}
          </button>
        {/if}
      </div>

      <div
        style="display:flex;align-items:center;gap:16px;flex-wrap:wrap;margin-bottom:14px"
      >
        {#if selected.propertiesPath}
          <span
            class="switch-wrap"
            title="Toggles `online-mode` in server.properties (applies on next server start)"
          >
            <span class="small muted">online-mode</span>
            <button
              type="button"
              class="switch"
              class:on={selected.onlineMode ?? true}
              disabled={togglingOnline[selected.tag]}
              aria-label="Toggle online mode for {selected.tag}"
              onclick={() => toggleOnline(selected.tag)}
            >
              <span class="knob"></span>
            </button>
            <span class="mono small faint" style="min-width:32px">
              {selected.onlineMode ?? true ? "true" : "false"}
            </span>
          </span>
        {/if}
        {#if selected.hasServerJar}
          <span
            class="switch-wrap"
            title="JVM heap for the managed start script (Aikar's flags, server.jar)"
          >
            <span class="small muted">RAM</span>
            <select
              class="ram-select mono small"
              value={String(selected.ramMb ?? 6144)}
              onchange={(e) =>
                changeRam(selected.tag, (e.currentTarget as HTMLSelectElement).value)}
            >
              {#each RAM_OPTIONS as opt (opt)}
                <option value={String(opt)}>{opt / 1024} GB</option>
              {/each}
            </select>
          </span>
        {/if}
      </div>

      <div style="display:flex;gap:8px;flex-wrap:wrap">
        {#if isRunning}
          <button class="btn btn-sm" onclick={() => stopServer(selected.tag)}>Stop</button>
        {:else}
          <button
            class="btn btn-primary btn-sm"
            disabled={!selected.script}
            onclick={() => startServer(selected.tag)}
          >
            Run server
          </button>
        {/if}
        <button class="btn btn-sm" onclick={() => openFolder(selected.dir)}>Open folder</button>
        <button class="btn btn-sm btn-danger" onclick={() => (pendingDelete = selected.tag)}>
          Delete
        </button>
      </div>
    </article>

    {#if selectedTag && servers.logs[selectedTag]}
      <span class="section-label">
        Console
        {#if serverTags.length > 1}
          <select
            class="console-picker"
            value={selectedTag}
            onchange={(e) =>
              (selectedTag = (e.currentTarget as HTMLSelectElement).value || null)}
          >
            {#each serverTags as t (t)}
              <option value={t}>{t}{servers.running[t] ? " \u2014 running" : ""}</option>
            {/each}
          </select>
        {/if}
      </span>
      <div class="console" bind:this={consoleEl}>
        {#each servers.logs[selectedTag] ?? [] as line, i (i)}
          <span class:sys={line.startsWith("[manager]")}>{line}</span>{"\n"}
        {/each}
      </div>
      {#if servers.running[selectedTag]}
        <div class="cmd-row">
          <input
            class="cmd-input"
            type="text"
            placeholder="Send a command to the server \u2014 stop, say hello, whitelist add \u2026"
            bind:value={commandText}
            onkeydown={onCommandKey}
          />
          <button class="btn btn-sm" onclick={sendCommand}>Send</button>
        </div>
      {:else}
        <p class="kbd-note" style="margin-top:8px">Start the server to send commands.</p>
      {/if}
    {/if}
  {:else}
    <div class="empty-state">Select a server above.</div>
  {/if}
{/if}

<Confirm
  open={pendingDelete !== null}
  title="Delete server pack"
  body={`Remove the extracted server ${pendingDelete ?? ""} and all of its world data? This cannot be undone.`}
  confirmLabel="Delete"
  onconfirm={() => pendingDelete && deleteServer(pendingDelete)}
  onclose={() => (pendingDelete = null)}
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
