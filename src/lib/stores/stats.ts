const STATS_KEY = 'mp_stats';
const HISTORY_KEY = 'mp_history';
const MAX_HISTORY_ENTRIES = 50000;

export interface TrackStats {
  playCount: number;
  lastPlayed: number;    // Date.now() ms
  totalListened: number; // seconds actually heard
}

export interface HistoryEntry {
  id: string;
  trackId: string;
  timestamp: number;     // Date.now() ms when play was qualified
  listenedSeconds: number;
}

export type StatsMap = Record<string, TrackStats>; // track.id → stats

export function loadStats(): StatsMap {
  try { return JSON.parse(localStorage.getItem(STATS_KEY) ?? '{}'); } catch { return {}; }
}

function saveStats(s: StatsMap) {
  localStorage.setItem(STATS_KEY, JSON.stringify(s));
}

export function loadHistory(): HistoryEntry[] {
  try { return JSON.parse(localStorage.getItem(HISTORY_KEY) ?? '[]'); } catch { return []; }
}

function saveHistory(history: HistoryEntry[]) {
  if (history.length > MAX_HISTORY_ENTRIES) {
    history = history.slice(history.length - MAX_HISTORY_ENTRIES);
  }
  localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
}

/** Call when a track reaches the listening threshold (>= 25% duration or >= 10s). Returns entry ID. */
export function recordPlay(trackId: string): string {
  const timestamp = Date.now();
  const entryId = `${timestamp}_${Math.random().toString(36).substring(2, 7)}`;

  const s = loadStats();
  const prev = s[trackId] ?? { playCount: 0, lastPlayed: 0, totalListened: 0 };
  s[trackId] = { ...prev, playCount: prev.playCount + 1, lastPlayed: timestamp };
  saveStats(s);

  const history = loadHistory();
  history.push({
    id: entryId,
    trackId,
    timestamp,
    listenedSeconds: 0,
  });
  saveHistory(history);

  return entryId;
}

/** Call when updating listened position or leaving a track. */
export function recordListened(trackId: string, seconds: number, entryId?: string | null): void {
  const roundedSeconds = Math.round(seconds);
  if (roundedSeconds < 2) return;

  const s = loadStats();
  const prev = s[trackId] ?? { playCount: 0, lastPlayed: 0, totalListened: 0 };
  s[trackId] = { ...prev, totalListened: prev.totalListened + roundedSeconds };
  saveStats(s);

  if (entryId) {
    const history = loadHistory();
    const entry = history.find(e => e.id === entryId);
    if (entry) {
      entry.listenedSeconds = Math.max(entry.listenedSeconds, roundedSeconds);
      saveHistory(history);
    }
  }
}

/** Returns play history entries filtered by a timestamp range (e.g. for annual Wrapped recaps). */
export function getHistoryForPeriod(startMs: number, endMs: number): HistoryEntry[] {
  const history = loadHistory();
  return history.filter(e => e.timestamp >= startMs && e.timestamp <= endMs);
}

export function clearStats(): void {
  localStorage.removeItem(STATS_KEY);
  localStorage.removeItem(HISTORY_KEY);
}

const REPAIR_FLAG_KEY = 'mp_stats_totalListened_repair_v1';

/**
 * One-time repair for a stats-inflation bug: the playback poller used to
 * call recordListened() every second with the growing absolute position,
 * and recordListened() ADDS its argument to totalListened rather than
 * replacing it — so a single play summed roughly threshold+…+duration
 * instead of counting duration once (a 4-minute track could inflate to
 * hours). History entries were unaffected (Math.max, not addition above),
 * so totalListened per track is rebuilt here from the still-accurate
 * history log. No-ops after the first run.
 */
export function repairTotalListenedFromHistory(): void {
  if (localStorage.getItem(REPAIR_FLAG_KEY)) return;

  const history = loadHistory();
  const totals: Record<string, number> = {};
  for (const entry of history) {
    totals[entry.trackId] = (totals[entry.trackId] ?? 0) + entry.listenedSeconds;
  }

  const s = loadStats();
  for (const trackId of Object.keys(totals)) {
    if (s[trackId]) {
      s[trackId] = { ...s[trackId], totalListened: totals[trackId] };
    }
  }
  saveStats(s);

  localStorage.setItem(REPAIR_FLAG_KEY, '1');
}

