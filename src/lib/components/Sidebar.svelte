<script lang="ts">
  import { version } from "../../../package.json";
  import { servers } from "../servers.svelte";
  import DownloadInfo from "./DownloadInfo.svelte";

  type Page = "versions" | "library" | "server" | "settings";

  let { page = $bindable("versions") }: { page?: Page } = $props();

  const items: { id: Page; label: string }[] = [
    { id: "versions", label: "Versions" },
    { id: "library", label: "Library" },
    { id: "server", label: "Server" },
    { id: "settings", label: "Settings" },
  ];

  const glyphs: Record<Page, string> = {
    versions:
      "M12 3v12m0 0l-4-4m4 4l4-4M4 17v2a2 2 0 002 2h12a2 2 0 002-2v-2",
    library: "M4 7l8-4 8 4v10l-8 4-8-4V7zm8 0v14M12 7L4 7m8 0l8 0",
    server:
      "M4 5h16a1 1 0 011 1v4a1 1 0 01-1 1H4a1 1 0 01-1-1V6a1 1 0 011-1zM4 13h16a1 1 0 011 1v4a1 1 0 01-1 1H4a1 1 0 01-1-1v-4a1 1 0 011-1zM7 8h.01M7 16h.01",
    settings:
      "M12 15a3 3 0 100-6 3 3 0 000 6zm7.4-3a7.4 7.4 0 00-.1-1.2l2-1.5-2-3.4-2.3 1a7.4 7.4 0 00-2-1.2L14.6 3h-5l-.4 2.5a7.4 7.4 0 00-2 1.2l-2.3-1-2 3.4 2 1.5a7.4 7.4 0 000 2.4l-2 1.5 2 3.4 2.3-1a7.4 7.4 0 002 1.2l.4 2.5h5l.4-2.5a7.4 7.4 0 002-1.2l2.3 1 2-3.4-2-1.5c.06-.4.1-.8.1-1.2z",
  };

  const runningTags = $derived(
    Object.keys(servers.running).filter((t) => servers.running[t]),
  );
</script>

<aside class="sidebar">
  <div class="brand">
    <div class="brand-pixel"></div>
    <div class="brand-name">
      ONEPIXEL<br /><span>MANAGER</span>
    </div>
  </div>

  <nav class="nav">
    {#each items as item (item.id)}
      <button
        class="nav-item"
        class:active={page === item.id}
        onclick={() => (page = item.id)}
      >
        <svg
          class="nav-glyph"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.6"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d={glyphs[item.id]} />
        </svg>
        {item.label}
      </button>
    {/each}
  </nav>

  <DownloadInfo />

  <div class="sidebar-footer">
    <span>v{version}</span>
    {#if runningTags.length > 0}
      <button
        type="button"
        class="server-indicator on"
        title={`Running: ${runningTags.join(", ")}`}
        onclick={() => (page = "server")}
      >
        <svg
          viewBox="0 0 24 24"
          width="14"
          height="14"
          fill="none"
          stroke="currentColor"
          stroke-width="1.6"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d={glyphs.server} />
        </svg>
        {runningTags.length}
      </button>
    {:else}
      <span class="server-indicator" title="No servers running">
        <svg
          viewBox="0 0 24 24"
          width="14"
          height="14"
          fill="none"
          stroke="currentColor"
          stroke-width="1.6"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d={glyphs.server} />
        </svg>
      </span>
    {/if}
  </div>
</aside>
