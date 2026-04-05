<script lang="ts">
  import type { Album, Track } from '../types';
  import { currentTrack, isPlaying, playTrack, pause, resume } from '../stores/player';

  let {
    album,
    onclose,
  }: {
    album: Album;
    onclose: () => void;
  } = $props();

  function formatDuration(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  async function handleTrack(track: Track) {
    if ($currentTrack?.id === track.id) {
      if ($isPlaying) await pause();
      else await resume();
    } else {
      await playTrack(track, album);
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="view">

    <!-- Left: large cover with 3D effect -->
    <div class="cover-wrap">
      <div class="cover-art">
        {#if album.cover_art}
          <img src={album.cover_art} alt={album.title} draggable="false" />
        {:else}
          <div class="cover-placeholder">♪</div>
        {/if}
      </div>
      <div class="cover-edge-r"></div>
      <div class="cover-edge-b"></div>
    </div>

    <!-- Right: info + tracklist -->
    <div class="info">
      <div class="album-meta">
        <h2 class="album-title">{album.title}</h2>
        <p class="album-artist">{album.artist}</p>
        {#if album.year}
          <p class="album-year">{album.year}</p>
        {/if}
      </div>

      <ul class="tracklist">
        {#each album.tracks as track (track.id)}
          {@const active = $currentTrack?.id === track.id}
          <li class="track" class:active>
            <button class="track-btn" onclick={() => handleTrack(track)}>
              <span class="track-num">
                {#if active && $isPlaying}
                  <span class="playing-dot">▶</span>
                {:else}
                  {track.track_number || '—'}
                {/if}
              </span>
              <span class="track-title">{track.title}</span>
              <span class="track-dur">{formatDuration(track.duration)}</span>
            </button>
          </li>
        {/each}
      </ul>
    </div>

  </div>

  <!-- Bottom hints -->
  <div class="hints">
    <button class="back-btn" onclick={onclose}>
      <span class="btn-icon circle">○</span>
      <span>Back</span>
    </button>
    <span class="hint-item">
      <span class="btn-icon cross">✕</span>
      <span>Play</span>
    </span>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(180, 182, 192, 0.55);
    backdrop-filter: blur(6px);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 24px;
    z-index: 100;
    animation: overlay-in 0.25s ease;
  }

  @keyframes overlay-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .view {
    display: flex;
    gap: 40px;
    align-items: flex-start;
    animation: view-in 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @keyframes view-in {
    from {
      opacity: 0;
      transform: perspective(800px) rotateY(-25deg) scale(0.85);
    }
    to {
      opacity: 1;
      transform: perspective(800px) rotateY(0deg) scale(1);
    }
  }

  /* ── Cover ── */
  .cover-wrap {
    position: relative;
    width: 260px;
    height: 260px;
    flex-shrink: 0;
  }

  .cover-art {
    position: absolute;
    inset: 0;
    background: rgba(90, 95, 120, 0.18);
    overflow: hidden;
    box-shadow:
      4px 6px 10px rgba(0, 0, 0, 0.3),
      0 16px 40px rgba(0, 0, 0, 0.22);
  }

  .cover-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .cover-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 64px;
    color: rgba(90, 95, 120, 0.3);
  }

  .cover-edge-r {
    position: absolute;
    top: 0;
    left: 260px;
    width: 10px;
    height: 260px;
    background: linear-gradient(to right,
      rgba(10, 10, 22, 0.6),
      rgba(10, 10, 22, 0.25)
    );
  }

  .cover-edge-b {
    position: absolute;
    top: 260px;
    left: 0;
    width: 270px;
    height: 9px;
    background: linear-gradient(to bottom,
      rgba(10, 10, 22, 0.55),
      rgba(10, 10, 22, 0.2)
    );
  }

  /* ── Info ── */
  .info {
    display: flex;
    flex-direction: column;
    gap: 20px;
    width: 300px;
    max-height: 320px;
  }

  .album-meta {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .album-title {
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
    line-height: 1.2;
    margin: 0;
  }

  .album-artist {
    font-size: 13px;
    color: var(--text-secondary);
    margin: 0;
  }

  .album-year {
    font-size: 12px;
    color: var(--text-dim);
    margin: 0;
  }

  /* ── Tracklist ── */
  .tracklist {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0;
    overflow-y: auto;
    max-height: 240px;
    padding-right: 4px;
  }

  .tracklist::-webkit-scrollbar { width: 3px; }
  .tracklist::-webkit-scrollbar-thumb { background: var(--text-dim); }

  .track { display: flex; }

  .track-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    background: none;
    border: none;
    cursor: pointer;
    padding: 6px 8px;
    text-align: left;
    transition: background 0.1s;
  }

  .track-btn:hover {
    background: rgba(255,255,255,0.3);
  }

  .track.active .track-btn {
    background: rgba(255,255,255,0.45);
  }

  .track-num {
    font-size: 11px;
    color: var(--text-dim);
    width: 18px;
    flex-shrink: 0;
    text-align: right;
  }

  .playing-dot {
    color: var(--text-secondary);
    font-size: 9px;
  }

  .track-title {
    flex: 1;
    font-size: 13px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .track.active .track-title {
    font-weight: 600;
  }

  .track-dur {
    font-size: 11px;
    color: var(--text-dim);
    flex-shrink: 0;
  }

  /* ── Bottom hints ── */
  .hints {
    display: flex;
    align-items: center;
    gap: 24px;
    animation: view-in 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 12px;
    color: var(--text-secondary);
    padding: 0;
  }

  .back-btn:hover { color: var(--text-primary); }

  .hint-item {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .btn-icon {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 600;
    box-shadow: 0 1px 3px rgba(0,0,0,0.2);
  }

  .cross  { background: #4a90d9; color: #fff; }
  .circle { background: #d94a4a; color: #fff; }
</style>
