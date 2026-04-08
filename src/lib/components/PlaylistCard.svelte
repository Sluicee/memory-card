<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { Playlist } from '../stores/playlists';
  import { albums } from '../stores/library';

  let {
    playlist,
    onclick,
    onhover,
  }: {
    playlist: Playlist;
    onclick: () => void;
    onhover: (playlist: Playlist | null) => void;
  } = $props();

  const coverSrc = $derived((() => {
    for (const track of playlist.tracks) {
      const album = $albums.find(
        (a) => a.title === track.album && (a.artist === track.album_artist || a.artist === track.artist)
      );
      if (album?.cover_art) return convertFileSrc(album.cover_art);
    }
    return null;
  })());

  const isFavourites = $derived(playlist.id === 'favourites');
</script>

<button
  class="card"
  {onclick}
  onmouseenter={() => onhover(playlist)}
  onmouseleave={() => onhover(null)}
>
  <div class="art-wrap">
    <div class="art">
      {#if isFavourites}
        <div class="art-favourites">
          <svg viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" width="38" height="38">
            <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/>
          </svg>
        </div>
      {:else if coverSrc}
        <img src={coverSrc} alt={playlist.name} draggable="false" />
      {:else}
        <div class="art-placeholder">♪</div>
      {/if}
      <div class="art-overlay">
        <span class="pl-count">{playlist.tracks.length}</span>
      </div>
    </div>
    <div class="edge-r"></div>
    <div class="edge-b"></div>
  </div>
  <div class="pl-name">{playlist.name}</div>
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

  .art-favourites {
    width: 100%;
    height: 100%;
    background: var(--track-hover);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .art-overlay {
    position: absolute;
    bottom: 0;
    right: 0;
    background: rgba(0, 0, 0, 0.55);
    padding: 1px 5px;
    z-index: 1;
  }

  .pl-count {
    font-size: 9px;
    color: var(--text-dim);
    letter-spacing: 0.04em;
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

  .pl-name {
    font-size: 10px;
    color: var(--text-secondary);
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 93px;
    letter-spacing: 0.02em;
  }
</style>
