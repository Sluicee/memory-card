<script lang="ts">
  import { t } from "$lib/stores/i18n";
  import { isScanning, librarySize, scanStatus } from "$lib/stores/library";
  import type { Album, Artist, Clip } from "$lib/types";
  import type { Playlist } from "$lib/stores/playlists";

  let {
    activeTab,
    searchOpen = $bindable(false),
    searchQuery = $bindable(""),
    hoveredAlbum = null,
    hoveredArtist = null,
    hoveredPlaylist = null,
    hoveredClip = null,
    selectedArtistFilter = $bindable(null),
    onSearchKey,
    onClearArtistFilter,
  }: {
    activeTab: "library" | "artists" | "playlists" | "queue" | "clips";
    searchOpen?: boolean;
    searchQuery?: string;
    hoveredAlbum?: Album | null;
    hoveredArtist?: Artist | null;
    hoveredPlaylist?: Playlist | null;
    hoveredClip?: Clip | null;
    selectedArtistFilter?: string | null;
    onSearchKey?: (e: KeyboardEvent) => void;
    onClearArtistFilter?: () => void;
  } = $props();

  let searchInput = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (searchOpen) {
      setTimeout(() => searchInput?.focus(), 30);
    }
  });
</script>

<header class="header">
  <div class="header-left">
    <div class="mc-card"></div>
    <div class="memory-block">
      <span class="memory-label">{$t("memoryCard")}</span>
      {#if $librarySize !== "0 MB"}
        <span class="lib-size">{$librarySize}</span>
      {/if}
    </div>
  </div>

  <div class="header-right">
    {#if searchOpen}
      <input
        bind:this={searchInput}
        bind:value={searchQuery}
        onkeydown={onSearchKey}
        class="search-input"
        placeholder={$t("searchPlaceholder")}
        autocomplete="off"
        spellcheck="false"
      />
    {:else if $isScanning}
      <div class="scan-status">
        <span class="scan-status-text">
          {#if $scanStatus.totalFiles > 0}
            {$scanStatus.filesScanned.toLocaleString()} / {$scanStatus.totalFiles.toLocaleString()}
            · {$scanStatus.albumsFound} albums
          {:else}
            {$t("scanning")}
          {/if}
        </span>
        <div class="scan-status-bar">
          <div
            class="scan-status-fill"
            style="width: {$scanStatus.totalFiles > 0
              ? Math.round(
                  ($scanStatus.filesScanned / $scanStatus.totalFiles) * 100,
                )
              : 0}%"
          ></div>
        </div>
      </div>
    {/if}
    {#if activeTab === "library" && hoveredAlbum && !selectedArtistFilter}
      <span class="hovered-title" class:hovered-title--small={searchOpen}
        >{hoveredAlbum.title}</span
      >
    {:else if selectedArtistFilter && activeTab === "library"}
      <span
        class="hovered-title"
        style="display:flex; align-items:center; gap: 8px;"
      >
        {selectedArtistFilter}
        <button
          class="action-btn"
          onclick={onClearArtistFilter}
          style="font-size: 20px; color: var(--text-dim)">&times;</button
        >
      </span>
    {:else if activeTab === "artists" && hoveredArtist}
      <span class="hovered-title">{hoveredArtist.name}</span>
    {:else if activeTab === "playlists" && hoveredPlaylist}
      <span class="hovered-title">{hoveredPlaylist.name}</span>
    {:else if activeTab === "clips" && hoveredClip}
      <span class="hovered-title">{hoveredClip.title}</span>
    {/if}
  </div>
</header>
