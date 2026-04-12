import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { appDataDir, join } from '@tauri-apps/api/path';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { Track, Album } from '../types';
import { recordPlay, recordListened } from './stats';

const VOL_KEY   = 'mp_volume';
const TRACK_KEY = 'mp_last_track';

function loadVolume(): number {
  const v = parseFloat(localStorage.getItem(VOL_KEY) ?? '');
  return isNaN(v) ? 1.0 : Math.max(0, Math.min(1, v));
}

function saveVolume(v: number) {
  localStorage.setItem(VOL_KEY, String(v));
}

function saveLastTrack(track: Track | null, album: Album | null) {
  if (track && album) {
    localStorage.setItem(TRACK_KEY, JSON.stringify({ track, album }));
  } else {
    localStorage.removeItem(TRACK_KEY);
  }
}

export function loadLastTrack(): { track: Track; album: Album } | null {
  try {
    const raw = localStorage.getItem(TRACK_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch { return null; }
}

// ── Stores ────────────────────────────────────────────────────────────────────

export const currentTrack      = writable<Track | null>(null);
export const currentAlbum      = writable<Album | null>(null);
export const currentPlaylistId = writable<string | null>(null);
export const isPlaying         = writable(false);
export const isPaused          = writable(false);
export const volume            = writable(loadVolume());
export const position          = writable(0);
export const duration          = writable(0);
export const isShuffled        = writable(false);

export type RepeatMode = 'none' | 'one' | 'all';
export const repeatMode = writable<RepeatMode>('none');

// ── Source queue reactive mirrors (shuffle / playlist → stores) ──────────────
export const sourceQueueItems = writable<QueueItem[]>([]);
export const sourceQueueIndex = writable<number>(-1);

// ── User queue (manually added tracks, play before source resumes) ────────────
let _userQueue: QueueItem[] = [];
export const userQueueItems = writable<QueueItem[]>([]);

// Source return context — saved before the first user queue track plays in album mode.
// After the user queue exhausts, playback resumes from this position in the source album.
let _sourceReturnAlbum: Album | null = null;
let _sourceReturnTrackId: string | null = null;

/** Reactive mirror of the source return context — used by QueueView to show the correct upcoming tracks. */
export const sourceReturnContext = writable<{ album: Album; trackId: string } | null>(null);

function syncSourceReturn() {
  sourceReturnContext.set(
    _sourceReturnAlbum && _sourceReturnTrackId
      ? { album: _sourceReturnAlbum, trackId: _sourceReturnTrackId }
      : null
  );
}

export function toggleRepeat() {
  const modes: RepeatMode[] = ['none', 'one', 'all'];
  const cur = get(repeatMode);
  const next = modes[(modes.indexOf(cur) + 1) % modes.length];
  repeatMode.set(next);
  // Repeat One and Shuffle are mutually exclusive
  if (next === 'one') {
    _queue = [];
    _qIdx = -1;
    isShuffled.set(false);
    currentPlaylistId.set(null);
    syncSourceQueue();
  }
  preloadNext();
}

/**
 * Calculates the likely next track based on the current state (repeat, shuffle, album).
 */
function getNextTrack(): Track | null {
  const rm = get(repeatMode);
  const curTrack = get(currentTrack);
  const album = get(currentAlbum);

  if (rm === 'one') return curTrack;

  // User queue has highest priority
  if (_userQueue.length > 0) return _userQueue[0].track;

  if (_queue.length > 0) {
    const nextIdx = _qIdx + 1;
    if (nextIdx < _queue.length) return _queue[nextIdx].track;
    if (rm === 'all') return _queue[0].track;
    return null;
  }

  if (album && curTrack) {
    const idx = album.tracks.findIndex(t => t.id === curTrack.id);
    if (idx !== -1) {
      const nextIdx = idx + 1;
      if (nextIdx < album.tracks.length) return album.tracks[nextIdx];
      if (rm === 'all') return album.tracks[0];
    }
  }

  return null;
}

/**
 * Tells the backend to prepare the next track's file and decoder in advance.
 */
export async function preloadNext() {
  const next = getNextTrack();
  if (next) {
    console.log('[player] Preloading next track:', next.title);
    await invoke('audio_preload', { path: next.path });
  }
}

// ── Shuffle queue (module-level, not reactive) ────────────────────────────────

export type QueueItem = { track: Track; album: Album };
let _queue: QueueItem[] = [];
let _qIdx = -1;

function fisherYates<T>(arr: T[]): T[] {
  const a = [...arr];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

function syncSourceQueue() {
  sourceQueueItems.set([..._queue]);
  sourceQueueIndex.set(_qIdx);
}

function syncUserQueue() {
  userQueueItems.set([..._userQueue]);
}

// ── Polling ───────────────────────────────────────────────────────────────────

let pollTimer: ReturnType<typeof setInterval> | null = null;
let _advancing = false;

function startPolling() {
  if (pollTimer) return;
  pollTimer = setInterval(async () => {
    if (!get(currentTrack)) return;
    // Set _advancing = true synchronously before any await so that concurrent
    // interval ticks can't both slip past the guard (TOCTOU race fix).
    if (_advancing) return;
    _advancing = true;
    try {
      position.set(await invoke<number>('audio_get_position'));
      if (await invoke<boolean>('audio_is_finished')) {
        const finished = get(currentTrack);
        if (finished) recordListened(finished.id, get(duration));
        const rm = get(repeatMode);
        if (rm === 'one') {
          const track = get(currentTrack);
          const album = get(currentAlbum);
          if (track && album) await playTrack(track, album, _queue.length > 0);
        } else if (_userQueue.length > 0) {
          // User queue plays before source resumes.
          // Save where to return in the source album (only in album mode, first entry).
          if (_queue.length === 0 && !_sourceReturnAlbum) {
            _sourceReturnAlbum = get(currentAlbum);
            _sourceReturnTrackId = get(currentTrack)?.id ?? null;
            syncSourceReturn();
          }
          const next = _userQueue.shift()!;
          syncUserQueue();
          // Always fromShuffle=true so stopShuffle() doesn't wipe the source context.
          await playTrack(next.track, next.album, true);
        } else if (_queue.length > 0) {
          const next = _qIdx + 1;
          if (next < _queue.length) {
            _qIdx = next;
            syncSourceQueue();
            await playTrack(_queue[_qIdx].track, _queue[_qIdx].album, true);
          } else if (rm === 'all') {
            _qIdx = 0;
            syncSourceQueue();
            await playTrack(_queue[0].track, _queue[0].album, true);
          } else {
            // Queue exhausted — stop cleanly, keep track info for display
            _queue = [];
            _qIdx = -1;
            isShuffled.set(false);
            currentPlaylistId.set(null);
            syncSourceQueue();
            await invoke('audio_stop');
            isPlaying.set(false);
            isPaused.set(false);
            stopPolling();
          }
        } else {
          // Album mode: use saved source context if returning from user queue,
          // otherwise fall back to current album/track.
          const albumToUse = _sourceReturnAlbum ?? get(currentAlbum);
          const trackIdToUse = _sourceReturnTrackId ?? get(currentTrack)?.id;
          _sourceReturnAlbum = null;
          _sourceReturnTrackId = null;
          syncSourceReturn();
          if (albumToUse) {
            const idx = albumToUse.tracks.findIndex(t => t.id === trackIdToUse);
            if (rm === 'all') {
              const nextIdx = idx !== -1 ? (idx + 1) % albumToUse.tracks.length : 0;
              await playTrack(albumToUse.tracks[nextIdx], albumToUse);
            } else {
              const next = idx !== -1 ? albumToUse.tracks[idx + 1] : null;
              if (next) await playTrack(next, albumToUse);
              // else: end of album — stay silent, keep track info for display
            }
          }
        }
      }
    } finally {
      _advancing = false;
    }
  }, 1000);
}

function stopPolling() {
  if (pollTimer) { clearInterval(pollTimer); pollTimer = null; }
}

// ── Commands (unchanged from working version) ─────────────────────────────────

export function stopShuffle() {
  _queue = [];
  _qIdx = -1;
  isShuffled.set(false);
  currentPlaylistId.set(null);
  syncSourceQueue();
}

export async function playTrack(track: Track, album: Album, fromShuffle = false) {
  try {
    if (!fromShuffle) {
      stopShuffle(); // also clears currentPlaylistId
      // Explicit track selection cancels any pending source-return context.
      _sourceReturnAlbum = null;
      _sourceReturnTrackId = null;
      syncSourceReturn();
    }

    // Record listened time for the outgoing track before switching
    const prevTrack = get(currentTrack);
    if (prevTrack) recordListened(prevTrack.id, get(position));

    await invoke('audio_play', { path: track.path, duration: track.duration });
    currentAlbum.set(album);
    currentTrack.set(track);
    recordPlay(track.id);
    duration.set(track.duration);
    position.set(0);
    isPlaying.set(true);
    isPaused.set(false);
    saveLastTrack(track, album);
    startPolling();
    // Small delay to let the current track stabilize before starting disk I/O for the next one.
    setTimeout(() => preloadNext(), 500);
  } catch (e) {
    console.error('Play failed:', e);
  }
}

export async function pause() {
  await invoke('audio_pause');
  isPlaying.set(false);
  isPaused.set(true);
}

export async function resume() {
  const track = get(currentTrack);
  const album = get(currentAlbum);

  // If we have a track/album but are neither playing nor paused (initial state after restart),
  // we need to call playTrack to initialize the backend sink.
  if (track && album && !get(isPlaying) && !get(isPaused)) {
    await playTrack(track, album);
    return;
  }

  await invoke('audio_resume');
  isPlaying.set(true);
  isPaused.set(false);
  startPolling();
}

export async function stop() {
  await invoke('audio_stop');
  isPlaying.set(false);
  isPaused.set(false);
  currentTrack.set(null);
  currentPlaylistId.set(null);
  position.set(0);
  saveLastTrack(null, null);
  _queue = []; _qIdx = -1; isShuffled.set(false);
  syncSourceQueue();
  stopPolling();
}

export async function setVolume(v: number) {
  volume.set(v);
  saveVolume(v);
  await invoke('audio_set_volume', { volume: v });
}

const VOL_STEPS = 20;
const VOL_CURVE = 1.7;

export async function stepVolume(delta: number) {
  const v = get(volume);
  const currentStep = v <= 0 ? 0 : Math.round(Math.pow(v, 1 / VOL_CURVE) * VOL_STEPS);
  const nextStep = Math.max(0, Math.min(VOL_STEPS, currentStep + delta));
  const nextVol = nextStep === 0 ? 0 : Math.pow(nextStep / VOL_STEPS, VOL_CURVE);
  await setVolume(nextVol);
}

export async function seekTo(nextPosition: number) {
  const track = get(currentTrack);
  if (!track) return;

  const total = get(duration) || track.duration || 0;
  const clamped = Math.max(0, Math.min(nextPosition, total));
  position.set(clamped);

  try {
    await invoke('audio_seek', { position: clamped });
  } catch (e) {
    console.error('Seek failed:', e);
    position.set(await invoke<number>('audio_get_position'));
  }
}

export async function playNext(album: Album) {
  // User queue has priority over everything
  if (_userQueue.length > 0) {
    // Save source context before entering user queue (album mode only, first entry).
    if (_queue.length === 0 && !_sourceReturnAlbum) {
      _sourceReturnAlbum = get(currentAlbum);
      _sourceReturnTrackId = get(currentTrack)?.id ?? null;
      syncSourceReturn();
    }
    const next = _userQueue.shift()!;
    syncUserQueue();
    // Always fromShuffle=true so stopShuffle() doesn't wipe the source context.
    await playTrack(next.track, next.album, true);
    return;
  }
  if (_queue.length > 0) {
    const next = _qIdx + 1;
    if (next < _queue.length) {
      _qIdx = next;
      syncSourceQueue();
      await playTrack(_queue[_qIdx].track, _queue[_qIdx].album, true);
    } else {
      stopShuffle();
    }
    return;
  }
  // Album mode: use saved source context if returning from user queue.
  const albumToUse = _sourceReturnAlbum ?? album;
  const trackIdToUse = _sourceReturnTrackId ?? get(currentTrack)?.id;
  _sourceReturnAlbum = null;
  _sourceReturnTrackId = null;
  syncSourceReturn();
  if (!trackIdToUse) return;
  const idx = albumToUse.tracks.findIndex((t) => t.id === trackIdToUse);
  const next = albumToUse.tracks[idx + 1];
  if (next) await playTrack(next, albumToUse);
}

export async function playPrev(album: Album) {
  const track = get(currentTrack);
  if (!track) return;
  if (get(position) > 3) {
    // Restart the current track through playTrack so that stats, preload,
    // and saveLastTrack are handled consistently.
    await playTrack(track, album, _queue.length > 0);
    return;
  }
  if (_queue.length > 0) {
    const prev = _qIdx - 1;
    if (prev >= 0) {
      _qIdx = prev;
      syncSourceQueue();
      await playTrack(_queue[_qIdx].track, _queue[_qIdx].album, true);
    }
    return;
  }
  const idx = album.tracks.findIndex((t) => t.id === track.id);
  const prev = album.tracks[idx - 1];
  if (prev) await playTrack(prev, album);
}

// ── Shuffle ───────────────────────────────────────────────────────────────────

export async function playShuffledAll(albums: Album[]) {
  if (get(isShuffled) && _queue.length > albumCount(albums)) {
    stopShuffle();
    return;
  }
  if (get(repeatMode) === 'one') repeatMode.set('none');
  const all: QueueItem[] = albums.flatMap(a => a.tracks.map(t => ({ track: t, album: a })));
  const current = get(currentTrack);
  const currentAlbumVal = get(currentAlbum);
  if (current && currentAlbumVal && (get(isPlaying) || get(isPaused))) {
    const rest = all.filter(item => item.track.id !== current.id);
    _queue = [{ track: current, album: currentAlbumVal }, ...fisherYates(rest)];
    _qIdx = 0;
    isShuffled.set(true);
    syncSourceQueue();
    preloadNext();
  } else {
    _queue = fisherYates(all);
    _qIdx = 0;
    isShuffled.set(true);
    syncSourceQueue();
    if (_queue[0]) await playTrack(_queue[0].track, _queue[0].album, true);
  }
}

function albumCount(albums: Album[]): number {
  return albums.length;
}

export async function playShuffled(album: Album) {
  if (get(isShuffled) && _queue.length === album.tracks.length) {
    stopShuffle();
    return;
  }
  if (get(repeatMode) === 'one') repeatMode.set('none');
  const current = get(currentTrack);
  if (current && (get(isPlaying) || get(isPaused))) {
    const rest = album.tracks.filter(t => t.id !== current.id).map(t => ({ track: t, album }));
    _queue = [{ track: current, album }, ...fisherYates(rest)];
    _qIdx = 0;
    isShuffled.set(true);
    syncSourceQueue();
    preloadNext();
  } else {
    _queue = fisherYates(album.tracks.map(t => ({ track: t, album })));
    _qIdx = 0;
    isShuffled.set(true);
    syncSourceQueue();
    if (_queue[0]) await playTrack(_queue[0].track, _queue[0].album, true);
  }
}

export async function playPlaylist(items: QueueItem[], startIdx = 0, playlistId: string | null = null) {
  if (!items.length) return;
  _queue = [...items];
  _qIdx = Math.max(0, Math.min(startIdx, items.length - 1));
  isShuffled.set(false);
  currentPlaylistId.set(playlistId);
  syncSourceQueue();
  await playTrack(_queue[_qIdx].track, _queue[_qIdx].album, true);
}

export async function playShuffledPlaylist(items: QueueItem[], playlistId: string | null = null) {
  if (!items.length) return;
  if (get(repeatMode) === 'one') repeatMode.set('none');
  const current = get(currentTrack);
  if (current && (get(isPlaying) || get(isPaused))) {
    const currentItem = items.find(i => i.track.id === current.id);
    const rest = items.filter(i => i.track.id !== current.id);
    const head = currentItem ?? { track: current, album: get(currentAlbum)! };
    _queue = [head, ...fisherYates(rest)];
    _qIdx = 0;
    isShuffled.set(true);
    currentPlaylistId.set(playlistId);
    syncSourceQueue();
    preloadNext();
  } else {
    _queue = fisherYates([...items]);
    _qIdx = 0;
    isShuffled.set(true);
    currentPlaylistId.set(playlistId);
    syncSourceQueue();
    await playTrack(_queue[0].track, _queue[0].album, true);
  }
}

export async function initVolume() {
  const v = get(volume);
  await invoke('audio_set_volume', { volume: v });
}

// ── OS Media Controls Sync ───────────────────────────────────────────────────

// getCoverUrl removed because album.cover_art already contains the absolute path.

currentTrack.subscribe(async track => {
  if (track) {
    const album = get(currentAlbum);
    const coverUrl = album?.cover_art; // Use the path already stored in the album object
    
    // We update metadata first, but we DON'T await it forever if it's failing
    invoke('update_media_metadata', { 
      title: track.title, 
      artist: track.artist, 
      album: album?.title || 'Unknown',
      coverUrl: coverUrl, // Tauri 2 maps camelCase JS to snake_case Rust
      durationMs: Math.floor(track.duration * 1000)
    }).catch(err => {
      console.error('SMTC Metadata update failed:', err);
    }).finally(() => {
      // Vital: Wait a tiny bit (100ms) to ensure Windows SMTC has "settled" 
      // the new metadata before we push the "Playing" state update.
      setTimeout(syncPlayback, 100);
    });
  }
});

function syncPlayback() {
  const playing = get(isPlaying);
  const pos = Math.floor(get(position) * 1000); // ms
  invoke('update_media_playback_state', { 
    isPlaying: playing,
    positionMs: pos
  }).catch(console.error);
}

// Sync on major state changes
isPlaying.subscribe(syncPlayback);
isPaused.subscribe(syncPlayback);

// We should also sync when a new track starts playing to ensure "Playing" state is fresh
currentTrack.subscribe(() => {
    setTimeout(syncPlayback, 100);
});

// Periodic heartbeat sync for Windows 11 (otherwise OS widget may freeze or show 'Play' icon)
let heartbeatTimer: number | null = null;
isPlaying.subscribe(playing => {
  if (playing) {
    if (!heartbeatTimer) {
      heartbeatTimer = window.setInterval(syncPlayback, 5000);
    }
  } else {
    if (heartbeatTimer) {
      clearInterval(heartbeatTimer);
      heartbeatTimer = null;
    }
  }
});

// ── Listen for OS Events ─────────────────────────────────────────────────────

listen<string>('smtc-event', (event) => handleSystemAction(event.payload));
listen<string>('thumbnail-event', (event) => handleSystemAction(event.payload));

async function handleSystemAction(action: string) {
  console.log('SMTC action received:', action, 'Current state isPlaying:', get(isPlaying));
  const album = get(currentAlbum);
  
  if (action === 'play') {
    await resume();
  } else if (action === 'pause') {
    await pause();
  } else if (action === 'toggle') {
    if (get(isPlaying)) await pause(); else await resume();
  } else if (action === 'next') {
    if (album) await playNext(album);
  } else if (action === 'previous') {
    if (album) await playPrev(album);
  }

  // Sync back immediately to keep OS widget state consistent
  setTimeout(syncPlayback, 50);
}

// ── User queue management (tracks added manually, play before source resumes) ──

/** Append a track to the user queue. Plays after current track, before source. */
export function addToQueue(track: Track, album: Album) {
  _userQueue.push({ track, album });
  syncUserQueue();
  preloadNext();
}

/** Remove a track from the user queue by index. */
export function removeFromQueue(idx: number) {
  if (idx < 0 || idx >= _userQueue.length) return;
  _userQueue.splice(idx, 1);
  syncUserQueue();
  preloadNext();
}

/** Reorder user queue (drag-to-reorder). */
export function moveInQueue(from: number, to: number) {
  if (from === to || from < 0 || to < 0 || from >= _userQueue.length || to >= _userQueue.length) return;
  const [item] = _userQueue.splice(from, 1);
  _userQueue.splice(to, 0, item);
  syncUserQueue();
  preloadNext();
}

/** Clear the entire user queue. Source (album / shuffle) is unaffected. */
export function clearQueue() {
  _userQueue = [];
  _sourceReturnAlbum = null;
  _sourceReturnTrackId = null;
  syncSourceReturn();
  syncUserQueue();
  preloadNext();
}

// ── Source queue navigation ───────────────────────────────────────────────────

/**
 * Play a specific item in the user queue immediately.
 * Items before it are dropped (skipped). Items after it remain queued.
 */
export async function playFromUserQueue(idx: number) {
  if (idx < 0 || idx >= _userQueue.length) return;
  // Save source context before playing user queue (album mode only, first entry).
  if (_queue.length === 0 && !_sourceReturnAlbum) {
    _sourceReturnAlbum = get(currentAlbum);
    _sourceReturnTrackId = get(currentTrack)?.id ?? null;
    syncSourceReturn();
  }
  const item = _userQueue[idx];
  _userQueue = _userQueue.slice(idx + 1); // keep what comes after
  syncUserQueue();
  // Always fromShuffle=true so stopShuffle() doesn't wipe the source context.
  await playTrack(item.track, item.album, true);
}

/** Jump to a specific absolute index in the source (shuffle/playlist) queue. */
export async function jumpToInSourceQueue(absIdx: number) {
  if (absIdx < 0 || absIdx >= _queue.length) return;
  _qIdx = absIdx;
  syncSourceQueue();
  await playTrack(_queue[_qIdx].track, _queue[_qIdx].album, true);
}

/** Jump to a specific track in the current album (album mode only). */
export async function jumpToInAlbum(track: Track, album: Album) {
  await playTrack(track, album, false);
}
