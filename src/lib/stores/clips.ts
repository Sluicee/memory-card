import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Clip } from '../types';
import { stop as stopMusic } from './player';

const CLIP_FOLDERS_KEY = 'mp_clip_folders';

// Sanity clamp around whatever ClipPlayerView measures as the actually-
// available display area (see `measureTargetLongSide` there, driven by a
// ResizeObserver on .stage — covers windowed, fullscreen, and mini modes
// uniformly by measuring instead of guessing per view mode). Only matters
// for the *transcode* fallback path (see buildClipStreamUrl/
// clip_stream_server.rs) — a remux can't be scaled at all, it always
// sends the source's own resolution and lets the browser's own decode+
// display handle downscaling, same as any normal video player. Floor:
// never decode below what even the small windowed view already needs,
// regardless of a transient 0 mid-layout. Ceiling: never chase transcode
// CPU cost for a panel bigger than realtime VP9 encoding can actually
// sustain on modest hardware — measured live: encoding alone pegged 5+
// CPU cores at 1920x1440 on a 12-core desktop.
export const PLAYBACK_BOUND_MIN = 640;
export const PLAYBACK_BOUND_MAX = 1920;

// HTMLMediaElement.canPlayType() reflects whatever the actual playback
// engine can decode on this machine right now — WebKitGTK's GStreamer
// plugin set on Linux, WebView2/Media Foundation on Windows — correctly
// and portably, without this app needing separate per-OS/per-distro
// codec-availability detection. Backs clip_stream_server.rs's remux
// decision: a clip already in one of these codecs gets stream-copied
// (near-free) instead of live-transcoded to VP9 (several CPU cores,
// realtime, for the duration of playback) — see is_remux_safe there.
const VIDEO_CODEC_PROBES: Array<{ family: string; mimeCodec: string }> = [
  { family: 'vp9', mimeCodec: 'video/webm; codecs="vp9"' },
  { family: 'av1', mimeCodec: 'video/webm; codecs="av01.0.05M.08"' },
];

// canPlayType() lies about AV1 under WebKitGTK: the codec *is* decodable
// (dav1ddec is present, so the probe says "probably"), but the decoded
// frames go through glupload on their way to the compositor and come out as
// a flat green rectangle. Excluding av1 here costs a live VP9 transcode for
// AV1 clips on Linux, which is the same path every other non-VP9 codec
// already takes — and it actually renders.
const isLinux = /Linux|X11/.test(navigator.userAgent);

let cachedPlayableCodecs: string[] | null = null;

export function getPlayableVideoCodecs(): string[] {
  if (!cachedPlayableCodecs) {
    const probe = document.createElement('video');
    cachedPlayableCodecs = VIDEO_CODEC_PROBES
      .filter(({ family }) => !(isLinux && family === 'av1'))
      .filter(({ mimeCodec }) => probe.canPlayType(mimeCodec) !== '')
      .map(({ family }) => family);
  }
  return cachedPlayableCodecs;
}

/**
 * Aspect-fits a clip's source resolution into a `longSide`-capped box,
 * rounded to even pixels (ffmpeg-friendly). Never scales *up* past the
 * clip's native resolution — `Math.min(1, ...)` was missing before, so any
 * clip with a longer side under the bound (a plausible size for a lot of
 * real clip sources) got upscaled by ffmpeg's own `scale=` filter to
 * exactly fill it, decoding and displaying it visibly blockier than the
 * source ever was — most obvious full-window in fullscreen mode, where
 * canvasScale is closer to 1 and doesn't shrink that blockiness back down.
 */
export function computePlaybackBox(clip: Clip, longSide: number): { w: number; h: number } {
  const srcW = clip.width || longSide;
  const srcH = clip.height || Math.round((longSide * 9) / 16);
  const scale = Math.min(1, longSide / Math.max(srcW, srcH));
  const w = Math.max(2, Math.round((srcW * scale) / 2) * 2);
  const h = Math.max(2, Math.round((srcH * scale) / 2) * 2);
  return { w, h };
}

function loadClipFolders(): string[] {
  try { return JSON.parse(localStorage.getItem(CLIP_FOLDERS_KEY) ?? '[]'); } catch { return []; }
}
function saveClipFolders(paths: string[]) {
  localStorage.setItem(CLIP_FOLDERS_KEY, JSON.stringify(paths));
}

export const clipFolderPaths = writable<string[]>(loadClipFolders());
export const clips           = writable<Clip[]>([]);
export const isClipScanning  = writable(false);
export const clipScanStatus  = writable({ filesScanned: 0, clipsFound: 0, totalFiles: 0 });
export const selectedClip    = writable<Clip | null>(null);

// ── Cache persistence ─────────────────────────────────────────────────────────

async function saveCache() {
  const data = JSON.stringify(get(clips));
  try { await invoke('save_clip_cache', { data }); } catch (e) { console.error(e); }
}

export async function loadClipCache(): Promise<boolean> {
  return new Promise(async (resolve) => {
    const unlisten: Array<() => void> = [];

    unlisten.push(await listen<Clip[]>('clip:scan:clips', (e) => {
      clips.update((c) => [...c, ...e.payload]);
    }));

    unlisten.push(await listen<void>('clip:scan:done', () => {
      unlisten.forEach(u => u());
      resolve(true);
    }));

    try {
      const found = await invoke<boolean>('load_clip_cache');
      if (!found) {
        unlisten.forEach(u => u());
        resolve(false);
      }
    } catch (e) {
      console.error('Clip cache load failed:', e);
      unlisten.forEach(u => u());
      resolve(false);
    }
  });
}

// ── Scan helpers ──────────────────────────────────────────────────────────────

async function scanOne(path: string): Promise<void> {
  const unlisten: Array<() => void> = [];
  let clipsFound = 0;

  await new Promise<void>(async (resolve) => {
    unlisten.push(await listen<{ total: number }>('clip:scan:start', (e) => {
      clipScanStatus.update(s => ({ ...s, totalFiles: e.payload.total, filesScanned: 0, clipsFound: 0 }));
      clipsFound = 0;
    }));
    unlisten.push(await listen<{ files_scanned: number }>(
      'clip:scan:progress',
      (e) => clipScanStatus.update(s => ({ ...s, filesScanned: e.payload.files_scanned }))
    ));
    unlisten.push(await listen<Clip[]>('clip:scan:clips', (e) => {
      clipsFound += e.payload.length;
      clipScanStatus.update(s => ({ ...s, clipsFound }));
      // Upsert by id so overlapping/re-scanned folders don't duplicate entries.
      clips.update((c) => {
        const copy = [...c];
        for (const item of e.payload) {
          const idx = copy.findIndex(x => x.id === item.id);
          if (idx >= 0) copy[idx] = item;
          else copy.push(item);
        }
        return copy;
      });
    }));
    unlisten.push(await listen<void>('clip:scan:done', () => {
      unlisten.forEach(u => u());
      resolve();
    }));

    try {
      await invoke('scan_clip_folder', { path });
    } catch (e) {
      console.error('Clip scan failed:', e);
      unlisten.forEach(u => u());
      resolve();
    }
  });
}

// ── Public scan API ────────────────────────────────────────────────────────────

// Serialises scanClipFolder/refreshClipLibrary calls, same reasoning as
// library.ts's scanLock: avoids overlapping Tauri event listeners.
let scanLock: Promise<void> = Promise.resolve();

export async function scanClipFolder(path: string) {
  scanLock = scanLock.then(async () => {
    const paths = get(clipFolderPaths);
    const norm = (p: string) => p.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase();
    const normPath = norm(path);
    const exists = paths.some(p => norm(p) === normPath);
    if (!exists) {
      const next = [...paths, path];
      clipFolderPaths.set(next);
      saveClipFolders(next);
    }

    isClipScanning.set(true);
    clipScanStatus.set({ filesScanned: 0, clipsFound: 0, totalFiles: 0 });

    await scanOne(path);

    isClipScanning.set(false);
    await saveCache();
  }).catch((e) => { console.error('scanClipFolder error:', e); });
  return scanLock;
}

export async function refreshClipLibrary() {
  scanLock = scanLock.then(async () => {
    const paths = get(clipFolderPaths);
    if (!paths.length) return;

    isClipScanning.set(true);
    clips.set([]);
    clipScanStatus.set({ filesScanned: 0, clipsFound: 0, totalFiles: 0 });

    for (const path of paths) {
      await scanOne(path);
    }

    isClipScanning.set(false);
    await saveCache();
  }).catch((e) => { console.error('refreshClipLibrary error:', e); });
  return scanLock;
}

export function clearClipLibrary() {
  clips.set([]);
  clipFolderPaths.set([]);
  saveClipFolders([]);
  clipScanStatus.set({ filesScanned: 0, clipsFound: 0, totalFiles: 0 });
  invoke('save_clip_cache', { data: '[]' }).catch(() => {});
  invoke('clear_clip_thumbs').catch(() => {});
}

// ── Playback (native <video>, streamed from a local HTTP server) ──────────────
//
// clip_stream_server.rs serves the requested clip either stream-copied
// (remuxed, near-free) or live-transcoded to VP9/Opus WebM depending on
// getPlayableVideoCodecs(); ClipPlayerView plays that URL directly through
// a normal <video> element. Audio+video are one muxed stream, so unlike
// player.ts's music playback there's no separate rodio/audio_* transport
// to keep in sync here — the browser's own <video> element owns decode,
// timing, and AV sync entirely. Position/duration are read straight off
// the element (video.currentTime/duration) rather than tracked in a store
// here; see ClipPlayerView for that.

let streamInfo: { port: number; token: string } | null = null;

async function getStreamInfo(): Promise<{ port: number; token: string }> {
  if (!streamInfo) {
    const [port, token] = await invoke<[number, string]>('get_clip_stream_info');
    streamInfo = { port, token };
  }
  return streamInfo;
}

/** Builds the clip-stream URL for `path`, starting at `seekSecs`, sized to `box` (transcode-path only, see PLAYBACK_BOUND_MAX). */
export async function buildClipStreamUrl(path: string, seekSecs: number, box: { w: number; h: number }): Promise<string> {
  const { port, token } = await getStreamInfo();
  const params = new URLSearchParams({
    token,
    path,
    seek: seekSecs.toFixed(3),
    w: String(box.w),
    h: String(box.h),
    playable: getPlayableVideoCodecs().join(','),
  });
  return `http://127.0.0.1:${port}/stream?${params.toString()}`;
}

// Opening a clip stops whatever music was playing — a track can't sanely
// play under the clip's own audio track — and does NOT auto-resume it on
// close (surprising, but matches the old playClip's behavior).
export { stopMusic as prepareClipPlayback };
