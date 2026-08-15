export interface Track {
  id: string;
  path: string;
  title: string;
  artist: string;
  album: string;
  album_artist: string;
  track_number: number;
  disc_number: number;
  duration: number;
  year: number | null;
  search_index: string;
}

export interface Album {
  id: string;
  title: string;
  artist: string;
  year: number | null;
  cover_art: string | null;
  tracks: Track[];
  total_duration: number;
  search_index: string;
}

export interface Artist {
  name: string;
  albums: Album[];
}

export interface Clip {
  id: string;
  path: string;
  title: string;
  thumbnail: string | null;
  duration: number;
  width: number;
  height: number;
  search_index: string;
  /** ffmpeg's codec names, filled in by the scanner. Absent on clips scanned
   *  before these were recorded — see isRemuxable in stores/clips.ts. */
  video_codec?: string | null;
  audio_codec?: string | null;
}

export interface EqualizerSettings {
  enabled: boolean;
  preamp: number;
  gains: number[];
}

export interface EqualizerPreset {
  id: string;
  name: string;
  preamp: number;
  gains: number[];
}

