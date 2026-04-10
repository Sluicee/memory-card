<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { Artist } from '../types';

  let {
    artist,
    onclick,
    onhover,
  }: {
    artist: Artist;
    onclick: () => void;
    onhover: (artist: Artist | null) => void;
  } = $props();

  const coverSrc = $derived((() => {
    for (const album of artist.albums) {
      if (album.cover_art) return convertFileSrc(album.cover_art);
    }
    return null;
  })());
</script>

<button
  class="card"
  {onclick}
  onmouseenter={() => onhover(artist)}
  onmouseleave={() => onhover(null)}
>
  <div class="art-wrap">
    <div class="art">
      {#if coverSrc}
        <img src={coverSrc} alt={artist.name} draggable="false" />
      {:else}
        <div class="art-placeholder">👩‍🎤</div>
      {/if}
      <div class="art-overlay">
        <span class="pl-count">{artist.albums.length}</span>
      </div>
    </div>
  </div>
  <div class="artist-name">{artist.name}</div>
</button>

<style>
  .card {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
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
    border-radius: 50%;
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
    border-radius: 50%;
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
    z-index: 2;
  }

  .card:hover .art,
  .card:focus-visible .art {
    box-shadow: 3px 5px 12px rgba(0, 0, 0, 0.28);
    filter: brightness(1.05) saturate(1.04);
  }

  .card:hover .art::before,
  .card:focus-visible .art::before {
    transform: translateX(115%);
    opacity: 1;
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

  .art-overlay {
    position: absolute;
    bottom: 2px;
    background: rgba(0, 0, 0, 0.55);
    padding: 1px 6px;
    border-radius: 10px;
    z-index: 1;
  }

  .pl-count {
    font-size: 9px;
    color: var(--text-dim);
    letter-spacing: 0.04em;
  }

  .artist-name {
    font-size: 10px;
    color: var(--text-secondary);
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 93px;
    letter-spacing: 0.02em;
    margin-top: 2px;
  }
</style>
