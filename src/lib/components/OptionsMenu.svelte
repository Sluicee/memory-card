<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { scanFolder, refreshLibrary, clearLibrary } from '../stores/library';
  import PS2Btn from './PS2Btn.svelte';

  let { onclose, onStats }: { onclose: () => void; onStats: () => void } = $props();

  async function addFolder() {
    const path = await invoke<string | null>('pick_folder');
    if (path) {
      await scanFolder(path);
    }
    onclose();
  }

  async function refresh() {
    onclose();
    await refreshLibrary();
  }

  function clear() {
    clearLibrary();
    onclose();
  }

  function openStats() {
    onclose();
    onStats();
  }

  const items = [
    { label: 'Add new folder',  action: addFolder  },
    { label: 'Refresh library', action: refresh    },
    { label: 'Statistics',      action: openStats  },
    { label: 'Clear library',   action: clear      },
  ];
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="overlay"
  onmousedown={(e) => e.target === e.currentTarget && onclose()}
>
  <nav class="menu">
    {#each items as item}
      <button class="menu-item" onclick={item.action}>
        {item.label}
      </button>
    {/each}
  </nav>

  <div class="close-hint">
    <button class="hint-btn" onclick={onclose}>
      <PS2Btn type="circle" />
      <span>Close</span>
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
