import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { Track, Album } from '../types';

export const currentTrack = writable<Track | null>(null);
export const currentAlbum = writable<Album | null>(null);
export const isPlaying = writable(false);
export const isPaused = writable(false);
export const volume = writable(1.0);

export async function playTrack(track: Track, album: Album) {
  try {
    await invoke('audio_play', { path: track.path, duration: track.duration });
    currentTrack.set(track);
    currentAlbum.set(album);
    isPlaying.set(true);
    isPaused.set(false);
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
  await invoke('audio_resume');
  isPlaying.set(true);
  isPaused.set(false);
}

export async function stop() {
  await invoke('audio_stop');
  isPlaying.set(false);
  isPaused.set(false);
  currentTrack.set(null);
}

export async function setVolume(v: number) {
  volume.set(v);
  await invoke('audio_set_volume', { volume: v });
}

export async function playNext(album: Album) {
  const track = get(currentTrack);
  if (!track) return;
  const idx = album.tracks.findIndex((t) => t.id === track.id);
  const next = album.tracks[idx + 1];
  if (next) await playTrack(next, album);
}

export async function playPrev(album: Album) {
  const track = get(currentTrack);
  if (!track) return;
  const idx = album.tracks.findIndex((t) => t.id === track.id);
  const prev = album.tracks[idx - 1];
  if (prev) await playTrack(prev, album);
}

export async function playShuffled(album: Album) {
  const shuffled = [...album.tracks].sort(() => Math.random() - 0.5);
  if (shuffled.length > 0) await playTrack(shuffled[0], album);
}
