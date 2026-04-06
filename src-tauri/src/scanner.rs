use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Track {
    pub id: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub track_number: u32,
    pub disc_number: u32,
    pub duration: f64,
    pub year: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub cover_art: Option<String>,
    pub tracks: Vec<Track>,
    pub total_duration: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScanProgress {
    pub files_scanned: u32,
    pub albums_found: u32,
}

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "m4a", "aac", "wav", "opus"];

/// FNV-1a hash — deterministic filename-safe identifier for album cover files.
fn cover_filename(album_id: &str, mime: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in album_id.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let ext = if mime.contains("png") { "png" } else { "jpg" };
    format!("{:016x}.{}", hash, ext)
}

/// Result of reading a single audio file. Cover data is returned inline
/// so the file is opened exactly once per track.
struct TrackResult {
    track: Track,
    /// Raw cover bytes + MIME type, if present in this file's tags.
    cover: Option<(Vec<u8>, String)>,
}

/// Opens the audio file once and extracts both track metadata and cover art.
fn read_track_and_cover(path: &Path) -> Option<TrackResult> {
    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    let duration = tagged_file.properties().duration().as_secs_f64();
    let path_str = path.to_string_lossy().to_string();

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let tag = tagged_file.primary_tag();

    let title = tag
        .and_then(|t| t.title().as_deref().map(String::from))
        .unwrap_or(file_stem);
    let artist = tag
        .and_then(|t| t.artist().as_deref().map(String::from))
        .unwrap_or_else(|| "Unknown Artist".to_string());
    let album = tag
        .and_then(|t| t.album().as_deref().map(String::from))
        .unwrap_or_else(|| "Unknown Album".to_string());
    let album_artist = tag
        .and_then(|t| t.get_string(&ItemKey::AlbumArtist).map(String::from))
        .unwrap_or_else(|| artist.clone());
    let track_number = tag.and_then(|t| t.track()).unwrap_or(0);
    let disc_number = tag.and_then(|t| t.disk()).unwrap_or(1);
    let year = tag.and_then(|t| t.year());

    // Extract cover from the same TaggedFile — no second open needed
    let cover = tag
        .and_then(|t| t.pictures().first())
        .map(|pic| {
            let mime = pic.mime_type()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "image/jpeg".to_string());
            (pic.data().to_vec(), mime)
        });

    Some(TrackResult {
        track: Track {
            id: path_str.clone(),
            path: path_str,
            title,
            artist,
            album,
            album_artist,
            track_number,
            disc_number,
            duration,
            year,
        },
        cover,
    })
}

/// Scans folder, emitting events as albums are built up.
/// Events:
///   "scan:progress" -> ScanProgress
///   "scan:album"    -> Album  (emitted when album is complete)
///   "scan:done"     -> ()
pub fn scan_folder_streaming(folder_path: &str, app: &tauri::AppHandle, covers_dir: &Path) {
    // Phase 1: collect audio paths (fast, sequential)
    let paths: Vec<PathBuf> = WalkDir::new(folder_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file()
                && e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| AUDIO_EXTENSIONS.contains(&x.to_lowercase().as_str()))
                    .unwrap_or(false)
        })
        .map(|e| e.into_path())
        .collect();

    let total = paths.len() as u32;

    // Phase 2: read metadata in parallel (one file open per track)
    let files_scanned = Arc::new(AtomicU32::new(0));
    let app_ref = app.clone();
    let counter = Arc::clone(&files_scanned);

    let results: Vec<TrackResult> = paths
        .par_iter()
        .filter_map(|path| {
            let result = read_track_and_cover(path);
            let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
            if n % 50 == 0 || n == total {
                app_ref
                    .emit("scan:progress", ScanProgress { files_scanned: n, albums_found: 0 })
                    .ok();
            }
            result
        })
        .collect();

    // Phase 3: group into albums (sequential — avoids mutex contention)
    let mut albums: HashMap<String, Album> = HashMap::new();
    for result in results {
        let TrackResult { track, cover } = result;
        let album_key = format!("{}::{}", track.album_artist, track.album);
        let album = albums.entry(album_key.clone()).or_insert_with(|| Album {
            id: album_key,
            title: track.album.clone(),
            artist: track.album_artist.clone(),
            year: track.year,
            cover_art: None,
            tracks: Vec::new(),
            total_duration: 0.0,
        });

        // Save cover only for the first track that has one for this album
        if album.cover_art.is_none() {
            if let Some((data, mime)) = cover {
                let filename = cover_filename(&album.id, &mime);
                let dest = covers_dir.join(&filename);
                if std::fs::write(&dest, &data).is_ok() {
                    album.cover_art = Some(dest.to_string_lossy().into_owned());
                }
            }
        }

        album.total_duration += track.duration;
        album.tracks.push(track);
    }

    // Phase 4: sort and emit
    let mut album_list: Vec<Album> = albums.into_values().collect();
    for album in &mut album_list {
        album.tracks.sort_by_key(|t| (t.disc_number, t.track_number));
    }
    album_list.sort_by(|a, b| a.title.cmp(&b.title));

    for album in &album_list {
        app.emit("scan:album", album).ok();
    }
    app.emit("scan:done", ()).ok();
}

pub fn calculate_library_size(folder_path: &str) -> u64 {
    WalkDir::new(folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
