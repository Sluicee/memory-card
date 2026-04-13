<script lang="ts">
  import type { Album } from "../types";
  import AlbumCard from "./AlbumCard.svelte";
  import { playUiSfx } from "$lib/ui-sfx";
  import { tick } from "svelte";

  const COLS = 4;
  const ROWS = 3;
  const PER_PAGE = COLS * ROWS;

  let {
    albums,
    onselect,
    onhover,
    initialPage = 0,
  }: {
    albums: Album[];
    onselect: (album: Album) => void;
    onhover: (album: Album | null) => void;
    initialPage?: number;
  } = $props();

  // virtualIndex layout: [last-clone=0] [page0=1] [page1=2] ... [pageN=N] [first-clone=N+1]
  let currentPage = $state(0); // logical page (0..totalPages-1)
  let virtualIndex = $state(1); // DOM index in the slider
  let noTransition = $state(false);
  let scrollCooldown = false;
  let prevLength = 0;
  let initialPageSet = false;

  // Gamepad cursor: index within current page (-1 = inactive)
  let gpCursor = $state(-1);

  let totalPages = $derived(Math.max(1, Math.ceil(albums.length / PER_PAGE)));

  // Page slice helpers
  function pageAlbums(pageIdx: number): Album[] {
    return albums.slice(pageIdx * PER_PAGE, (pageIdx + 1) * PER_PAGE);
  }

  $effect(() => {
    const len = albums.length;
    const tp = totalPages;

    if (len === 0) {
      currentPage = 0;
      virtualIndex = 1;
      initialPageSet = false;
    } else if (len < prevLength) {
      // Albums reduced (filter or clear) — clamp to valid page
      const clamped = Math.min(currentPage, tp - 1);
      currentPage = clamped;
      virtualIndex = clamped + 1;
    } else if (!initialPageSet) {
      // First non-zero batch — apply initial page
      const page = Math.min(initialPage, tp - 1);
      currentPage = page;
      virtualIndex = page + 1;
      initialPageSet = true;
    }
    prevLength = len;
  });

  async function snapTo(newVirtual: number, newPage: number) {
    noTransition = true;
    virtualIndex = newVirtual;
    currentPage = newPage;
    await tick();
    // Brief delay so browser applies no-transition before we re-enable
    setTimeout(() => {
      noTransition = false;
    }, 20);
  }

  function nextPage() {
    playUiSfx("nextPrev");
    const next = virtualIndex + 1;
    virtualIndex = next;
    if (next > totalPages) {
      // Landed on first-page clone — animate there, then snap to real first page
      currentPage = 0;
      setTimeout(() => snapTo(1, 0), 370);
    } else {
      currentPage = next - 1;
    }
  }

  function prevPage() {
    playUiSfx("nextPrev");
    const prev = virtualIndex - 1;
    virtualIndex = prev;
    if (prev < 1) {
      // Landed on last-page clone — animate there, then snap to real last page
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
    let pageItems = pageAlbums(currentPage);
    if (pageItems.length === 0) return false;

    if (gpCursor < 0) {
      // First gamepad interaction — show cursor at entry position
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
          // Wrap to next page, same row
          nextPage();
          pageItems = pageAlbums(currentPage);
          gpCursor = Math.min(row * COLS, pageItems.length - 1);
        }
        break;
      }
      case "left": {
        if (col > 0) {
          gpCursor = gpCursor - 1;
        } else {
          // Wrap to prev page, same row, rightmost col
          prevPage();
          pageItems = pageAlbums(currentPage);
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
          // At bottom boundary — signal caller
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

    const hovered = pageAlbums(currentPage)[gpCursor];
    if (hovered) onhover(hovered);
    return false;
  }

  export function gamepadConfirm() {
    if (gpCursor < 0) return;
    const album = pageAlbums(currentPage)[gpCursor];
    if (album) onselect(album);
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
      <!-- Clone of last page (enables wrapping leftward) -->
      <div class="page">
        <div class="grid">
          {#each pageAlbums(totalPages - 1) as album (album.id + "_lc")}
            <AlbumCard
              {album}
              onclick={() => onselect(album)}
              onhover={(a) => {
                gpCursor = -1;
                onhover(a);
              }}
            />
          {/each}
        </div>
      </div>

      <!-- Real pages -->
      {#each Array(totalPages) as _, pageIdx}
        <div class="page">
          <div class="grid">
            {#each pageAlbums(pageIdx) as album, i (album.id)}
              <AlbumCard
                {album}
                focused={pageIdx === currentPage && i === gpCursor}
                onclick={() => onselect(album)}
                onhover={(a) => {
                  gpCursor = -1;
                  onhover(a);
                }}
              />
            {/each}
          </div>
        </div>
      {/each}

      <!-- Clone of first page (enables wrapping rightward) -->
      <div class="page">
        <div class="grid">
          {#each pageAlbums(0) as album (album.id + "_fc")}
            <AlbumCard
              {album}
              onclick={() => onselect(album)}
              onhover={(a) => {
                gpCursor = -1;
                onhover(a);
              }}
            />
          {/each}
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .wrapper {
    width: 100%;
    height: 100%;
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
    gap: 22px 30px;
  }
</style>
