<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { Album } from '../types';

  let {
    album,
    onclick,
    onhover,
    focused = false,
  }: {
    album: Album;
    onclick: () => void;
    onhover: (album: Album | null) => void;
    focused?: boolean;
  } = $props();

  const coverSrc = $derived(album.cover_art ? convertFileSrc(album.cover_art) : null);
</script>

<button
  class="card"
  class:gp-focused={focused}
  {onclick}
  onmouseenter={() => onhover(album)}
  onmouseleave={() => onhover(null)}
>
  <div class="art-wrap">
    <div class="art">
      {#if coverSrc}
        <img src={coverSrc} alt={album.title} draggable="false" />
      {:else}
        <div class="art-placeholder">
          <svg viewBox="0 0 16 16" width="32" height="32" fill="currentColor">
            <path d="M9 13c0 1.105-1.12 2-2.5 2S4 14.105 4 13s1.12-2 2.5-2 2.5.895 2.5 2zM9 3v7h1v-7H9z"/>
            <path d="M9 3v.5a.5.5 0 0 0 .5.5h4a.5.5 0 0 0 .5-.5V3a.5.5 0 0 0-.5-.5h-4A.5.5 0 0 0 9 3z"/>
          </svg>
        </div>
      {/if}
    </div>
    <div class="edge-r"></div>
    <div class="edge-b"></div>
  </div>
</button>

<style>
  .card {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 93px;
    flex-shrink: 0;
  }

  .art-wrap {
    position: relative;
    width: 93px;
    height: 93px;
  }

  .art {
    position: absolute;
    inset: 0;
    background: rgba(90, 95, 120, 0.18);
    overflow: hidden;
    box-shadow: 2px 3px 6px rgba(0, 0, 0, 0.22);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: transform 0.18s ease, box-shadow 0.18s ease, filter 0.18s ease;
  }

  .art::before {
    content: '';
    position: absolute;
    inset: 0;
    background:
      radial-gradient(circle at 28% 20%, rgba(255, 255, 255, 0.18), transparent 32%),
      linear-gradient(
        112deg,
        transparent 22%,
        rgba(255, 255, 255, 0.08) 36%,
        rgba(255, 255, 255, 0.42) 48%,
        rgba(255, 255, 255, 0.12) 58%,
        transparent 72%
      );
    transform: translateX(-135%);
    opacity: 0;
    transition: transform 0.38s ease, opacity 0.2s ease;
    pointer-events: none;
    mix-blend-mode: screen;
  }

  .card:hover .art,
  .card:focus-visible .art {
    box-shadow: 3px 5px 12px rgba(0, 0, 0, 0.28);
    filter: brightness(1.05) saturate(1.04);
  }

  .card:hover .art::before,
  .card:focus-visible .art::before,
  .card.gp-focused .art::before {
    transform: translateX(115%);
    opacity: 1;
  }

  .card.gp-focused .art {
    box-shadow:
      0 0 0 2px rgba(100, 140, 255, 0.85),
      3px 5px 14px rgba(0, 0, 0, 0.32);
    filter: brightness(1.1) saturate(1.08);
  }

  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .art-placeholder {
    font-size: 34px;
    color: rgba(90, 95, 120, 0.45);
    text-align: center;
  }

  .edge-r {
    position: absolute;
    top: 0;
    left: 93px;
    width: 4px;
    height: 93px;
    background: linear-gradient(to right,
      rgba(10, 10, 22, 0.55),
      rgba(10, 10, 22, 0.25)
    );
  }

  .edge-b {
    position: absolute;
    top: 93px;
    left: 0;
    width: 97px;
    height: 4px;
    background: linear-gradient(to bottom,
      rgba(10, 10, 22, 0.50),
      rgba(10, 10, 22, 0.20)
    );
  }
</style>
