const STATS_KEY = 'mp_stats';

export interface TrackStats {
  playCount: number;
  lastPlayed: number;    // Date.now() ms
  totalListened: number; // seconds actually heard
}

export type StatsMap = Record<string, TrackStats>; // track.id → stats

export function loadStats(): StatsMap {
  try { return JSON.parse(localStorage.getItem(STATS_KEY) ?? '{}'); } catch { return {}; }
}

function saveStats(s: StatsMap) {
  localStorage.setItem(STATS_KEY, JSON.stringify(s));
}

/** Call when a track starts playing. */
export function recordPlay(trackId: string): void {
  const s = loadStats();
  const prev = s[trackId] ?? { playCount: 0, lastPlayed: 0, totalListened: 0 };
  s[trackId] = { ...prev, playCount: prev.playCount + 1, lastPlayed: Date.now() };
  saveStats(s);
}

/** Call when leaving a track (switch or finish). seconds = position at that moment. */
export function recordListened(trackId: string, seconds: number): void {
  if (seconds < 2) return;
  const s = loadStats();
  const prev = s[trackId] ?? { playCount: 0, lastPlayed: 0, totalListened: 0 };
  s[trackId] = { ...prev, totalListened: prev.totalListened + Math.round(seconds) };
  saveStats(s);
}

export function clearStats(): void {
  localStorage.removeItem(STATS_KEY);
}
