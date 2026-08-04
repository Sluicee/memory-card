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

const MIN_INTERVALS: Record<UiSfxName, number> = {
  steps: 40,
  confirm: 40,
  back: 40,
  open: 80,
  nextPrev: 40,
  scan: 150,
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

// Managed AudioContext state
let audioCtx: AudioContext | null = null;
let idleTimer: ReturnType<typeof setTimeout> | null = null;
let ctxCreatedAt = 0;

const rawCache = new Map<UiSfxName, ArrayBuffer>();
const decodedCache = new Map<UiSfxName, AudioBuffer>();
const lastPlayedAt = new Map<UiSfxName, number>();

const IDLE_TIMEOUT_MS = 5000; // Close context after 5s of silence to flush driver buffers
const MAX_CTX_LIFETIME_MS = 30000; // Recycle context after 30s of active use to prevent drift

function closeAudioContext() {
  if (idleTimer) {
    clearTimeout(idleTimer);
    idleTimer = null;
  }
  if (audioCtx) {
    const ctxToClose = audioCtx;
    audioCtx = null;
    decodedCache.clear();
    try {
      void ctxToClose.close();
    } catch (_) {}
  }
}

function scheduleIdleClose() {
  if (idleTimer) clearTimeout(idleTimer);
  idleTimer = setTimeout(() => {
    closeAudioContext();
  }, IDLE_TIMEOUT_MS);
}

async function fetchRawSfx(name: UiSfxName): Promise<ArrayBuffer | null> {
  if (rawCache.has(name)) {
    return rawCache.get(name)!;
  }
  try {
    const res = await fetch(SOURCES[name]);
    const buffer = await res.arrayBuffer();
    rawCache.set(name, buffer);
    return buffer;
  } catch (e) {
    console.error(`Failed to fetch SFX ${name}:`, e);
    return null;
  }
}

export function primeUiSfx() {
  if (!browser) return;
  for (const name of Object.keys(SOURCES) as UiSfxName[]) {
    void fetchRawSfx(name);
  }
}

async function getOrInitAudioContext(): Promise<AudioContext | null> {
  if (!browser) return null;

  const now = Date.now();
  if (audioCtx && now - ctxCreatedAt > MAX_CTX_LIFETIME_MS) {
    closeAudioContext();
  }

  if (!audioCtx || audioCtx.state === 'closed') {
    const AudioCtx = window.AudioContext || (window as any).webkitAudioContext;
    if (!AudioCtx) return null;
    audioCtx = new AudioCtx({ latencyHint: 'interactive' });
    ctxCreatedAt = now;
    decodedCache.clear();
  }

  if (audioCtx.state === 'suspended') {
    try {
      await audioCtx.resume();
    } catch (_) {}
  }

  return audioCtx;
}

async function ensureDecodedBuffer(ctx: AudioContext, name: UiSfxName): Promise<AudioBuffer | null> {
  if (decodedCache.has(name)) {
    return decodedCache.get(name)!;
  }

  let raw = rawCache.get(name);
  if (!raw) {
    raw = await fetchRawSfx(name) || undefined;
  }
  if (!raw) return null;

  try {
    // ArrayBuffer is sliced because decodeAudioData detaches the buffer in some spec implementations
    const decoded = await ctx.decodeAudioData(raw.slice(0));
    decodedCache.set(name, decoded);
    return decoded;
  } catch (e) {
    console.error(`Failed to decode SFX ${name}:`, e);
    return null;
  }
}

function playUiSfxDirect(ctx: AudioContext, buffer: AudioBuffer, volume: number) {
  const source = ctx.createBufferSource();
  source.buffer = buffer;

  const gainNode = ctx.createGain();
  gainNode.gain.value = volume;

  source.connect(gainNode);
  gainNode.connect(ctx.destination);

  source.onended = () => {
    try {
      source.disconnect();
      gainNode.disconnect();
    } catch (_) {}
  };

  source.start(0);
}

export function playUiSfx(name: UiSfxName, volume = DEFAULT_VOLUMES[name]) {
  if (!get(sfxEnabled)) return;
  if (!browser) return;

  const now = Date.now();
  const lastTime = lastPlayedAt.get(name) || 0;
  const minInterval = MIN_INTERVALS[name] || 40;

  // Throttle rapid duplicate sound triggers to prevent sound buffer queue backlog
  if (now - lastTime < minInterval) {
    return;
  }
  lastPlayedAt.set(name, now);

  scheduleIdleClose();

  void (async () => {
    try {
      const ctx = await getOrInitAudioContext();
      if (!ctx) return;
      const buffer = await ensureDecodedBuffer(ctx, name);
      if (buffer && ctx.state !== 'closed') {
        playUiSfxDirect(ctx, buffer, volume);
      }
    } catch (e) {
      console.error('Play sfx error:', e);
    }
  })();
}

