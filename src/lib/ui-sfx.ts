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
  sfxEnabled.subscribe((value) => {
    localStorage.setItem(STORAGE_KEY, value.toString());
  });
}

let audioCtx: AudioContext | null = null;
const decodedCache = new Map<UiSfxName, AudioBuffer>();
const loadingPromises = new Map<UiSfxName, Promise<AudioBuffer | null>>();

function getAudioContext(): AudioContext | null {
  if (!browser) return null;
  if (!audioCtx) {
    const AudioCtx = window.AudioContext || (window as any).webkitAudioContext;
    if (AudioCtx) {
      // Request low-latency interactive audio context
      audioCtx = new AudioCtx({ latencyHint: 'interactive' });
    }
  }
  return audioCtx;
}

function loadSfxBuffer(name: UiSfxName): Promise<AudioBuffer | null> {
  if (decodedCache.has(name)) {
    return Promise.resolve(decodedCache.get(name)!);
  }
  const cachedPromise = loadingPromises.get(name);
  if (cachedPromise) return cachedPromise;

  const promise = (async () => {
    const ctx = getAudioContext();
    if (!ctx) return null;

    try {
      const res = await fetch(SOURCES[name]);
      const arrayBuffer = await res.arrayBuffer();
      const decoded = await ctx.decodeAudioData(arrayBuffer);
      decodedCache.set(name, decoded);
      return decoded;
    } catch (e) {
      console.error(`Failed to load/decode SFX ${name}:`, e);
      loadingPromises.delete(name);
      return null;
    }
  })();

  loadingPromises.set(name, promise);
  return promise;
}

export function primeUiSfx() {
  if (!browser) return;
  // Initialize context eagerly
  getAudioContext();
  for (const name of Object.keys(SOURCES) as UiSfxName[]) {
    void loadSfxBuffer(name);
  }
}

function playUiSfxDirect(ctx: AudioContext, buffer: AudioBuffer, volume: number) {
  const source = ctx.createBufferSource();
  source.buffer = buffer;

  const gainNode = ctx.createGain();
  gainNode.gain.value = volume;

  source.connect(gainNode);
  gainNode.connect(ctx.destination);
  source.start(0);
}

export function playUiSfx(name: UiSfxName, volume = DEFAULT_VOLUMES[name]) {
  if (!get(sfxEnabled)) return;
  if (!browser) return;

  try {
    const ctx = getAudioContext();
    if (!ctx) return;

    // Unsuspend audio context without awaiting (non-blocking)
    if (ctx.state === 'suspended') {
      void ctx.resume();
    }

    const buffer = decodedCache.get(name);
    if (buffer) {
      // Instant synchronous playback
      playUiSfxDirect(ctx, buffer, volume);
    } else {
      // Fallback if buffer hasn't finished preloading yet
      void loadSfxBuffer(name).then((buf) => {
        if (buf) playUiSfxDirect(ctx, buf, volume);
      });
    }
  } catch (e) {
    console.error('Play sfx error:', e);
  }
}
