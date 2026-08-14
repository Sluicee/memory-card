<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { scanFolder, refreshLibrary, clearLibrary } from '../stores/library';
  import { scanClipFolder, refreshClipLibrary, clearClipLibrary } from '../stores/clips';
  import { updateInfo, updateProgress, installUpdate, clearUpdateError } from '../stores/updates';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
  import { getVersion } from '@tauri-apps/api/app';
  import PS2Btn from './PS2Btn.svelte';
  import { playUiSfx, sfxEnabled } from '$lib/ui-sfx';
  import { t, toggleLocale } from '$lib/stores/i18n';
  import { onMount } from 'svelte';

  const DISCORD_RPC_KEY = 'mc_discord_rpc_enabled';

  function loadDiscordRpcEnabled(): boolean {
    try {
      const stored = localStorage.getItem(DISCORD_RPC_KEY);
      if (stored !== null) return stored === 'true';
    } catch {}
    return true;
  }

  let {
    onclose,
    onStats,
    activeTab,
  }: {
    onclose: () => void;
    onStats: () => void;
    activeTab: "library" | "artists" | "playlists" | "queue" | "clips";
  } = $props();
  let autostartEnabled = $state(false);
  let discordRpcEnabled = $state(loadDiscordRpcEnabled());
  let appVersion = $state('');

  onMount(async () => {
    // Прошлая неудачная попытка не должна встречать пользователя надписью
    // «Ошибка обновления» через полчаса после самой ошибки.
    clearUpdateError();

    try {
      appVersion = await getVersion();
    } catch (e) {
      console.error('Failed to get app version:', e);
    }
    
    try {
      autostartEnabled = await isEnabled();
    } catch (e) {
      console.error('Failed to check autostart status:', e);
    }
    // Sync Discord RPC state to backend on open
    try {
      await invoke('set_discord_rpc_enabled', { enabled: discordRpcEnabled });
    } catch (e) {
      console.error('Failed to sync Discord RPC state:', e);
    }
  });

  async function toggleDiscordRpc() {
    playUiSfx('confirm');
    discordRpcEnabled = !discordRpcEnabled;
    try {
      localStorage.setItem(DISCORD_RPC_KEY, discordRpcEnabled.toString());
      await invoke('set_discord_rpc_enabled', { enabled: discordRpcEnabled });
    } catch (e) {
      console.error('Failed to toggle Discord RPC:', e);
    }
  }

  async function toggleAutostart() {
    playUiSfx('confirm');
    try {
      if (autostartEnabled) {
        await disable();
        autostartEnabled = false;
      } else {
        await enable();
        autostartEnabled = true;
      }
    } catch (e) {
      console.error('Failed to toggle autostart:', e);
    }
  }

  function toggleSfx() {
    sfxEnabled.update(n => !n);
    playUiSfx('confirm');
  }

  async function addFolder() {
    const path = await invoke<string | null>('pick_folder');
    if (path) {
      playUiSfx('scan');
      await scanFolder(path);
    }
    handleClose(false);
  }

  async function refresh() {
    playUiSfx('scan');
    handleClose(false);
    await refreshLibrary();
  }

  function clear() {
    playUiSfx('confirm');
    clearLibrary();
    handleClose(false);
  }

  async function addClipFolder() {
    const path = await invoke<string | null>('pick_folder');
    if (path) {
      playUiSfx('scan');
      await scanClipFolder(path);
    }
    handleClose(false);
  }

  async function refreshClips() {
    playUiSfx('scan');
    handleClose(false);
    await refreshClipLibrary();
  }

  function clearClips() {
    playUiSfx('confirm');
    clearClipLibrary();
    handleClose(false);
  }

  function openStats() {
    playUiSfx('open');
    onclose();
    onStats();
  }

  function handleClose(playSound = true) {
    if (playSound) playUiSfx('back');
    onclose();
  }

  function handleOverlayMouseDown(e: MouseEvent) {
    if (e.target === e.currentTarget) handleClose();
  }

  // Пока апдейт качается, кнопка занята — стадия живёт в сторе, поэтому
  // прогресс не теряется, если закрыть и снова открыть меню.
  const updateBusy = $derived(
    $updateProgress.stage !== 'idle' && $updateProgress.stage !== 'error'
  );

  async function getUpdate() {
    if (!$updateInfo || updateBusy) return;
    playUiSfx('confirm');

    if (!$updateInfo.can_auto_update) {
      openUrl($updateInfo.url);
      handleClose(false);
      return;
    }

    // Ушли в браузер (нет latest.json для этого релиза) — меню закрываем,
    // как и в старом поведении. При ошибке остаёмся, чтобы показать её.
    if (await installUpdate() === 'fallback') handleClose(false);
  }

  const updateLabel = $derived.by(() => {
    if (!$updateInfo) return '';
    switch ($updateProgress.stage) {
      case 'preparing':  return $t('updatePreparing');
      case 'downloading':
        return $updateProgress.ratio === null
          ? $t('updatePreparing')
          : $t('updateDownloading', Math.round($updateProgress.ratio * 100));
      case 'installing': return $t('updateInstalling');
      case 'restarting': return $t('updateRestarting');
      case 'error':      return $t('updateFailed');
      default:           return $t('getUpdate', $updateInfo.version);
    }
  });

  // updateItem отдельно, чтобы не путать с колонками
  const updateItem = $derived(
    $updateInfo ? { label: updateLabel, action: getUpdate } : null
  );

  import { isEqualizerOpen } from '$lib/stores/equalizer';

  type MenuItem = { label: string; action: () => void; active?: boolean };

  const leftItems: MenuItem[] = $derived(
    activeTab === 'clips'
      ? [
          { label: $t('addClipFolder'), action: addClipFolder },
          { label: $t('refreshClips'),  action: refreshClips  },
          { label: $t('clearClips'),    action: clearClips    },
        ]
      : [
          { label: $t('addFolder'),     action: addFolder  },
          { label: $t('refreshLibrary'), action: refresh   },
          { label: $t('equalizer'),      action: () => { playUiSfx('open'); isEqualizerOpen.set(true); handleClose(false); } },
          { label: $t('statistics'),    action: openStats  },
          { label: $t('clearLibrary'),  action: clear      },
        ],
  );

  import { sortMode, cycleSortMode } from '$lib/stores/sortMode';

  // Toggle items show only the key (no ": ON"/": OFF" suffix) and are
  // highlighted via `active` when on, left unstyled when off — same idea
  // as a lit vs. unlit button rather than a "key: value" caption.
  const rightItems: MenuItem[] = $derived([
    { label: $t('sfx'),        active: $sfxEnabled,       action: toggleSfx       },
    { label: $t('autostart'),  active: autostartEnabled,  action: toggleAutostart },
    { label: $t('discordRpc'), active: discordRpcEnabled, action: toggleDiscordRpc },
    {
      label:
        $sortMode === 'artist'
          ? $t('sortByArtist')
          : $sortMode === 'title'
            ? $t('sortByTitle')
            : $t('sortByYear'),
      action: () => {
        playUiSfx('confirm');
        cycleSortMode();
      },
    },
    { label: $t('switchLanguage'), action: () => { playUiSfx('confirm'); toggleLocale(); } },
  ]);

  // Gamepad cursor: -1 = none, 0 = updateItem, 1..L = left col, L+1..L+R = right col
  let gpIdx = $state(-1);

  const LEFT_COUNT = $derived(leftItems.length);

  function getUpdateOffset() { return updateItem ? 1 : 0; }

  export function gamepadNavigate(dir: 'up' | 'down' | 'left' | 'right') {
    const U = getUpdateOffset();
    const total = U + leftItems.length + rightItems.length;
    if (total === 0) return;

    if (gpIdx < 0) {
      gpIdx = dir === 'up' ? total - 1 : U;
      return;
    }

    // На update-кнопке
    if (gpIdx < U) {
      if (dir === 'down') gpIdx = U;
      return;
    }

    const inRight = gpIdx >= U + LEFT_COUNT;
    const colStart = inRight ? U + LEFT_COUNT : U;
    const colLen   = inRight ? rightItems.length : leftItems.length;
    const row      = gpIdx - colStart;

    if (dir === 'up') {
      if (row === 0) { if (updateItem) gpIdx = 0; return; }
      gpIdx = colStart + row - 1;
    } else if (dir === 'down') {
      if (row >= colLen - 1) return;
      gpIdx = colStart + row + 1;
    } else if (dir === 'left') {
      if (!inRight) return;
      gpIdx = U + Math.min(row, leftItems.length - 1);
    } else if (dir === 'right') {
      if (inRight) return;
      gpIdx = U + LEFT_COUNT + Math.min(row, rightItems.length - 1);
    }
  }

  export function gamepadConfirm() {
    if (gpIdx < 0) return;
    const U = getUpdateOffset();
    if (gpIdx < U) { updateItem?.action(); return; }
    if (gpIdx < U + LEFT_COUNT) { leftItems[gpIdx - U]?.action(); return; }
    rightItems[gpIdx - U - LEFT_COUNT]?.action();
  }

  export function gamepadClearCursor() {
    gpIdx = -1;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="overlay"
  onmousedown={handleOverlayMouseDown}
>
  <nav class="menu">
    {#if updateItem}
      <div class="update-slot">
        <button
          class="menu-item full-width highlight"
          class:gp-focused={gpIdx === 0}
          class:busy={updateBusy}
          class:failed={$updateProgress.stage === 'error'}
          disabled={updateBusy}
          onclick={updateItem.action}
        >{updateItem.label}</button>
        {#if updateBusy}
          <div class="update-bar" class:indeterminate={$updateProgress.ratio === null}>
            <div
              class="update-bar-fill"
              style:width={$updateProgress.ratio !== null
                ? `${$updateProgress.ratio * 100}%`
                : undefined}
            ></div>
          </div>
        {/if}
      </div>
    {/if}

    <div class="columns">
      <div class="col">
        {#each leftItems as item, i}
          {@const flatIdx = getUpdateOffset() + i}
          <button
            class="menu-item"
            class:gp-focused={flatIdx === gpIdx}
            onclick={item.action}
          >{item.label}</button>
        {/each}
      </div>

      <div class="col">
        {#each rightItems as item, i}
          {@const flatIdx = getUpdateOffset() + LEFT_COUNT + i}
          <button
            class="menu-item"
            class:toggle-on={item.active}
            class:gp-focused={flatIdx === gpIdx}
            onclick={item.action}
          >{item.label}</button>
        {/each}
      </div>
    </div>
  </nav>

  <div class="close-hint">
    <button class="hint-btn" onclick={() => handleClose()}>
      <PS2Btn type="circle" />
      <span>{$t('close')}</span>
    </button>
  </div>

  {#if appVersion}
    <div class="app-version">v{appVersion}</div>
  {/if}
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
    gap: 24px;
    z-index: 100;
    animation: fade-in 0.18s ease;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .menu {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    animation: slide-in 0.25s cubic-bezier(0.34, 1.4, 0.64, 1);
  }

  @keyframes slide-in {
    from { opacity: 0; transform: translateY(20px) scale(0.95); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }

  .columns {
    display: flex;
    flex-direction: row;
    align-items: flex-start;
    gap: 0;
  }

  .col {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

.menu-item {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 28px;
    font-weight: 800;
    color: var(--text-primary);
    text-shadow: var(--text-shadow);
    padding: 8px 28px;
    transition: color 0.12s;
    letter-spacing: 0.01em;
    white-space: nowrap;
  }

  .menu-item.full-width {
    font-size: 28px;
    padding: 8px 28px;
  }

  .menu-item:hover,
  .menu-item.gp-focused {
    color: var(--track-active);
  }

  /* Toggle items (SFX, autostart, Discord RPC): the key itself lights up
     when the setting is on, instead of appending a ": ON"/": OFF" value. */
  .menu-item.toggle-on {
    color: var(--track-active);
    filter: drop-shadow(0 0 6px rgba(202, 167, 0, 0.55));
  }

  .menu-item.highlight {
    color: var(--track-active);
    animation: pulse 1.5s infinite ease-in-out;
  }

  /* Во время загрузки пульсация уступает место полосе прогресса */
  .menu-item.busy {
    animation: none;
    cursor: default;
  }

  .menu-item.failed {
    animation: none;
    color: var(--text-secondary);
  }

  .update-slot {
    display: flex;
    flex-direction: column;
    align-items: stretch;
  }

  .update-bar {
    height: 4px;
    margin: 0 28px 6px;
    background: rgba(255, 255, 255, 0.16);
    border-radius: 2px;
    overflow: hidden;
  }

  .update-bar-fill {
    height: 100%;
    width: 0;
    background: var(--track-active);
    box-shadow: 0 0 6px var(--track-active);
    border-radius: 2px;
    transition: width 0.12s linear;
  }

  /* Размер файла ещё неизвестен (сервер не прислал Content-Length) —
     показываем бегущий блик вместо конкретного процента. */
  .update-bar.indeterminate .update-bar-fill {
    width: 35%;
    transition: none;
    animation: sweep 1.1s ease-in-out infinite;
  }

  @keyframes sweep {
    0%   { transform: translateX(-100%); }
    100% { transform: translateX(286%); }
  }

  @keyframes pulse {
    0% { filter: brightness(1); }
    50% { filter: brightness(1.5) drop-shadow(0 0 5px var(--track-active)); }
    100% { filter: brightness(1); }
  }

  .close-hint {
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

  .app-version {
    position: absolute;
    bottom: 24px;
    right: 32px;
    font-size: 11px;
    color: var(--text-dim);
    letter-spacing: 0.05em;
  }
</style>
