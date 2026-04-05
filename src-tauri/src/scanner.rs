use base64::{engine::general_purpose::STANDARD, Engine};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
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

/// Scans folder, emitting events as albums are built up.
/// Events:
///   "scan:progress" -> ScanProgress
///   "scan:album"    -> Album  (emitted when album is complete)
///   "scan:done"     -> u32 (total album count)
pub fn scan_folder_streaming(folder_path: &str, app: &tauri::AppHandle) {
    let mut albums: HashMap<String, Album> = HashMap::new();
    let mut files_scanned: u32 = 0;

    for entry in WalkDir::new(folder_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let is_audio = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false);

        if !is_audio {
            continue;
        }

        files_scanned += 1;

        if let Ok(track) = read_track(path) {
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

            if album.cover_art.is_none() {
                album.cover_art = read_cover_art(path);
            }

            album.total_duration += track.duration;
            album.tracks.push(track);
        }

        // Emit progress every 10 files
        if files_scanned % 10 == 0 {
            let _ = app.emit(
                "scan:progress",
                ScanProgress {
                    files_scanned,
                    albums_found: albums.len() as u32,
                },
            );
        }
    }

    // Sort tracks and emit completed albums
    let mut album_list: Vec<Album> = albums.into_values().collect();
    for album in &mut album_list {
        album.tracks.sort_by_key(|t| (t.disc_number, t.track_number));
    }
    album_list.sort_by(|a, b| a.title.cmp(&b.title));

    for album in &album_list {
        let _ = app.emit("scan:album", album);
    }

    let _ = app.emit("scan:done", album_list.len() as u32);
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

fn read_track(path: &Path) -> Result<Track, Box<dyn std::error::Error>> {
    let tagged_file = Probe::open(path)?.read()?;
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

    Ok(Track {
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
    })
}

fn read_cover_art(path: &Path) -> Option<String> {
    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged_file.primary_tag()?;
    let picture = tag.pictures().first()?;
    let mime = picture
        .mime_type()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "image/jpeg".to_string());
    let data = STANDARD.encode(picture.data());
    Some(format!("data:{};base64,{}", mime, data))
}
