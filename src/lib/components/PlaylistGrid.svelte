<script lang="ts">
  import type { Playlist } from '../stores/playlists';
  import PlaylistCard from './PlaylistCard.svelte';
  import { playUiSfx } from '$lib/ui-sfx';

  const COLS = 4;
  const ROWS = 3;
  const PER_PAGE = COLS * ROWS;

  let {
    playlists,
    onselect,
    onhover,
  }: {
    playlists: Playlist[];
    onselect: (playlist: Playlist) => void;
    onhover: (playlist: Playlist | null) => void;
  } = $props();

  let currentPage = $state(0);
  let scrollCooldown = false;

  let totalPages = $derived(Math.max(1, Math.ceil(playlists.length / PER_PAGE)));

  $effect(() => {
    if (playlists.length === 0) currentPage = 0;
  });

  function nextPage() {
    if (currentPage < totalPages - 1) { playUiSfx('nextPrev'); currentPage++; }
  }

  function prevPage() {
    if (currentPage > 0) { playUiSfx('nextPrev'); currentPage--; }
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    if (scrollCooldown) return;
    scrollCooldown = true;
    setTimeout(() => (scrollCooldown = false), 550);
    if (e.deltaY > 0) nextPage(); else prevPage();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="wrapper" onwheel={onWheel}>
  <div class="stage">
    <div class="slider" style="transform: translateX({-currentPage * 100}%)">
      {#each Array(totalPages) as _, pageIdx}
        <div class="page">
          <div class="grid">
            {#each playlists.slice(pageIdx * PER_PAGE, (pageIdx + 1) * PER_PAGE) as playlist (playlist.id)}
              <PlaylistCard
                {playlist}
                onclick={() => onselect(playlist)}
                onhover={(p) => onhover(p)}
              />
            {/each}
          </div>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .wrapper {
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .stage {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .slider {
    width: 100%;
    height: 100%;
    display: flex;
    transition: transform 0.35s cubic-bezier(0.25, 0.46, 0.45, 0.94);
  }

  .page {
    flex-shrink: 0;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 8px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(4, 93px);
    grid-template-rows: repeat(3, auto);
    gap: 5px 30px;
  }
</style>
