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

// ── Polling ───────────────────────────────────────────────────────────────────

let pollTimer: ReturnType<typeof setInterval> | null = null;

function startPolling() {
  if (pollTimer) return;
  pollTimer = setInterval(async () => {
    if (!get(currentTrack)) return;
    position.set(await invoke<number>('audio_get_position'));
    if (await invoke<boolean>('audio_is_finished')) {
      const finished = get(currentTrack);
      if (finished) recordListened(finished.id, get(duration));
      if (_queue.length > 0) {
        const next = _qIdx + 1;
        if (next < _queue.length) {
          _qIdx = next;
          await playTrack(_queue[_qIdx].track, _queue[_qIdx].album, true);
        } else {
          // Queue exhausted — stop cleanly, keep track info for display
          _queue = [];
          _qIdx = -1;
          isShuffled.set(false);
          currentPlaylistId.set(null);
          await invoke('audio_stop');
          isPlaying.set(false);
          isPaused.set(false);
          stopPolling();
        }
      } else {
        const album = get(currentAlbum);
        if (album) await playNext(album);
      }
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
}

export async function playTrack(track: Track, album: Album, fromShuffle = false) {
  try {
    if (!fromShuffle) {
      stopShuffle(); // also clears currentPlaylistId
    }

    // Record listened time for the outgoing track before switching
    const prevTrack = get(currentTrack);
    if (prevTrack) recordListened(prevTrack.id, get(position));

    await invoke('audio_play', { path: track.path, duration: track.duration });
    currentTrack.set(track);
    recordPlay(track.id);
    currentAlbum.set(album);
    duration.set(track.duration);
    position.set(0);
    isPlaying.set(true);
    isPaused.set(false);
    saveLastTrack(track, album);
    startPolling();
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
  stopPolling();
}

export async function setVolume(v: number) {
  volume.set(v);
  saveVolume(v);
  await invoke('audio_set_volume', { volume: v });
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
  if (_queue.length > 0) {
    const next = _qIdx + 1;
    if (next < _queue.length) { 
      _qIdx = next; 
      await playTrack(_queue[_qIdx].track, _queue[_qIdx].album, true); 
    } else {
      stopShuffle();
      // If we finished shuffle queue, maybe play next in album? 
      // For now, just stop or let it finish.
    }
    return;
  }
  const track = get(currentTrack);
  if (!track) return;
  const idx = album.tracks.findIndex((t) => t.id === track.id);
  const next = album.tracks[idx + 1];
  if (next) await playTrack(next, album);
}

export async function playPrev(album: Album) {
  const track = get(currentTrack);
  if (!track) return;
  if (get(position) > 3) {
    await invoke('audio_play', { path: track.path, duration: track.duration });
    position.set(0);
    return;
  }
  if (_queue.length > 0) {
    const prev = _qIdx - 1;
    if (prev >= 0) { 
      _qIdx = prev; 
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
    // Basic toggle: if already shuffled across all, stop
    stopShuffle();
    return;
  }
  const all: QueueItem[] = albums.flatMap(a => a.tracks.map(t => ({ track: t, album: a })));
  _queue = fisherYates(all);
  _qIdx = 0;
  isShuffled.set(true);
  if (_queue[0]) await playTrack(_queue[0].track, _queue[0].album, true);
}

function albumCount(albums: Album[]): number {
  return albums.length;
}

export async function playShuffled(album: Album) {
  if (get(isShuffled) && _queue.length === album.tracks.length) {
    // Basic toggle: if already shuffled this specific album, stop
    stopShuffle();
    return;
  }
  _queue = fisherYates(album.tracks.map(t => ({ track: t, album })));
  _qIdx = 0;
  isShuffled.set(true);
  if (_queue[0]) await playTrack(_queue[0].track, _queue[0].album, true);
}

export async function playPlaylist(items: QueueItem[], startIdx = 0, playlistId: string | null = null) {
  if (!items.length) return;
  _queue = [...items];
  _qIdx = Math.max(0, Math.min(startIdx, items.length - 1));
  isShuffled.set(false);
  currentPlaylistId.set(playlistId);
  await playTrack(_queue[_qIdx].track, _queue[_qIdx].album, true);
}

export async function playShuffledPlaylist(items: QueueItem[], playlistId: string | null = null) {
  if (!items.length) return;
  _queue = fisherYates([...items]);
  _qIdx = 0;
  isShuffled.set(true);
  currentPlaylistId.set(playlistId);
  await playTrack(_queue[0].track, _queue[0].album, true);
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
