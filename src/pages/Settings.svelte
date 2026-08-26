<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  import { api } from "../lib/api";
  import { toasts } from "../lib/toast.svelte";
  import type {
    DetectedJava,
    DetectedLauncher,
    UpdateStatus,
  } from "../lib/types";

  const KIND_OPTIONS: { id: string; label: string; hint: string }[] = [
    { id: "elyprism", label: "ElyPrism", hint: "Prism fork with Ely.by accounts — imported via -I flag" },
    { id: "prism", label: "Prism Launcher", hint: "Imported via the -I CLI flag" },
    { id: "xmcl", label: "XMCL", hint: "Opens the mrpack with the launcher" },
    { id: "custom", label: "Other / custom", hint: "Receives the mrpack path as an argument" },
  ];

  let selectedKind = $state("prism");
  let exePath = $state("");
  let detected = $state<DetectedLauncher[]>([]);
  let saving = $state(false);
  let savedFlash = $state(false);

  let updateStatus = $state<UpdateStatus | null>(null);
  let checkingUpdates = $state(false);

  let javaPathInput = $state("");
  let detectedJava = $state<DetectedJava | null>(null);
  let javaFlash = $state(false);

  async function loadJava() {
    try {
      const config = await api.getConfig();
      javaPathInput = config.javaPath ?? "";
      detectedJava = await api.detectJava();
    } catch (e) {
      detectedJava = null;
    }
  }

  async function saveJava() {
    try {
      await api.setJavaPath(javaPathInput.trim() || null);
      javaFlash = true;
      setTimeout(() => (javaFlash = false), 1600);
      await loadJava();
      toasts.show("Java settings saved");
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function browseJava() {
    const file = await open({
      multiple: false,
      filters: [{ name: "Executable", extensions: ["exe"] }],
    });
    if (typeof file === "string") {
      javaPathInput = file;
      saveJava();
    }
  }

  async function load() {
    try {
      const config = await api.getConfig();
      if (config.launcher) {
        selectedKind = config.launcher.kind;
        exePath = config.launcher.exePath ?? "";
      }
      detected = await api.detectLaunchers();
    } catch (e) {
      toasts.error(String(e));
    }
  }

  async function save() {
    saving = true;
    try {
      await api.setLauncher(selectedKind, exePath.trim() || null);
      savedFlash = true;
      setTimeout(() => (savedFlash = false), 1600);
      toasts.show("Launcher settings saved");
    } catch (e) {
      toasts.error(String(e));
    } finally {
      saving = false;
    }
  }

  function useDetected(d: DetectedLauncher) {
    selectedKind = d.kind;
    exePath = d.exePath;
    save();
  }

  async function browseExe() {
    const file = await open({
      multiple: false,
      filters: [{ name: "Executable", extensions: ["exe"] }],
    });
    if (typeof file === "string") {
      exePath = file;
    }
  }

  async function checkUpdate() {
    checkingUpdates = true;
    try {
      updateStatus = await api.checkUpdates();
    } catch (e) {
      toasts.error(String(e));
    } finally {
      checkingUpdates = false;
    }
  }

  onMount(() => {
    load();
    loadJava();
  });
</script>

<div class="page-head">
  <div>
    <h1 class="page-title">Settings</h1>
    <p class="page-subtitle">Choose where ONEPIXEL gets installed and how it launches.</p>
  </div>
</div>

<span class="section-label" style="margin-top:0">Launcher</span>

<div class="stack">
  {#each KIND_OPTIONS as opt (opt.id)}
    <button
      class="radio-row"
      class:selected={selectedKind === opt.id}
      onclick={() => (selectedKind = opt.id)}
    >
      <span class="radio-dot"></span>
      <span style="flex:1;text-align:left">
        <span style="font-weight:600">{opt.label}</span>
        <span class="faint small" style="display:block">{opt.hint}</span>
      </span>
    </button>
  {/each}
</div>

<div class="card" style="margin-top:12px">
  <div class="row">
    <input
      type="text"
      placeholder="Path to the launcher executable (optional)…"
      style="flex:1"
      bind:value={exePath}
    />
    <button class="btn btn-sm" onclick={browseExe}>Browse…</button>
    <button class="btn btn-primary btn-sm" onclick={save} disabled={saving}>
      {savedFlash ? "Saved ✓" : "Save"}
    </button>
  </div>
  <p class="kbd-note" style="margin-top:10px">
    If no executable is set, the downloaded .mrpack is opened through the system file
    association instead.
  </p>
</div>

{#if detected.length > 0}
  <span class="section-label">Detected on this machine</span>
  <div class="stack">
    {#each detected as d (d.exePath)}
      <div class="card row">
        <div style="min-width:0">
          <span class="mono small">{d.exePath}</span>
          <span class="badge" style="margin-left:10px">{d.kind}</span>
        </div>
        <button class="btn btn-sm" onclick={() => useDetected(d)}>Use this</button>
      </div>
    {/each}
  </div>
{/if}

<span class="section-label">Java</span>

<div class="card">
  <div class="row">
    <input
      type="text"
      placeholder="Path to java.exe — leave empty for automatic detection"
      style="flex:1"
      bind:value={javaPathInput}
    />
    <button class="btn btn-sm" onclick={browseJava}>Browse…</button>
    <button class="btn btn-primary btn-sm" onclick={saveJava}>
      {javaFlash ? "Saved" : "Save"}
    </button>
  </div>
  <p class="kbd-note" style="margin-top:10px">
    {#if detectedJava}
      Detected: <span class="mono">{detectedJava.path}</span> · Java {detectedJava.major}
    {:else}
      No Java 17+ found yet — a Temurin JDK 17 will be downloaded automatically on
      the first managed server start.
    {/if}
  </p>
</div>

<span class="section-label">Manager updates</span>

<div class="card">
  <div class="row">
    <div>
      <p>Current version <span class="mono">v{updateStatus?.currentVersion ?? "0.1.0"}</span></p>
      {#if updateStatus && !updateStatus.configured}
        <p class="kbd-note" style="margin-top:6px">
          The manager's own repository isn't published yet. Self-update will activate
          automatically once it exists.
        </p>
      {:else if updateStatus?.configured}
        <p class="kbd-note" style="margin-top:6px">
          {#if updateStatus.updateAvailable}
            Update available: <span class="mono">{updateStatus.latestVersion}</span>
            {#if updateStatus.url}
              —
              <a href={updateStatus.url} target="_blank" rel="noreferrer" style="color:var(--text)">open release</a>
            {/if}
          {:else}
            You are up to date.
          {/if}
        </p>
      {:else}
        <p class="kbd-note" style="margin-top:6px">
          Checks GitHub for a newer release of ONEPIXEL Manager.
        </p>
      {/if}
    </div>
    <button class="btn btn-sm" onclick={checkUpdate} disabled={checkingUpdates}>
      Check for updates
    </button>
  </div>
</div>

<style>
  a {
    color: var(--text);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
</style>
