<script lang="ts">
  import type { Album } from '../types';

  let { album, onclick }: { album: Album; onclick: () => void } = $props();
</script>

<button class="card" {onclick}>
  <div class="art-wrap">
    <div class="art">
      {#if album.cover_art}
        <img src={album.cover_art} alt={album.title} draggable="false" />
      {:else}
        <div class="art-placeholder">♪</div>
      {/if}
    </div>
    <!-- Right edge: y=0→140, no top-right gap -->
    <div class="edge-r"></div>
    <!-- Bottom edge: x=0→146, includes corner — no bottom-left gap -->
    <div class="edge-b"></div>
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
    gap: 8px;
    width: 140px;
    flex-shrink: 0;
  }

  .art-wrap {
    position: relative;
    width: 140px;
    height: 140px;
  }

  .art {
    position: absolute;
    inset: 0;
    background: rgba(90, 95, 120, 0.18);
    overflow: hidden;
    box-shadow: 2px 3px 6px rgba(0, 0, 0, 0.22);
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
    font-size: 28px;
    color: rgba(90, 95, 120, 0.3);
  }

  /* Right edge — x: 140→146, y: 0→140, no gap at top-right corner */
  .edge-r {
    position: absolute;
    top: 0;
    left: 140px;
    width: 6px;
    height: 140px;
    background: linear-gradient(to right,
      rgba(10, 10, 22, 0.55),
      rgba(10, 10, 22, 0.25)
    );
  }

  /* Bottom edge — x: 0→146, y: 140→145
     Width covers the corner so no gap at bottom-right either */
  .edge-b {
    position: absolute;
    top: 140px;
    left: 0;
    width: 146px;
    height: 5px;
    background: linear-gradient(to bottom,
      rgba(10, 10, 22, 0.50),
      rgba(10, 10, 22, 0.20)
    );
  }

  .title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-primary);
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    width: 140px;
  }
</style>
