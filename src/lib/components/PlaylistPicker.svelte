<script lang="ts">
  import type { Track } from '../types';
  import { playlists, addToPlaylist, createPlaylist } from '../stores/playlists';
  import PS2Btn from './PS2Btn.svelte';
  import { playUiSfx } from '$lib/ui-sfx';
  import { t } from '$lib/stores/i18n';

  let {
    track,
    onclose,
  }: {
    track: Track;
    onclose: () => void;
  } = $props();

  let creatingNew = $state(false);
  let newName = $state('');
  let addedToId = $state<string | null>(null);
  let nameInput = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (creatingNew) setTimeout(() => nameInput?.focus(), 10);
  });

  function handleAddTo(playlistId: string) {
    addToPlaylist(playlistId, track);
    playUiSfx('confirm');
    addedToId = playlistId;
    setTimeout(onclose, 600);
  }

  function handleCreateNew() {
    if (!newName.trim()) return;
    const id = createPlaylist(newName.trim());
    addToPlaylist(id, track);
    playUiSfx('confirm');
    addedToId = id;
    newName = '';
    setTimeout(onclose, 600);
  }

  function handleClose() {
    playUiSfx('back');
    onclose();
  }

  function handleOverlayMouseDown(e: MouseEvent) {
    if (e.target === e.currentTarget) handleClose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') handleCreateNew();
    if (e.key === 'Escape') {
      if (creatingNew) { creatingNew = false; newName = ''; }
      else handleClose();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onmousedown={handleOverlayMouseDown}>
  <div class="track-label">
    <span class="tl-title">{track.title}</span>
    <span class="tl-sep">·</span>
    <span class="tl-artist">{track.artist}</span>
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <nav class="menu" onkeydown={creatingNew ? handleKeydown : undefined}>
    {#if creatingNew}
      <input
        bind:this={nameInput}
        bind:value={newName}
        class="new-input"
        placeholder={$t('playlistNamePlaceholder')}
        maxlength="40"
      />
    {:else}
      {#each $playlists as pl (pl.id)}
        {@const alreadyIn = pl.tracks.some((t) => t.id === track.id)}
        <button
          class="menu-item"
          class:added={addedToId === pl.id}
          class:already={alreadyIn && addedToId === null}
          onclick={() => !alreadyIn && !addedToId && handleAddTo(pl.id)}
        >
          {#if addedToId === pl.id}
            ✓ {pl.name}
          {:else if alreadyIn}
            {pl.name} <span class="already-tag">{$t('alreadyAdded')}</span>
          {:else}
            {pl.name}
          {/if}
        </button>
      {/each}
      <button class="menu-item menu-item--new" onclick={() => (creatingNew = true)}>
        {$t('newPlaylist')}
      </button>
    {/if}
  </nav>

  <div class="close-hint">
    {#if creatingNew}
      <button class="hint-btn" onclick={() => { creatingNew = false; newName = ''; }}>
        <PS2Btn type="circle" />
        <span>{$t('back')}</span>
      </button>
      <button class="hint-btn hint-btn--create" onclick={handleCreateNew}>
        <PS2Btn type="cross" />
        <span>{$t('create')}</span>
      </button>
    {:else}
      <button class="hint-btn" onclick={handleClose}>
        <PS2Btn type="circle" />
        <span>{$t('back')}</span>
      </button>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.72);
    backdrop-filter: blur(3px);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 20px;
    z-index: 150;
    animation: fade-in 0.18s ease;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .track-label {
    display: flex;
    align-items: baseline;
    gap: 8px;
    animation: slide-in 0.25s cubic-bezier(0.34, 1.4, 0.64, 1);
  }

  .tl-title {
    font-size: 13px;
    color: var(--track-active);
    text-shadow: var(--text-shadow);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 260px;
  }

  .tl-sep { font-size: 11px; color: var(--text-dim); }

  .tl-artist {
    font-size: 11px;
    color: var(--text-dim);
    text-shadow: var(--text-shadow);
  }

  .menu {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    max-height: 360px;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0 4px;
    animation: slide-in 0.25s cubic-bezier(0.34, 1.4, 0.64, 1);
  }

  .menu::-webkit-scrollbar { width: 3px; }
  .menu::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.15); border-radius: 2px; }

  @keyframes slide-in {
    from { opacity: 0; transform: translateY(20px) scale(0.95); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }

  .menu-item {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 32px;
    font-weight: 800;
    font-family: inherit;
    color: var(--text-primary);
    text-shadow: var(--text-shadow);
    padding: 10px 32px;
    transition: color 0.12s;
    letter-spacing: 0.01em;
  }

  .menu-item:hover:not(:disabled) { color: var(--track-active); }

  .menu-item.added { color: var(--track-active); }

  .menu-item.already {
    opacity: 0.35;
    cursor: default;
  }

  .already-tag {
    font-size: 14px;
    color: var(--text-dim);
    font-weight: 400;
  }

  .menu-item--new {
    color: var(--text-secondary);
    font-size: 24px;
    opacity: 0.65;
    transition: color 0.12s, opacity 0.12s;
  }

  .menu-item--new:hover { color: var(--track-hover); opacity: 1; }

  .new-input {
    width: 280px;
    background: rgba(10, 10, 22, 0.55);
    border: 1px solid rgba(90, 95, 120, 0.35);
    border-radius: 4px;
    color: var(--track-active);
    font-family: inherit;
    font-size: 22px;
    font-weight: 800;
    text-shadow: var(--text-shadow);
    letter-spacing: 0.01em;
    padding: 3px 8px;
    outline: none;
    text-align: center;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .new-input:focus {
    border-color: rgba(90, 95, 180, 0.65);
    box-shadow: 0 0 10px rgba(80, 100, 200, 0.15);
  }

  .new-input::placeholder { color: var(--text-dim); font-size: 16px; font-weight: 400; }

  .close-hint {
    display: flex;
    align-items: center;
    gap: 24px;
    margin-top: 16px;
  }

  .hint-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 13px;
    color: var(--text-secondary);
    text-shadow: var(--text-shadow);
    padding: 0;
    transition: color 0.15s;
  }

  .hint-btn:hover { color: var(--track-hover); }
  .hint-btn--create:hover { color: var(--track-active); }
</style>
