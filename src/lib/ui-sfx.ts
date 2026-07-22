import { browser } from '$app/environment';
import { writable, get } from 'svelte/store';

const SOURCES = {
  confirm: '/sfx/confirm.wav',
  back: '/sfx/back.wav',
  open: '/sfx/open.wav',
  nextPrev: '/sfx/next_prev.wav',
  steps: '/sfx/steps.wav',
  scan: '/sfx/scan.wav',
} as const;

const DEFAULT_VOLUMES: Record<UiSfxName, number> = {
  confirm: 0.225,
  back: 0.1,
  open: 0.11,
  nextPrev: 0.10,
  steps: 0.14,
  scan: 0.01,
};

export type UiSfxName = keyof typeof SOURCES;

// Persistence logic
const STORAGE_KEY = 'mc_sfx_enabled';

function getInitialState() {
  if (browser) {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored !== null) {
      return stored === 'true';
    }
  }
  return true;
}

export const sfxEnabled = writable(getInitialState());

if (browser) {
  sfxEnabled.subscribe(value => {
    localStorage.setItem(STORAGE_KEY, value.toString());
  });
}

let audioCtx: AudioContext | null = null;
const bufferCache = new Map<UiSfxName, Promise<AudioBuffer | null>>();

function getAudioContext(): AudioContext | null {
  if (!browser) return null;
  if (!audioCtx) {
    audioCtx = new (window.AudioContext || (window as any).webkitAudioContext)();
  }
  return audioCtx;
}

function loadSfxBuffer(name: UiSfxName): Promise<AudioBuffer | null> {
  const cached = bufferCache.get(name);
  if (cached) return cached;

  const promise = (async () => {
    const ctx = getAudioContext();
    if (!ctx) return null;

    try {
      const res = await fetch(SOURCES[name]);
      const arrayBuffer = await res.arrayBuffer();
      return await ctx.decodeAudioData(arrayBuffer);
    } catch (e) {
      console.error(`Failed to load/decode SFX ${name}:`, e);
      bufferCache.delete(name);
      return null;
    }
  })();

  bufferCache.set(name, promise);
  return promise;
}

export function primeUiSfx() {
  if (!browser) return;
  for (const name of Object.keys(SOURCES) as UiSfxName[]) {
    void loadSfxBuffer(name);
  }
}

export async function playUiSfx(name: UiSfxName, volume = DEFAULT_VOLUMES[name]) {
  if (!get(sfxEnabled)) return;
  if (!browser) return;

  try {
    const ctx = getAudioContext();
    if (!ctx) return;

    if (ctx.state === 'suspended') {
      await ctx.resume();
    }

    const buffer = await loadSfxBuffer(name);
    if (!buffer) return;

    const source = ctx.createBufferSource();
    source.buffer = buffer;

    const gainNode = ctx.createGain();
    gainNode.gain.value = volume;

    source.connect(gainNode);
    gainNode.connect(ctx.destination);
    source.start(0);
  } catch (e) {
    console.error('Play sfx error:', e);
  }
}
