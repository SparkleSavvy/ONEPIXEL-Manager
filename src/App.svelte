<script lang="ts">
  import { onMount } from "svelte";

  import Sidebar from "./lib/components/Sidebar.svelte";
  import Versions from "./pages/Versions.svelte";
  import Library from "./pages/Library.svelte";
  import Settings from "./pages/Settings.svelte";
  import { downloads } from "./lib/downloads.svelte";
  import { servers } from "./lib/servers.svelte";
  import { toasts } from "./lib/toast.svelte";

  type Page = "versions" | "library" | "settings";
  let page = $state<Page>("versions");

  onMount(() => {
    downloads.init();
    servers.init();
  });
</script>

<div class="app-shell">
  <Sidebar bind:page />

  <main class="main-pane">
    {#if page === "versions"}
      <Versions />
    {:else if page === "library"}
      <Library />
    {:else}
      <Settings />
    {/if}
  </main>
</div>

<div class="toast-zone">
  {#each toasts.items as t (t.id)}
    <button
      type="button"
      class="toast"
      style:border-color={t.kind === "error" ? "#4a4a50" : undefined}
      onclick={() => toasts.dismiss(t.id)}
    >
      {t.text}
    </button>
  {/each}
</div>
