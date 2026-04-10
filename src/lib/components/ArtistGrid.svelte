<script lang="ts">
  import type { Artist } from '../types';
  import ArtistCard from './ArtistCard.svelte';
  import { playUiSfx } from '$lib/ui-sfx';
  import { tick } from 'svelte';

  const COLS = 4;
  const ROWS = 3;
  const PER_PAGE = COLS * ROWS;

  let {
    artists,
    onselect,
    onhover,
  }: {
    artists: Artist[];
    onselect: (artist: Artist) => void;
    onhover: (artist: Artist | null) => void;
  } = $props();

  let currentPage = $state(0);
  let virtualIndex = $state(1);
  let noTransition = $state(false);
  let scrollCooldown = false;
  let prevLength = 0;
  let initialPageSet = false;

  let totalPages = $derived(Math.max(1, Math.ceil(artists.length / PER_PAGE)));

  function pageArtists(pageIdx: number): Artist[] {
    return artists.slice(pageIdx * PER_PAGE, (pageIdx + 1) * PER_PAGE);
  }

  $effect(() => {
    const len = artists.length;
    const tp = totalPages;

    if (len === 0) {
      currentPage = 0;
      virtualIndex = 1;
      initialPageSet = false;
    } else if (len < prevLength) {
      const clamped = Math.min(currentPage, tp - 1);
      currentPage = clamped;
      virtualIndex = clamped + 1;
    } else if (!initialPageSet) {
      currentPage = 0;
      virtualIndex = 1;
      initialPageSet = true;
    }
    prevLength = len;
  });

  async function snapTo(newVirtual: number, newPage: number) {
    noTransition = true;
    virtualIndex = newVirtual;
    currentPage = newPage;
    await tick();
    setTimeout(() => { noTransition = false; }, 20);
  }

  function nextPage() {
    playUiSfx('nextPrev');
    const next = virtualIndex + 1;
    virtualIndex = next;
    if (next > totalPages) {
      currentPage = 0;
      setTimeout(() => snapTo(1, 0), 370);
    } else {
      currentPage = next - 1;
    }
  }

  function prevPage() {
    playUiSfx('nextPrev');
    const prev = virtualIndex - 1;
    virtualIndex = prev;
    if (prev < 1) {
      currentPage = totalPages - 1;
      setTimeout(() => snapTo(totalPages, totalPages - 1), 370);
    } else {
      currentPage = prev - 1;
    }
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
      class:no-transition={noTransition}
      style="transform: translateX({-virtualIndex * 100}%)"
    >
      <!-- Clone of last page -->
      <div class="page">
        <div class="grid">
          {#each pageArtists(totalPages - 1) as artist (artist.name + '_lc')}
            <ArtistCard {artist} onclick={() => onselect(artist)} onhover={(a) => onhover(a)} />
          {/each}
        </div>
      </div>

      <!-- Real pages -->
      {#each Array(totalPages) as _, pageIdx}
        <div class="page">
          <div class="grid">
            {#each pageArtists(pageIdx) as artist (artist.name)}
              <ArtistCard
                {artist}
                onclick={() => onselect(artist)}
                onhover={(a) => onhover(a)}
              />
            {/each}
          </div>
        </div>
      {/each}

      <!-- Clone of first page -->
      <div class="page">
        <div class="grid">
          {#each pageArtists(0) as artist (artist.name + '_fc')}
            <ArtistCard {artist} onclick={() => onselect(artist)} onhover={(a) => onhover(a)} />
          {/each}
        </div>
      </div>
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

  .slider.no-transition {
    transition: none;
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
    grid-template-columns: repeat(4, 93px);
    grid-template-rows: repeat(3, auto);
    gap: 5px 30px;
  }
</style>
