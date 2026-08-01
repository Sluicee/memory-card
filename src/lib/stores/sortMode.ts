import { writable } from 'svelte/store';

export type SortMode = 'artist' | 'title' | 'year';

const SORT_KEY = 'mp_sort_mode';

function loadSortMode(): SortMode {
  try {
    const s = localStorage.getItem(SORT_KEY);
    if (s === 'title' || s === 'year' || s === 'artist') return s;
  } catch {}
  return 'artist';
}

export const sortMode = writable<SortMode>(loadSortMode());

sortMode.subscribe((v) => {
  try {
    localStorage.setItem(SORT_KEY, v);
  } catch {}
});

export function cycleSortMode(): SortMode {
  let next: SortMode = 'artist';
  sortMode.update((curr) => {
    if (curr === 'artist') next = 'title';
    else if (curr === 'title') next = 'year';
    else next = 'artist';
    return next;
  });
  return next;
}
