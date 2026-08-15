# Memory Card

A desktop music player with a retro PS2/CRT aesthetic, built with Tauri 2, SvelteKit 5, and Rust.

## Screenshots

![Album Grid](docs/screenshots/album-grid.png)
![Album Detail](docs/screenshots/album-detail.png)
![Player](docs/screenshots/player.png)

## Features

- Scans local folders for music, streaming albums to the UI as they are discovered
- Supports MP3, FLAC, OGG, WAV, M4A/AAC
- Embedded cover art displayed as a 3D spinning disc (Three.js)
- CRT scanlines, vignette, and PS2 color palette
- Shuffle, volume control, seek
- Playlists, play queue (with per-item removal and clear), and listening stats
- Equalizer with per-band gain and preamp, applied via bundled FFmpeg audio filters
- Clips: a music-video tab that scans a separate folder and plays videos in-window/fullscreen, streamed locally through the bundled FFmpeg sidecar (remuxed or transcoded to WebM as needed) — no system codec dependency
- Gamepad navigation across grids, lists, and the clip/equalizer panels
- Library cache persisted to disk — fast startup after first scan

## Installation

### Arch Linux (AUR)

You can install `memory-card` directly from the AUR using `yay` (or any other AUR helper):

```bash
yay -S memory-card-bin
```

### Windows & Other Linux Distros

Download the pre-compiled packages (`.exe` or `.deb`) from the **[GitHub Releases](https://github.com/Sluicee/memory-card/releases)** page.

### Updating

When a newer release exists, an update entry appears at the top of the options menu. On Windows,
macOS and Linux (AppImage, `.deb`, `.rpm`) it downloads and installs the update in place, with a
progress bar, and restarts the app — the `.deb`/`.rpm` path asks for your password, since it hands
the package to `dpkg`/`rpm`. AUR installs are left to `pacman`, and the entry opens the release
page instead.

## Documentation & Controls

For detailed information on how to organize your music and how the library is scanned, see the **[User Manual](docs/MANUAL.md)** ([RU](docs/MANUAL_RU.md)).

### Hotkeys

| Key | Action |
| --- | --- |
| `Space` | Play / Pause |
| `←` / `→` | Previous / Next Track |
| `↑` / `↓` | Volume Up / Down |
| `S` / `R` | Shuffle / Repeat |
| `F` or `/` | Search |
| `1`–`5` | Switch Tabs (Library/Artists/Playlists/Queue/Clips) |


## Tech Stack

| Layer    | Technology                                             |
|----------|---------------------------------------------------------|
| Frontend | SvelteKit 5, TypeScript, Three.js, Vite                |
| Backend  | Rust, Tauri 2, rodio, symphonia, lofty, tokio, tiny_http |
| Media    | FFmpeg, bundled as a sidecar binary (equalizer filters, clip probing/remux/transcode) |
| IPC      | Tauri `invoke` / `listen`, plus a local HTTP server for clip streaming |

## Requirements

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/) (stable)
- [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS

## Development

```bash
# Install frontend dependencies
npm install

# Run the full desktop app (recommended)
npx tauri dev

# Frontend-only dev server (Vite on :1420)
npm run dev

# Type-check
npm run check
```

## Build

```bash
npx tauri build
```

Produces a platform-native installer in `src-tauri/target/release/bundle/`.

## Notes

- Window is fixed at 950x900 and non-resizable by design.
- M4A/AAC files are decoded via symphonia entirely in memory to work around a rodio seek limitation.
- Playback position and track-end detection use a 1-second polling loop on the frontend.
- Settings and last-played track are stored in `localStorage`. The full library cache lives in Tauri's app data directory.
- Equalizer gain/preamp is applied by piping audio through the bundled FFmpeg sidecar with an `equalizer`/`volume` filter chain, rather than in-process DSP.
- Clips are served to a native `<video>` element by a local HTTP server (`127.0.0.1`, OS-assigned port): stream-copied (remuxed) when the source codec is already playable by the webview, otherwise live-transcoded to VP9/Opus WebM — both paths go through the same bundled FFmpeg sidecar.

## License

This project is licensed under the **GNU General Public License v3.0** (GPLv3). See the [LICENSE](LICENSE) file for details.

