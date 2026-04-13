import { writable, derived } from 'svelte/store';

export type Locale = 'en' | 'ru' | 'ja';

const LOCALES: Locale[] = ['en', 'ru', 'ja'];
const STORAGE_KEY = 'locale';

function loadLocale(): Locale {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === 'en' || v === 'ru' || v === 'ja') return v;
  } catch {}
  return 'en';
}

export const locale = writable<Locale>(loadLocale());

locale.subscribe((v) => {
  try { localStorage.setItem(STORAGE_KEY, v); } catch {}
});

export function toggleLocale() {
  locale.update((l) => {
    const i = LOCALES.indexOf(l);
    return LOCALES[(i + 1) % LOCALES.length];
  });
}

// ---------------------------------------------------------------------------
// Translations
// ---------------------------------------------------------------------------

type Dict = typeof en;
type DictKey = keyof Dict;

const en = {
  // Header
  memoryCard: 'Memory Card',
  scanning: 'Scanning…',
  startingScan: 'Starting scan…',
  scanFiles: (files: number, albums: number) => `${files} files · ${albums} albums`,
  scanFilesFound: (files: number, albums: number) => `${files} files · ${albums} albums found`,
  albumsFound: 'albums found',
  selectHint: 'Select',
  optionsWord: '⚙ Options',
  selectHintSuffix: 'to choose a music folder',
  noResultsFor: 'No results for',
  dropToAdd: 'Drop to add to library',
  // Tabs
  library: 'Library',
  playlists: 'Playlists',
  // Transport
  prev: 'Prev',
  next: 'Next',
  play: 'Play',
  pause: 'Pause',
  // Actions
  select: 'Select',
  search: 'Search',
  searchPlaceholder: 'Search…',
  shuffle: 'Shuffle',
  repeat: 'Repeat',
  repeatOne: 'Repeat 1',
  repeatAll: 'Repeat All',
  noTrackPlaying: 'No track playing',
  // Queue
  upNext: 'Up Next',
  nowPlaying: 'Now Playing',
  addToQueue: 'Add to queue',
  clearQueue: 'Clear',
  queueEmpty: 'Queue is empty',
  queueAddHint: 'Add tracks from an album view',
  queueTab: 'Queue',
  nextFromAlbum: 'Next from album',
  nextFromPlaylist: 'Next from playlist',
  nextFromShuffle: 'Next from shuffle',
  // Options menu
  getUpdate: (v: string) => `Get Update (${v})`,
  addFolder: 'Add new folder',
  refreshLibrary: 'Refresh library',
  statistics: 'Statistics',
  sfx: (on: boolean) => `SFX: ${on ? 'ON' : 'OFF'}`,
  autostart: (on: boolean) => `Launch at startup: ${on ? 'ON' : 'OFF'}`,
  discordRpc: (on: boolean) => `Discord RPC: ${on ? 'ON' : 'OFF'}`,
  clearLibrary: 'Clear library',
  close: 'Close',
  switchLanguage: 'English',
  // Album / playlist view
  back: 'Back',
  disc: (n: number) => `DISC ${n}`,
  trackCount: (n: number) => `${n} tracks`,
  noTracksYet: 'No tracks yet — add them with +',
  // Playlist picker
  alreadyAdded: '— added',
  newPlaylist: 'New playlist…',
  create: 'Create',
  playlistNamePlaceholder: 'Playlist name…',
  // Stats
  totalPlays: 'total plays',
  totalListened: 'total listened',
  artistsLabel: 'artists',
  tracksLabel: 'tracks',
  artistsTab: 'Artists',
  albumsTab: 'Albums',
  tracksTab: 'Tracks',
  recentTab: 'Recent',
  noPlaysYet: 'No plays yet',
  areYouSure: 'Are you sure?',
  clearStats: 'Clear stats',
  lastSeen: 'last',
  justNow: 'just now',
  minutesAgo: (n: number) => `${n}m ago`,
  hoursAgo: (n: number) => `${n}h ago`,
  daysAgo: (n: number) => `${n}d ago`,
};

const ru: Dict = {
  // Header
  memoryCard: 'Memory Card',
  scanning: 'Сканирование…',
  startingScan: 'Запуск…',
  scanFiles: (files: number, albums: number) => `${files} файлов · ${albums} альбомов`,
  scanFilesFound: (files: number, albums: number) => `${files} файлов · ${albums} альбомов найдено`,
  albumsFound: 'альбомов найдено',
  selectHint: 'Выберите',
  optionsWord: '⚙ Настройки',
  selectHintSuffix: 'чтобы добавить папку с музыкой',
  noResultsFor: 'Ничего не найдено по запросу',
  dropToAdd: 'Отпустите, чтобы добавить',
  // Tabs
  library: 'Библиотека',
  playlists: 'Плейлисты',
  // Transport
  prev: 'Назад',
  next: 'Далее',
  play: 'Играть',
  pause: 'Пауза',
  // Actions
  select: 'Выбрать',
  search: 'Поиск',
  searchPlaceholder: 'Поиск…',
  shuffle: 'Случайно',
  repeat: 'Повтор',
  repeatOne: 'Повтор 1',
  repeatAll: 'Повтор всех',
  noTrackPlaying: 'Ничего не играет',
  // Queue
  upNext: 'Далее',
  nowPlaying: 'Сейчас играет',
  addToQueue: 'В очередь',
  clearQueue: 'Очистить',
  queueEmpty: 'Очередь пуста',
  queueAddHint: 'Добавьте треки из альбома',
  queueTab: 'Очередь',
  nextFromAlbum: 'Далее из альбома',
  nextFromPlaylist: 'Далее из плейлиста',
  nextFromShuffle: 'Далее в шаффле',
  // Options menu
  getUpdate: (v: string) => `Обновить (${v})`,
  addFolder: 'Добавить папку',
  refreshLibrary: 'Обновить библиотеку',
  statistics: 'Статистика',
  sfx: (on: boolean) => `Звуки: ${on ? 'ВКЛ' : 'ВЫКЛ'}`,
  autostart: (on: boolean) => `Автозапуск: ${on ? 'ВКЛ' : 'ВЫКЛ'}`,
  discordRpc: (on: boolean) => `Discord RPC: ${on ? 'ВКЛ' : 'ВЫКЛ'}`,
  clearLibrary: 'Очистить библиотеку',
  close: 'Закрыть',
  switchLanguage: 'Русский',
  // Album / playlist view
  back: 'Назад',
  disc: (n: number) => `ДИСК ${n}`,
  trackCount: (n: number) => {
    const n10 = n % 10, n100 = n % 100;
    if (n10 === 1 && n100 !== 11) return `${n} трек`;
    if (n10 >= 2 && n10 <= 4 && (n100 < 10 || n100 >= 20)) return `${n} трека`;
    return `${n} треков`;
  },
  noTracksYet: 'Треков нет — добавьте с помощью +',
  // Playlist picker
  alreadyAdded: '— добавлен',
  newPlaylist: 'Новый плейлист…',
  create: 'Создать',
  playlistNamePlaceholder: 'Название плейлиста…',
  // Stats
  totalPlays: 'прослушиваний',
  totalListened: 'всего прослушано',
  artistsLabel: 'исполнителей',
  tracksLabel: 'треков',
  artistsTab: 'Исполнители',
  albumsTab: 'Альбомы',
  tracksTab: 'Треки',
  recentTab: 'Недавние',
  noPlaysYet: 'Нет прослушиваний',
  areYouSure: 'Уверены?',
  clearStats: 'Сбросить статистику',
  lastSeen: 'был',
  justNow: 'только что',
  minutesAgo: (n: number) => `${n} мин. назад`,
  hoursAgo: (n: number) => `${n} ч. назад`,
  daysAgo: (n: number) => `${n} дн. назад`,
};

const ja: Dict = {
  // Header
  memoryCard: 'Memory Card',
  scanning: 'スキャン中…',
  startingScan: '起動中…',
  scanFiles: (files: number, albums: number) => `${files} ファイル · ${albums} アルバム`,
  scanFilesFound: (files: number, albums: number) => `${files} ファイル · ${albums} アルバム見つかりました`,
  albumsFound: 'アルバム見つかりました',
  selectHint: '選択',
  optionsWord: '⚙ オプション',
  selectHintSuffix: 'して音楽フォルダを追加',
  noResultsFor: '検索結果なし:',
  dropToAdd: 'ドロップして追加',
  // Tabs
  library: 'ライブラリ',
  playlists: 'プレイリスト',
  // Transport
  prev: '前へ',
  next: '次へ',
  play: '再生',
  pause: '一時停止',
  // Actions
  select: '選択',
  search: '検索',
  searchPlaceholder: '検索…',
  shuffle: 'シャッフル',
  repeat: 'リピート',
  repeatOne: 'リピート1',
  repeatAll: 'リピート全',
  noTrackPlaying: '再生なし',
  // Queue
  upNext: '次の曲',
  nowPlaying: '再生中',
  addToQueue: 'キューに追加',
  clearQueue: 'クリア',
  queueEmpty: 'キューは空です',
  queueAddHint: 'アルバムからトラックを追加',
  queueTab: 'キュー',
  nextFromAlbum: 'アルバムから次へ',
  nextFromPlaylist: 'プレイリストから次へ',
  nextFromShuffle: 'シャッフルから次へ',
  // Options menu
  getUpdate: (v: string) => `アップデート (${v})`,
  addFolder: 'フォルダを追加',
  refreshLibrary: 'ライブラリを更新',
  statistics: '統計',
  sfx: (on: boolean) => `効果音: ${on ? 'ON' : 'OFF'}`,
  autostart: (on: boolean) => `自動起動: ${on ? 'ON' : 'OFF'}`,
  discordRpc: (on: boolean) => `Discord RPC: ${on ? 'ON' : 'OFF'}`,
  clearLibrary: 'ライブラリをクリア',
  close: '閉じる',
  switchLanguage: '日本語',
  // Album / playlist view
  back: '戻る',
  disc: (n: number) => `ディスク ${n}`,
  trackCount: (n: number) => `${n} トラック`,
  noTracksYet: 'トラックなし — + で追加',
  // Playlist picker
  alreadyAdded: '— 追加済み',
  newPlaylist: '新しいプレイリスト…',
  create: '作成',
  playlistNamePlaceholder: 'プレイリスト名…',
  // Stats
  totalPlays: '再生回数',
  totalListened: '総再生時間',
  artistsLabel: 'アーティスト',
  tracksLabel: 'トラック',
  artistsTab: 'アーティスト',
  albumsTab: 'アルバム',
  tracksTab: 'トラック',
  recentTab: '最近',
  noPlaysYet: '再生履歴なし',
  areYouSure: '本当に?',
  clearStats: '統計をクリア',
  lastSeen: '最終',
  justNow: 'たった今',
  minutesAgo: (n: number) => `${n}分前`,
  hoursAgo: (n: number) => `${n}時間前`,
  daysAgo: (n: number) => `${n}日前`,
};

// ---------------------------------------------------------------------------
// t — reactive translation function
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type TranslateFn = (key: DictKey, ...args: any[]) => string;

export const t = derived<typeof locale, TranslateFn>(locale, ($locale) => {
  const dict: Dict = $locale === 'ru' ? ru : $locale === 'ja' ? ja : en;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (key: DictKey, ...args: any[]): string => {
    const val = dict[key] ?? en[key];
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if (typeof val === 'function') return (val as (...a: any[]) => string)(...args);
    return val as string;
  };
});
