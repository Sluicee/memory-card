<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { Clip } from '../types';

  let {
    clip,
    onclick,
    onhover,
    focused = false,
  }: {
    clip: Clip;
    onclick: () => void;
    onhover: (clip: Clip | null) => void;
    focused?: boolean;
  } = $props();

  const thumbSrc = $derived(clip.thumbnail ? convertFileSrc(clip.thumbnail) : null);
</script>

<button
  class="card"
  class:gp-focused={focused}
  {onclick}
  onmouseenter={() => onhover(clip)}
  onmouseleave={() => onhover(null)}
>
  <div class="art-wrap">
    <div class="art">
      {#if thumbSrc}
        <img src={thumbSrc} alt={clip.title} draggable="false" />
      {:else}
        <div class="art-placeholder">
          <svg viewBox="0 0 16 16" width="30" height="30" fill="currentColor">
            <path d="M0 4a2 2 0 0 1 2-2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2V4zm10.5 9-3-3v6l3-3z" transform="translate(0,0)"/>
            <path d="M2.5 9.5A1.5 1.5 0 0 1 1 8h9v1.5A1.5 1.5 0 0 1 8.5 11h-6A1.5 1.5 0 0 1 1 9.5"/>
          </svg>
        </div>
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
    align-items: center;
    justify-content: center;
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
  }

  .card:hover .art,
  .card:focus-visible .art {
    box-shadow: 3px 5px 12px rgba(0, 0, 0, 0.28);
    filter: brightness(1.05) saturate(1.04);
  }

  .card:hover .art::before,
  .card:focus-visible .art::before,
  .card.gp-focused .art::before {
    transform: translateX(115%);
    opacity: 1;
  }

  .card.gp-focused .art {
    box-shadow:
      0 0 0 2px rgba(100, 140, 255, 0.85),
      3px 5px 14px rgba(0, 0, 0, 0.32);
    filter: brightness(1.1) saturate(1.08);
  }

  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .art-placeholder {
    color: rgba(90, 95, 120, 0.45);
    text-align: center;
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
</style>
