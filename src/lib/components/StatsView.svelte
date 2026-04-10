<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import PS2Btn from "./PS2Btn.svelte";
  import { loadStats, clearStats, type StatsMap } from "../stores/stats";
  import type { Album, Track } from "../types";
  import { playUiSfx } from "$lib/ui-sfx";
  import { t } from "$lib/stores/i18n";

  let { albums, onclose }: { albums: Album[]; onclose: () => void } = $props();

  // ── Derived stats ─────────────────────────────────────────────────────────────

  interface TrackEntry {
    track: Track;
    album: Album;
    playCount: number;
    lastPlayed: number;
    totalListened: number;
  }

  interface AlbumEntry {
    album: Album;
    playCount: number;
    lastPlayed: number;
    totalListened: number;
  }

  interface ArtistEntry {
    artist: string;
    playCount: number;
    lastPlayed: number;
    totalListened: number;
    coverArt: string | null;
  }

  function buildStats() {
    const s: StatsMap = loadStats();
    const trackMap = new Map<string, TrackEntry>();
    const albumMap = new Map<string, AlbumEntry>();
    const artistMap = new Map<string, ArtistEntry>();

    for (const album of albums) {
      for (const track of album.tracks) {
        const ts = s[track.id];
        if (!ts || ts.playCount === 0) continue;

        trackMap.set(track.id, {
          track,
          album,
          playCount: ts.playCount,
          lastPlayed: ts.lastPlayed,
          totalListened: ts.totalListened,
        });

        const ae = albumMap.get(album.id) ?? {
          album,
          playCount: 0,
          lastPlayed: 0,
          totalListened: 0,
        };
        ae.playCount += ts.playCount;
        ae.lastPlayed = Math.max(ae.lastPlayed, ts.lastPlayed);
        ae.totalListened += ts.totalListened;
        albumMap.set(album.id, ae);

        const artistName = track.artist || album.artist || "Unknown Artist";
        const are = artistMap.get(artistName) ?? {
          artist: artistName,
          playCount: 0,
          lastPlayed: 0,
          totalListened: 0,
          coverArt: album.cover_art,
        };
        are.playCount += ts.playCount;
        are.lastPlayed = Math.max(are.lastPlayed, ts.lastPlayed);
        are.totalListened += ts.totalListened;
        if (!are.coverArt && album.cover_art) are.coverArt = album.cover_art;
        artistMap.set(artistName, are);
      }
    }

    const tracks = [...trackMap.values()]
      .sort((a, b) => b.playCount - a.playCount)
      .slice(0, 12);
    const albumsTop = [...albumMap.values()]
      .sort((a, b) => b.playCount - a.playCount)
      .slice(0, 8);
    const artistsTop = [...artistMap.values()]
      .sort((a, b) => b.playCount - a.playCount)
      .slice(0, 8);
    const recent = [...trackMap.values()]
      .sort((a, b) => b.lastPlayed - a.lastPlayed)
      .slice(0, 10);

    const totalPlays = Object.values(s).reduce(
      (acc, t) => acc + t.playCount,
      0,
    );
    const totalListened = Object.values(s).reduce(
      (acc, t) => acc + t.totalListened,
      0,
    );
    const uniqueTracks = Object.values(s).filter((t) => t.playCount > 0).length;
    const uniqueArtists = artistMap.size;

    return {
      tracks,
      albumsTop,
      artistsTop,
      recent,
      totalPlays,
      totalListened,
      uniqueTracks,
      uniqueArtists,
    };
  }

  let stats = $state(buildStats());
  let confirming = $state(false);

  function handleClear() {
    if (!confirming) {
      playUiSfx("confirm");
      confirming = true;
      setTimeout(() => (confirming = false), 3000);
      return;
    }

    playUiSfx("scan");
    clearStats();
    stats = buildStats();
    confirming = false;
  }

  function setActiveTab(tab: Tab) {
    if (activeTab === tab) return;
    playUiSfx("steps");
    activeTab = tab;
  }

  function handleClose() {
    playUiSfx("back");
    onclose();
  }

  function handleOverlayMouseDown(e: MouseEvent) {
    if (e.target === e.currentTarget) handleClose();
  }

  // ── Format helpers ────────────────────────────────────────────────────────────

  function fmtTime(secs: number): string {
    if (secs < 60) return `${secs}s`;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    if (h > 0) return `${h}h ${m}m`;
    const s = Math.floor(secs % 60);
    return `${m}m ${s}s`;
  }

  function fmtDate(ms: number): string {
    if (!ms) return "—";
    const diff = Date.now() - ms;
    const min = Math.floor(diff / 60000);
    if (min < 1) return $t('justNow');
    if (min < 60) return $t('minutesAgo', min);
    const h = Math.floor(min / 60);
    if (h < 24) return $t('hoursAgo', h);
    const d = Math.floor(h / 24);
    if (d < 7) return $t('daysAgo', d);
    return new Date(ms).toLocaleDateString();
  }

  // ── Tab state ─────────────────────────────────────────────────────────────────

  type Tab = "artists" | "albums" | "tracks" | "recent";
  let activeTab = $state<Tab>("artists");
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onmousedown={handleOverlayMouseDown}>
  <div class="panel">
    <!-- Summary row -->
    <div class="summary">
      <div class="stat-card">
        <span class="stat-val">{stats.totalPlays}</span>
        <span class="stat-lbl">{$t('totalPlays')}</span>
      </div>
      <div class="stat-card">
        <span class="stat-val">{fmtTime(stats.totalListened)}</span>
        <span class="stat-lbl">{$t('totalListened')}</span>
      </div>
      <div class="stat-card">
        <span class="stat-val">{stats.uniqueArtists}</span>
        <span class="stat-lbl">{$t('artistsLabel')}</span>
      </div>
      <div class="stat-card">
        <span class="stat-val">{stats.uniqueTracks}</span>
        <span class="stat-lbl">{$t('tracksLabel')}</span>
      </div>
    </div>

    <!-- Tabs -->
    <div class="tabs">
      <button
        class="tab"
        class:active={activeTab === "artists"}
        onclick={() => setActiveTab("artists")}>{$t('artistsTab')}</button
      >
      <button
        class="tab"
        class:active={activeTab === "albums"}
        onclick={() => setActiveTab("albums")}>{$t('albumsTab')}</button
      >
      <button
        class="tab"
        class:active={activeTab === "tracks"}
        onclick={() => setActiveTab("tracks")}>{$t('tracksTab')}</button
      >
      <button
        class="tab"
        class:active={activeTab === "recent"}
        onclick={() => setActiveTab("recent")}>{$t('recentTab')}</button
      >
    </div>

    <!-- Content -->
    <div class="list">
      {#if activeTab === "artists"}
        {#if stats.artistsTop.length === 0}
          <p class="empty">{$t('noPlaysYet')}</p>
        {:else}
          {#each stats.artistsTop as entry, i (entry.artist)}
            <div class="row">
              <span class="rank">#{i + 1}</span>
              {#if entry.coverArt}
                <img
                  class="thumb"
                  src={convertFileSrc(entry.coverArt)}
                  alt=""
                />
              {:else}
                <div class="thumb thumb--round thumb--empty">♪</div>
              {/if}
              <div class="row-info">
                <span class="row-title">{entry.artist}</span>
                <span class="row-sub">{$t('lastSeen')} {fmtDate(entry.lastPlayed)}</span>
              </div>
              <div class="row-right">
                <span class="play-count">{entry.playCount}</span>
                <span class="row-sub">{fmtTime(entry.totalListened)}</span>
              </div>
            </div>
          {/each}
        {/if}
      {:else if activeTab === "albums"}
        {#if stats.albumsTop.length === 0}
          <p class="empty">{$t('noPlaysYet')}</p>
        {:else}
          {#each stats.albumsTop as entry, i (entry.album.id)}
            <div class="row">
              <span class="rank">#{i + 1}</span>
              {#if entry.album.cover_art}
                <img
                  class="thumb"
                  src={convertFileSrc(entry.album.cover_art)}
                  alt=""
                />
              {:else}
                <div class="thumb thumb--empty">♪</div>
              {/if}
              <div class="row-info">
                <span class="row-title">{entry.album.title}</span>
                <span class="row-sub"
                  >{entry.album.artist} · {$t('lastSeen')} {fmtDate(entry.lastPlayed)}</span
                >
              </div>
              <div class="row-right">
                <span class="play-count">{entry.playCount}</span>
                <span class="row-sub">{fmtTime(entry.totalListened)}</span>
              </div>
            </div>
          {/each}
        {/if}
      {:else if activeTab === "tracks"}
        {#if stats.tracks.length === 0}
          <p class="empty">{$t('noPlaysYet')}</p>
        {:else}
          {#each stats.tracks as entry, i (entry.track.id)}
            <div class="row">
              <span class="rank">#{i + 1}</span>
              <div class="row-info">
                <span class="row-title">{entry.track.title}</span>
                <span class="row-sub"
                  >{entry.track.artist} · {entry.album.title}</span
                >
              </div>
              <div class="row-right">
                <span class="play-count">{entry.playCount}</span>
                <span class="row-sub">{fmtTime(entry.totalListened)}</span>
              </div>
            </div>
          {/each}
        {/if}
      {:else if stats.recent.length === 0}
        <p class="empty">{$t('noPlaysYet')}</p>
      {:else}
        {#each stats.recent as entry (entry.track.id)}
          <div class="row">
            {#if entry.album.cover_art}
              <img
                class="thumb"
                src={convertFileSrc(entry.album.cover_art)}
                alt=""
              />
            {:else}
              <div class="thumb thumb--empty">♪</div>
            {/if}
            <div class="row-info">
              <span class="row-title">{entry.track.title}</span>
              <span class="row-sub"
                >{entry.track.artist} · {entry.album.title}</span
              >
            </div>
            <div class="row-right">
              <span class="row-sub">{fmtDate(entry.lastPlayed)}</span>
              <span class="play-count-sm">×{entry.playCount}</span>
            </div>
          </div>
        {/each}
      {/if}
    </div>

    <!-- Footer hints -->
    <div class="footer">
      <button class="hint-btn" onclick={handleClose}>
        <PS2Btn type="circle" />
        <span>{$t('back')}</span>
      </button>
      <button class="clear-btn" class:confirming onclick={handleClear}>
        {confirming ? $t('areYouSure') : $t('clearStats')}
      </button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.78);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: fade-in 0.18s ease;
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .panel {
    width: 480px;
    max-height: 82%;
    display: flex;
    flex-direction: column;
    gap: 12px;
    animation: slide-in 0.25s cubic-bezier(0.34, 1.4, 0.64, 1);
  }

  @keyframes slide-in {
    from {
      opacity: 0;
      transform: translateY(16px) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  /* ── Summary ── */
  .summary {
    display: flex;
    gap: 8px;
  }

  .stat-card {
    flex: 1;
    background: var(--card-bg);
    border-radius: 8px;
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    box-shadow: var(--btn-shadow);
  }

  .stat-val {
    font-size: 20px;
    font-weight: 700;
    color: var(--track-active);
  }

  .stat-lbl {
    font-size: 9px;
    color: var(--text-dim);
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  /* ── Tabs ── */
  .tabs {
    display: flex;
    gap: 2px;
    background: rgba(10, 10, 22, 0.5);
    border-radius: 8px;
    padding: 3px;
  }

  .tab {
    flex: 1;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 11px;
    font-family: inherit;
    color: var(--text-secondary);
    padding: 5px 0;
    border-radius: 6px;
    letter-spacing: 0.04em;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .tab.active {
    background: var(--card-bg);
    color: var(--track-active);
    box-shadow: var(--btn-shadow);
  }

  /* ── List ── */
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }

  .list::-webkit-scrollbar {
    width: 3px;
  }
  .list::-webkit-scrollbar-thumb {
    background: var(--text-dim);
    border-radius: 2px;
  }

  .empty {
    text-align: center;
    font-size: 13px;
    color: var(--text-dim);
    padding: 32px 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.02);
    transition: background 0.1s;
  }

  .row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .rank {
    font-size: 10px;
    color: var(--text-dim);
    width: 24px;
    flex-shrink: 0;
    text-align: right;
  }

  .thumb {
    width: 28px;
    height: 28px;
    border-radius: 4px;
    object-fit: cover;
    flex-shrink: 0;
  }

  .thumb--round {
    border-radius: 50%;
  }

  .thumb--empty {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    color: var(--text-dim);
    background: rgba(90, 95, 120, 0.15);
  }

  .row-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .row-title {
    font-size: 12px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .row-sub {
    font-size: 9px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .row-right {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    flex-shrink: 0;
  }

  .play-count {
    font-size: 16px;
    font-weight: 700;
    color: var(--track-active);
    line-height: 1;
  }

  .play-count-sm {
    font-size: 11px;
    color: var(--track-active);
  }

  /* ── Footer ── */
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 4px;
  }

  .hint-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 12px;
    color: var(--text-secondary);
    padding: 0;
    transition: color 0.15s;
  }

  .hint-btn:hover {
    color: var(--text-primary);
  }

  .clear-btn {
    background: none;
    border: 1px solid rgba(90, 95, 120, 0.3);
    border-radius: 4px;
    cursor: pointer;
    font-size: 10px;
    font-family: inherit;
    color: var(--text-dim);
    padding: 4px 10px;
    transition:
      color 0.15s,
      border-color 0.15s;
    letter-spacing: 0.04em;
  }

  .clear-btn:hover {
    color: var(--text-secondary);
    border-color: rgba(90, 95, 120, 0.6);
  }

  .clear-btn.confirming {
    color: #ff4d4d;
    border-color: #ff4d4d;
    font-weight: 700;
  }
</style>
