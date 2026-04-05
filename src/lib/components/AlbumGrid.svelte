<script lang="ts">
  import type { Album } from '../types';
  import AlbumCard from './AlbumCard.svelte';

  const COLS = 4;
  const ROWS = 3;
  const PER_PAGE = COLS * ROWS;

  let {
    albums,
    onselect,
    onhover,
  }: {
    albums: Album[];
    onselect: (album: Album) => void;
    onhover: (album: Album | null) => void;
  } = $props();

  let currentPage = $state(0);
  let scrollCooldown = false;

  let totalPages = $derived(Math.max(1, Math.ceil(albums.length / PER_PAGE)));

  $effect(() => {
    if (albums) currentPage = 0;
  });

  function nextPage() {
    if (currentPage < totalPages - 1) currentPage++;
  }

  function prevPage() {
    if (currentPage > 0) currentPage--;
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    if (scrollCooldown) return;
    scrollCooldown = true;
    setTimeout(() => (scrollCooldown = false), 550);

    if (e.deltaY > 0) nextPage();
    else prevPage();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="wrapper" onwheel={onWheel}>
  <div class="stage">
    <div
      class="slider"
      style="transform: translateX({-currentPage * 100}%)"
    >
      {#each Array(totalPages) as _, pageIdx}
        <div class="page">
          <div class="grid">
            {#each albums.slice(pageIdx * PER_PAGE, (pageIdx + 1) * PER_PAGE) as album (album.id)}
              <AlbumCard {album} onclick={() => onselect(album)} onhover={(a) => onhover(a)} />
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

  /* 3D perspective stage */
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
    align-items: center;
    justify-content: center;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(4, 140px);
    grid-template-rows: repeat(3, auto);
    gap: 34px 44px;
  }
</style>
