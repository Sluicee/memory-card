<script lang="ts">
  import type { Album } from '../types';

  let {
    album,
    onclick,
    onhover,
  }: {
    album: Album;
    onclick: () => void;
    onhover: (album: Album | null) => void;
  } = $props();
</script>

<button
  class="card"
  {onclick}
  onmouseenter={() => onhover(album)}
  onmouseleave={() => onhover(null)}
>
  <div class="art-wrap">
    <div class="art">
      {#if album.cover_art}
        <img src={album.cover_art} alt={album.title} draggable="false" />
      {:else}
        <div class="art-placeholder">♪</div>
      {/if}
    </div>
    <div class="edge-r"></div>
    <div class="edge-b"></div>
  </div>
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
</style>
