import { writable, get } from 'svelte/store';
import type { Track } from '../types';

export interface Playlist {
  id: string;
  name: string;
  tracks: Track[];
}

const PLAYLISTS_KEY = 'mp_playlists';

function loadPlaylists(): Playlist[] {
  try {
    const saved = JSON.parse(localStorage.getItem(PLAYLISTS_KEY) ?? 'null');
    if (Array.isArray(saved) && saved.length > 0) return saved;
  } catch {}
  return [{ id: 'favourites', name: 'Favourites', tracks: [] }];
}

export const playlists = writable<Playlist[]>(loadPlaylists());

playlists.subscribe((value) => {
  localStorage.setItem(PLAYLISTS_KEY, JSON.stringify(value));
});

export function addToPlaylist(playlistId: string, track: Track) {
  playlists.update((pls) =>
    pls.map((pl) => {
      if (pl.id !== playlistId) return pl;
      if (pl.tracks.some((t) => t.id === track.id)) return pl;
      return { ...pl, tracks: [...pl.tracks, track] };
    })
  );
}

export function createPlaylist(name: string): string {
  const id = `pl_${Date.now()}`;
  playlists.update((pls) => [...pls, { id, name: name.trim(), tracks: [] }]);
  return id;
}

export function isTrackInPlaylist(playlistId: string, trackId: string): boolean {
  return get(playlists)
    .find((pl) => pl.id === playlistId)
    ?.tracks.some((t) => t.id === trackId) ?? false;
}
