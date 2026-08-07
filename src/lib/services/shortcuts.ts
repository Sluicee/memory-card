import { playUiSfx } from "$lib/ui-sfx";
import { viewMode } from "$lib/stores/viewMode";
import { selectedAlbum } from "$lib/stores/library";
import { selectedClip } from "$lib/stores/clips";
import {
  currentTrack,
  currentAlbum,
  volume,
  setVolume,
  stepVolume,
  toggleRepeat,
} from "$lib/stores/player";
import type { Playlist } from "$lib/stores/playlists";

export interface ShortcutCallbacks {
  optionsOpen: boolean;
  setOptionsOpen: (v: boolean) => void;
  statsOpen: boolean;
  setStatsOpen: (v: boolean) => void;
  npPickerOpen: boolean;
  setNpPickerOpen: (v: boolean) => void;
  selectedPlaylist: Playlist | null;
  setSelectedPlaylist: (v: Playlist | null) => void;
  searchOpen: boolean;
  toggleSearch: () => void;
  setSearchOpen: (v: boolean) => void;
  setSearchQuery: (v: string) => void;
  selectedArtistFilter: string | null;
  setSelectedArtistFilter: (v: string | null) => void;
  activeTab: "library" | "artists" | "playlists" | "queue" | "clips";
  setActiveTab: (tab: "library" | "artists" | "playlists" | "queue" | "clips") => void;
  setFollowPlayback: (v: boolean) => void;
  handleTransportPlayPause: () => void;
  handlePrev: () => void;
  handleNext: () => void;
  handleShuffleAll: () => void;
}

let mutedVolume = 0;

export function handleGlobalKeydown(
  e: KeyboardEvent,
  callbacks: ShortcutCallbacks,
) {
  const tag = (e.target as HTMLElement)?.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA") return;

  let getSelectedAlbum: any;
  selectedAlbum.subscribe((val) => (getSelectedAlbum = val))();
  let getSelectedClip: any;
  selectedClip.subscribe((val) => (getSelectedClip = val))();

  switch (e.code) {
    case "Space":
      e.preventDefault();
      let track: any;
      currentTrack.subscribe((t) => (track = t))();
      if (track) callbacks.handleTransportPlayPause();
      break;
    case "ArrowLeft":
      e.preventDefault();
      let alb: any;
      currentAlbum.subscribe((a) => (alb = a))();
      if (alb) callbacks.handlePrev();
      break;
    case "ArrowRight":
      e.preventDefault();
      let albRight: any;
      currentAlbum.subscribe((a) => (albRight = a))();
      if (albRight) callbacks.handleNext();
      break;
    case "ArrowUp":
      e.preventDefault();
      playUiSfx("steps");
      stepVolume(1);
      break;
    case "ArrowDown":
      e.preventDefault();
      playUiSfx("steps");
      stepVolume(-1);
      break;
    case "KeyS":
      callbacks.handleShuffleAll();
      break;
    case "KeyR":
      playUiSfx("confirm");
      toggleRepeat();
      break;
    case "KeyF":
    case "Slash":
      callbacks.toggleSearch();
      break;
    case "KeyM":
      let curVol = 0;
      volume.subscribe((v) => (curVol = v))();
      if (curVol > 0) {
        mutedVolume = curVol;
        setVolume(0);
      } else {
        setVolume(mutedVolume || 1);
      }
      playUiSfx("steps");
      break;
    case "Escape":
      let curViewMode = "normal";
      viewMode.subscribe((v) => (curViewMode = v))();
      if (getSelectedClip) {
        // ClipPlayerView sits above everything else (z-index 250) — closing
        // it takes priority over whatever else might technically be "open"
        // underneath. One step: exits fullscreen/focus (if active) and
        // closes the clip together.
        playUiSfx("back");
        if (curViewMode !== "normal") viewMode.set("normal");
        selectedClip.set(null);
      } else if (curViewMode !== "normal") {
        playUiSfx("back");
        viewMode.set("normal");
      } else if (callbacks.optionsOpen) {
        callbacks.setOptionsOpen(false);
      } else if (callbacks.statsOpen) {
        callbacks.setStatsOpen(false);
      } else if (callbacks.npPickerOpen) {
        callbacks.setNpPickerOpen(false);
      } else if (callbacks.selectedPlaylist) {
        callbacks.setSelectedPlaylist(null);
      } else if (getSelectedAlbum) {
        selectedAlbum.set(null);
        callbacks.setFollowPlayback(false);
      } else if (callbacks.searchOpen) {
        playUiSfx("back");
        callbacks.setSearchOpen(false);
        callbacks.setSearchQuery("");
      } else if (callbacks.selectedArtistFilter) {
        playUiSfx("back");
        callbacks.setSelectedArtistFilter(null);
      }
      break;
    case "Digit1":
      callbacks.setActiveTab("library");
      callbacks.setSelectedArtistFilter(null);
      playUiSfx("back");
      break;
    case "Digit2":
      callbacks.setActiveTab("artists");
      playUiSfx("confirm");
      break;
    case "Digit3":
      callbacks.setActiveTab("playlists");
      playUiSfx("confirm");
      break;
    case "Digit4":
      callbacks.setActiveTab("queue");
      playUiSfx("confirm");
      break;
    case "Digit5":
      callbacks.setActiveTab("clips");
      playUiSfx("confirm");
      break;
  }
}
