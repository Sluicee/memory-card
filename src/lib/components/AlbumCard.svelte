<script lang="ts">
  import type { Album } from '../types';

  let { album, onclick }: { album: Album; onclick: () => void } = $props();
</script>

<button class="card" {onclick}>
  <div class="art">
    {#if album.cover_art}
      <img src={album.cover_art} alt={album.title} draggable="false" />
    {:else}
      <div class="art-placeholder">♪</div>
    {/if}
  </div>
  <span class="title">{album.title}</span>
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
    gap: 6px;
    width: 100%;
  }

  .art {
    width: 100%;
    aspect-ratio: 1;
    overflow: hidden;
    background: rgba(90, 95, 120, 0.18);
  }

  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .art-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 24px;
    color: rgba(90, 95, 120, 0.3);
  }

  .title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-primary);
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    width: 100%;
  }
</style>
