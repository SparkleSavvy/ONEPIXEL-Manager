<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  import { api, formatBytes, formatUnix } from "../lib/api";
  import { downloads } from "../lib/downloads.svelte";
  import { toasts } from "../lib/toast.svelte";
  import type { LibrarySnapshot, LogEvent, StatusEvent } from "../lib/types";
  import Confirm from "../lib/components/Confirm.svelte";

  let snap = $state<LibrarySnapshot | null>(null);
  let runningMap = $state<Record<string, boolean>>({});
  let logs = $state<Record<string, string[]>>({});
  let selectedServer = $state<string | null>(null);
  let commandText = $state("");
  let togglingOnline = $state<Record<string, boolean>>({});

  let pendingDeleteVersion = $state<string | null>(null);
  let pendingDeleteServer = $state<string | null>(null);

  let consoleEl: HTMLDivElement | undefined = $state();

  async function refresh() {
    snap = await api.listLibrary();
    for (const tag of snap.running) {
      if (!runningMap[tag]) runningMap[tag] = true;
      selectedServer ||= tag;
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
      runningMap[tag] = true;
      pushLog(tag, "[manager] server process started");
      selectedServer = tag;
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function stopServer(tag: string) {
    try {
      await api.stopServer(tag);
      pushLog(tag, "[manager] stop requested…");
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
        `online-mode=${srv.onlineMode} for ${tag}${runningMap[tag] ? " (restart the server to apply)" : ""}`,
      );
    } catch (e) {
      srv.onlineMode = !next;
      toasts.error(String(e));
    } finally {
      togglingOnline[tag] = false;
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
      delete logs[tag];
      await refresh();
    } catch (e) {
      toasts.error(String(e));
    }
  }

  function pushLog(tag: string, line: string) {
    const arr = logs[tag] ?? [];
    arr.push(line);
    // Cap the in-memory buffer.
    logs[tag] = arr.length > 2000 ? arr.slice(-1500) : arr;
  }

  $effect(() => {
    const tag = selectedServer;
    const lines = tag ? logs[tag]?.length ?? 0 : 0;
    void lines;
    if (consoleEl) consoleEl.scrollTop = consoleEl.scrollHeight;
  });

  onMount(() => {
    refresh();

    const unlistenStatus = listen<StatusEvent>("server-status", (e) => {
      const { tag, running, exitCode } = e.payload;
      runningMap[tag] = running;
      if (!running && exitCode !== null && exitCode !== undefined) {
        pushLog(tag, `[manager] process exited with code ${exitCode}`);
      }
      refresh();
    });

    const unlistenLog = listen<LogEvent>("server-log", (e) => {
      pushLog(e.payload.tag, e.payload.line);
    });

    const offDone = downloads.onDone(() => refresh());

    return () => {
      unlistenStatus.then((f) => f());
      unlistenLog.then((f) => f());
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
      {@const isRunning = runningMap[s.tag]}
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

{#if selectedServer}
  <span class="section-label">Console · {selectedServer}</span>
  <div class="console" bind:this={consoleEl}>
    {(logs[selectedServer] ?? []).join("\n")}
  </div>
  {#if runningMap[selectedServer]}
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
{:else if Object.keys(logs).length > 0}
  <span class="section-label">Console</span>
  <select
    onchange={(e) => (selectedServer = (e.currentTarget as HTMLSelectElement).value)}
    style="margin-bottom:10px"
  >
    {#each Object.keys(logs) as t (t)}
      <option value={t}>{t}</option>
    {/each}
  </select>
  <div class="console" bind:this={consoleEl}>
    {(logs[selectedServer!] ?? []).join("\n")}
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
