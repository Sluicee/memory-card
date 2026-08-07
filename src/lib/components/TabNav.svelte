<script lang="ts">
  import { t } from "$lib/stores/i18n";
  import { userQueueItems } from "$lib/stores/player";

  let {
    activeTab = $bindable("library"),
    onSelectTab,
  }: {
    activeTab?: "library" | "artists" | "playlists" | "queue" | "clips";
    onSelectTab: (tab: "library" | "artists" | "playlists" | "queue" | "clips") => void;
  } = $props();
</script>

<div class="tab-toggle">
  <div
    class="tab-thumb"
    class:tab-thumb--artists={activeTab === "artists"}
    class:tab-thumb--playlists={activeTab === "playlists"}
    class:tab-thumb--queue={activeTab === "queue"}
    class:tab-thumb--clips={activeTab === "clips"}
  ></div>
  <button
    class="tab-opt"
    class:tab-opt--active={activeTab === "library"}
    onclick={() => onSelectTab("library")}
    >{$t("library")}</button
  >
  <button
    class="tab-opt"
    class:tab-opt--active={activeTab === "artists"}
    onclick={() => onSelectTab("artists")}
    >{$t("artistsTab")}</button
  >
  <button
    class="tab-opt"
    class:tab-opt--active={activeTab === "playlists"}
    onclick={() => onSelectTab("playlists")}
    >{$t("playlists")}</button
  >
  <button
    class="tab-opt tab-opt--clips"
    class:tab-opt--active={activeTab === "clips"}
    onclick={() => onSelectTab("clips")}
    >{$t("clipsTab")}</button
  >
  <button
    class="tab-opt tab-opt--queue"
    class:tab-opt--active={activeTab === "queue"}
    onclick={() => onSelectTab("queue")}
  >
    {$t("queueTab")}
    {#if $userQueueItems.length > 0}
      <span class="queue-badge">{$userQueueItems.length}</span>
    {/if}
  </button>
</div>
