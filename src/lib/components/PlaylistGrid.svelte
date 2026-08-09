<script lang="ts">
  import type { Playlist } from '../stores/playlists';
  import PlaylistCard from './PlaylistCard.svelte';
  import { playUiSfx } from '$lib/ui-sfx';
  import { tick } from 'svelte';

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
  let virtualIndex = $state(1);
  let noTransition = $state(false);
  let scrollCooldown = false;
  let prevLength = 0;
  let initialPageSet = false;

  // Gamepad cursor: index within current page (-1 = inactive)
  let gpCursor = $state(-1);

  let totalPages = $derived(Math.max(1, Math.ceil(playlists.length / PER_PAGE)));

  function pagePlaylists(pageIdx: number): Playlist[] {
    return playlists.slice(pageIdx * PER_PAGE, (pageIdx + 1) * PER_PAGE);
  }

  $effect(() => {
    const len = playlists.length;
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

  // ── Gamepad API ──────────────────────────────────────────────────

  // Returns true if navigation hit a boundary and couldn't move
  export function gamepadNavigate(
    dir: "left" | "right" | "up" | "down",
  ): boolean {
    let pageItems = pagePlaylists(currentPage);
    if (pageItems.length === 0) return false;

    if (gpCursor < 0) {
      gpCursor = dir === "left" || dir === "up" ? pageItems.length - 1 : 0;
      onhover(pageItems[gpCursor]);
      return false;
    }

    const col = gpCursor % COLS;
    const row = Math.floor(gpCursor / COLS);

    switch (dir) {
      case "right": {
        if (col < COLS - 1 && gpCursor + 1 < pageItems.length) {
          gpCursor = gpCursor + 1;
        } else {
          nextPage();
          pageItems = pagePlaylists(currentPage);
          gpCursor = Math.min(row * COLS, pageItems.length - 1);
        }
        break;
      }
      case "left": {
        if (col > 0) {
          gpCursor = gpCursor - 1;
        } else {
          prevPage();
          pageItems = pagePlaylists(currentPage);
          const target = row * COLS + (COLS - 1);
          gpCursor = Math.min(target, pageItems.length - 1);
        }
        break;
      }
      case "down": {
        const next = (row + 1) * COLS + col;
        if (next < pageItems.length) {
          gpCursor = next;
        } else {
          return true;
        }
        break;
      }
      case "up": {
        if (row > 0) {
          gpCursor = (row - 1) * COLS + col;
        } else {
          return true;
        }
        break;
      }
    }

    const hovered = pagePlaylists(currentPage)[gpCursor];
    if (hovered) onhover(hovered);
    return false;
  }

  export function gamepadConfirm() {
    if (gpCursor < 0) return;
    const playlist = pagePlaylists(currentPage)[gpCursor];
    if (playlist) onselect(playlist);
  }

  export function gamepadClearCursor() {
    gpCursor = -1;
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
          {#each pagePlaylists(totalPages - 1) as playlist (playlist.id + '_lc')}
            <PlaylistCard
              {playlist}
              onclick={() => onselect(playlist)}
              onhover={(p) => { gpCursor = -1; onhover(p); }}
            />
          {/each}
        </div>
      </div>

      <!-- Real pages -->
      {#each Array(totalPages) as _, pageIdx}
        <div class="page">
          <div class="grid">
            {#each pagePlaylists(pageIdx) as playlist, i (playlist.id)}
              <PlaylistCard
                {playlist}
                focused={pageIdx === currentPage && i === gpCursor}
                onclick={() => onselect(playlist)}
                onhover={(p) => { gpCursor = -1; onhover(p); }}
              />
            {/each}
          </div>
        </div>
      {/each}

      <!-- Clone of first page -->
      <div class="page">
        <div class="grid">
          {#each pagePlaylists(0) as playlist (playlist.id + '_fc')}
            <PlaylistCard
              {playlist}
              onclick={() => onselect(playlist)}
              onhover={(p) => { gpCursor = -1; onhover(p); }}
            />
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
