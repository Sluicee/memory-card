<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { scanFolder, refreshLibrary, clearLibrary } from '../stores/library';
  import { updateInfo } from '../stores/updates';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
  import PS2Btn from './PS2Btn.svelte';
  import { playUiSfx, sfxEnabled } from '$lib/ui-sfx';
  import { t, toggleLocale } from '$lib/stores/i18n';
  import { onMount } from 'svelte';

  let { onclose, onStats }: { onclose: () => void; onStats: () => void } = $props();
  let autostartEnabled = $state(false);

  onMount(async () => {
    try {
      autostartEnabled = await isEnabled();
    } catch (e) {
      console.error('Failed to check autostart status:', e);
    }
  });

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

  function getUpdate() {
    if ($updateInfo) {
      playUiSfx('confirm');
      openUrl($updateInfo.url);
      handleClose(false);
    }
  }

  const items = $derived([
    ...($updateInfo ? [{ label: $t('getUpdate', $updateInfo.version), action: getUpdate, highlight: true }] : []),
    { label: $t('addFolder'),                        action: addFolder       },
    { label: $t('refreshLibrary'),                   action: refresh         },
    { label: $t('statistics'),                       action: openStats       },
    { label: $t('sfx', $sfxEnabled),                 action: toggleSfx       },
    { label: $t('autostart', autostartEnabled),      action: toggleAutostart },
    { label: $t('clearLibrary'),                     action: clear           },
    { label: $t('switchLanguage'),                   action: () => { playUiSfx('confirm'); toggleLocale(); } },
  ]);

</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="overlay"
  onmousedown={handleOverlayMouseDown}
>
  <nav class="menu">
    {#each items as item}
      <button class="menu-item" class:highlight={item.highlight} onclick={item.action}>
        {item.label}
      </button>
    {/each}
  </nav>

  <div class="close-hint">
    <button class="hint-btn" onclick={() => handleClose()}>
      <PS2Btn type="circle" />
      <span>{$t('close')}</span>
    </button>
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
    gap: 8px;
    animation: slide-in 0.25s cubic-bezier(0.34, 1.4, 0.64, 1);
  }

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
    color: var(--text-primary);
    text-shadow: var(--text-shadow);
    padding: 10px 32px;
    transition: color 0.12s;
    letter-spacing: 0.01em;
  }

  .menu-item:hover {
    color: var(--track-active);
  }

  .menu-item.highlight {
    color: var(--track-active);
    animation: pulse 1.5s infinite ease-in-out;
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
</style>
