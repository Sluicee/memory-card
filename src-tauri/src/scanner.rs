use lofty::prelude::*;
use any_ascii::any_ascii;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey};
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
    #[serde(default)]
    pub search_index: String,
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
    #[serde(default)]
    pub search_index: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScanProgress {
    pub files_scanned: u32,
    pub albums_found: u32,
}

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "m4a", "aac", "wav", "opus"];
const FOLDER_COVER_NAMES: &[&str] = &["cover", "folder", "front", "album", "albumart", "artwork"];
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png"];

/// FNV-1a hash — deterministic filename-safe identifier for album cover files.
pub fn cover_filename(album_id: &str, mime: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in album_id.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let ext = if mime.contains("png") { "png" } else { "jpg" };
    format!("{:016x}.{}", hash, ext)
}

struct TrackResult {
    track: Track,
    explicit_album_artist: Option<String>,
}

fn get_tag_year(tag: &lofty::tag::Tag) -> Option<u32> {
    if let Some(y) = tag.get_string(ItemKey::Year).and_then(|s| s.parse().ok()) {
        return Some(y);
    }
    if let Some(d) = tag.get_string(ItemKey::RecordingDate) {
        if d.len() >= 4 {
            if let Ok(y) = d[..4].parse::<u32>() {
                return Some(y);
            }
        }
    }
    None
}

/// Fallback duration calculator using symphonia when lofty metadata parsing fails or duration is 0
fn get_audio_duration_fallback(path: &Path) -> f64 {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let Ok(file) = std::fs::File::open(path) else { return 0.0 };
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    if let Ok(probed) = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()) {
        if let Some(track) = probed.format.tracks().first() {
            let tb = track.codec_params.time_base;
            let n_frames = track.codec_params.n_frames;
            if let (Some(tb), Some(n_frames)) = (tb, n_frames) {
                let time = tb.calc_time(n_frames);
                return time.seconds as f64 + time.frac;
            }
        }
    }
    0.0
}

/// Opens the audio file and extracts track metadata with safe fallbacks.
fn read_track_and_cover(path: &Path) -> Option<TrackResult> {
    println!("Scanning: {}", path.display());
    let path_str = path.to_string_lossy().to_string();

    let file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Unknown".to_string());

    let tagged_file_opt = Probe::open(path)
        .ok()
        .and_then(|p| {
            p.options(lofty::config::ParseOptions::new().parsing_mode(lofty::config::ParsingMode::Relaxed))
                .guess_file_type()
                .ok()?
                .read()
                .ok()
        })
        .or_else(|| {
            Probe::open(path).ok().and_then(|p| p.read().ok())
        });

    let (title, artist, album, explicit_album_artist, track_number, disc_number, duration, year) = match tagged_file_opt {
        Some(ref tagged_file) => {
            let mut duration = tagged_file.properties().duration().as_secs_f64();
            if duration <= 0.0 {
                duration = get_audio_duration_fallback(path);
            }
            let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

            let title = tag
                .and_then(|t| t.title().as_deref().map(String::from))
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    if let Some((_, t)) = file_stem.split_once(" - ") {
                        t.trim().to_string()
                    } else {
                        file_stem.clone()
                    }
                });

            let artist = tag
                .and_then(|t| t.artist().as_deref().map(String::from))
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    if let Some((a, _)) = file_stem.split_once(" - ") {
                        a.trim().to_string()
                    } else {
                        "Unknown Artist".to_string()
                    }
                });

            let album = tag
                .and_then(|t| t.album().as_deref().map(String::from))
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Unknown Album".to_string());

            let explicit_album_artist = tag.and_then(|t| t.get_string(ItemKey::AlbumArtist).map(String::from));
            let track_number = tag.and_then(|t| t.track()).unwrap_or(0);
            let disc_number = tag.and_then(|t| t.disk()).unwrap_or(1);
            let year = tag.and_then(get_tag_year);

            (title, artist, album, explicit_album_artist, track_number, disc_number, duration, year)
        }
        None => {
            // Fallback for files lofty failed to probe/read (e.g. corrupt ID3 tags, non-standard Mp3tag edits)
            let (artist, title) = if let Some((a, t)) = file_stem.split_once(" - ") {
                (a.trim().to_string(), t.trim().to_string())
            } else {
                ("Unknown Artist".to_string(), file_stem.clone())
            };
            let album = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown Album".to_string());
            let explicit_album_artist = None;
            let track_number = 0;
            let disc_number = 1;
            let duration = get_audio_duration_fallback(path);
            let year = None;

            (title, artist, album, explicit_album_artist, track_number, disc_number, duration, year)
        }
    };

    let album_artist = explicit_album_artist.clone().unwrap_or_else(|| artist.clone());

    let search_index = format!(
        "{} {}",
        any_ascii(&title).to_lowercase(),
        any_ascii(&artist).to_lowercase()
    );
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
            search_index,
        },
        explicit_album_artist,
    })
}

/// Reads the embedded cover art from a specific file with pictures enabled (lazy loading).
fn read_embedded_cover_from_file(path: &Path) -> Option<(Vec<u8>, String)> {
    println!("Reading embedded cover from: {}", path.display());
    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged_file.primary_tag()?;
    let pic = tag.pictures().first()?;
    let mime = pic.mime_type()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "image/jpeg".to_string());
    Some((pic.data().to_vec(), mime))
}

/// Look for a cover image file in the album folder.
/// Returns the destination path in covers_dir if found.
fn find_folder_cover(audio_path: &Path, album_id: &str, covers_dir: &Path) -> Option<String> {
    let folder = audio_path.parent()?;

    // Try well-known names first
    for name in FOLDER_COVER_NAMES {
        for ext in IMAGE_EXTENSIONS {
            let candidate = folder.join(format!("{}.{}", name, ext));
            if candidate.exists() {
                return copy_image_to_covers(&candidate, album_id, covers_dir);
            }
        }
    }

    // Fall back to any image in the folder
    let entries = std::fs::read_dir(folder).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    return copy_image_to_covers(&p, album_id, covers_dir);
                }
            }
        }
    }

    None
}

/// Write embedded tag cover bytes directly — no decode/resize needed since
/// tag art is already small (ID3 embeds are typically ≤ 600px).
fn save_embedded_cover(data: &[u8], mime: &str, album_id: &str, covers_dir: &Path) -> Option<String> {
    let dest = covers_dir.join(cover_filename(album_id, mime));
    if dest.exists() {
        return Some(dest.to_string_lossy().into_owned());
    }
    std::fs::write(&dest, data).ok()?;
    Some(dest.to_string_lossy().into_owned())
}

/// Copy a folder image, resizing to max 600px if needed.
/// Folder art can be 3000+ px, so we decode and resize.
fn copy_image_to_covers(src: &Path, album_id: &str, covers_dir: &Path) -> Option<String> {
    println!("Resizing cover image: {:?}", src);
    let dest = covers_dir.join(cover_filename(album_id, "image/jpeg"));
    if dest.exists() {
        return Some(dest.to_string_lossy().into_owned());
    }
    let data = std::fs::read(src).ok()?;
    let img = image::load_from_memory(&data).ok()?;
    let img = if img.width() > 600 || img.height() > 600 {
        img.resize(600, 600, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    img.save_with_format(&dest, image::ImageFormat::Jpeg).ok()?;
    Some(dest.to_string_lossy().into_owned())
}

/// Strip "feat."/"ft."/"featuring" and everything after it from an artist string.
/// Used for album key normalization so "Artist feat. Guest" groups with "Artist".
fn strip_feat(artist: &str) -> &str {
    let trimmed = artist.trim();
    let lower = trimmed.to_lowercase();
    let patterns = [" feat. ", " feat ", " ft. ", " ft ", " featuring ", " (feat.", " (ft."];
    let mut min_char_idx = trimmed.chars().count();

    for pattern in patterns {
        if let Some(byte_idx) = lower.find(pattern) {
            let char_idx = lower[..byte_idx].chars().count();
            if char_idx < min_char_idx {
                min_char_idx = char_idx;
            }
        }
    }

    let end_byte_idx = trimmed
        .char_indices()
        .nth(min_char_idx)
        .map(|(idx, _)| idx)
        .unwrap_or(trimmed.len());

    trimmed[..end_byte_idx].trim()
}

/// Strip disc-number suffix from album title for grouping purposes.
/// "Album (Disc 1)" → "Album", "Album Disc 2" → "Album", "Album [CD 1]" → "Album"
fn strip_disc_suffix(album: &str) -> &str {
    let trimmed = album.trim();
    let lower = trimmed.to_lowercase();

    // Bracketed forms: (Disc N), [Disc N], (CD N), [CD N]
    for (open, close, keyword) in [('(', ')', "disc "), ('(', ')', "cd "), ('[', ']', "disc "), ('[', ']', "cd ")] {
        if lower.ends_with(close) {
            if let Some(start_byte_idx) = lower.rfind(open) {
                let inner = &lower[start_byte_idx + 1..lower.len() - 1];
                if let Some(rest) = inner.strip_prefix(keyword) {
                    if !rest.is_empty() && rest.trim_start_matches(|c: char| c.is_ascii_digit()).is_empty() {
                        let char_idx = lower[..start_byte_idx].chars().count();
                        let end_byte_idx = trimmed.char_indices().nth(char_idx).map(|(idx, _)| idx).unwrap_or(trimmed.len());
                        return trimmed[..end_byte_idx].trim_end();
                    }
                }
            }
        }
    }

    // Unbracketed forms at end: " Disc N", " CD N"
    for keyword in ["disc ", "cd "] {
        if let Some(idx_byte_idx) = lower.rfind(keyword) {
            let after = &lower[idx_byte_idx + keyword.len()..];
            if !after.is_empty() && after.trim_start_matches(|c: char| c.is_ascii_digit()).is_empty()
                && (idx_byte_idx == 0 || lower.as_bytes()[idx_byte_idx - 1] == b' ') {
                let char_idx = lower[..idx_byte_idx].chars().count();
                let end_byte_idx = trimmed.char_indices().nth(char_idx).map(|(idx, _)| idx).unwrap_or(trimmed.len());
                return trimmed[..end_byte_idx].trim_end();
            }
        }
    }

    trimmed
}

/// Helper to handle grouping paths logically (e.g. merging CD 1 and CD 2 folders)
fn canonical_folder(path: &Path) -> String {
    let mut parent = path.parent().unwrap_or(path);
    if let Some(name) = parent.file_name().and_then(|n| n.to_str()) {
        let lower = name.to_lowercase();
        let is_cd = lower.starts_with("cd");
        let is_disc = lower.starts_with("disc");
        if is_cd || is_disc {
            let prefix_len = if is_cd { 2 } else { 4 };
            let rest = lower[prefix_len..].trim();
            if rest.is_empty() || rest.chars().all(|c| c.is_ascii_digit()) {
                if let Some(p) = parent.parent() {
                    parent = p;
                }
            }
        }
    }
    parent.to_string_lossy().replace('\\', "/")
}

/// Scan a folder and return all albums with embedded or folder-based cover art.
/// Internet cover fetching is handled separately in lib.rs (async context).
pub fn scan_folder(folder_path: &str, app: &tauri::AppHandle, covers_dir: &Path) -> Vec<Album> {
    // Phase 1: collect audio paths
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

    // Tell the frontend how many files we're about to process
    app.emit("scan:start", serde_json::json!({ "total": total })).ok();

    let counter = Arc::new(AtomicU32::new(0));
    let app_ref = app.clone();
    let cnt = Arc::clone(&counter);

    // Phase 2: read metadata in parallel
    let results: Vec<(TrackResult, PathBuf)> = paths
        .par_iter()
        .filter_map(|path| {
            let result = read_track_and_cover(path)?;
            let n = cnt.fetch_add(1, Ordering::Relaxed) + 1;
            if n.is_multiple_of(200) || n == total {
                app_ref.emit("scan:progress", ScanProgress { files_scanned: n, albums_found: 0 }).ok();
            }
            Some((result, path.clone()))
        })
        .collect();

    // Phase 3: group into albums
    let mut albums: HashMap<String, Album> = HashMap::new();

    for (result, audio_path) in results {
        let TrackResult { track, explicit_album_artist } = result;
        
        let album_name_norm = strip_disc_suffix(track.album.trim()).to_lowercase();
        let album_key = if album_name_norm.is_empty() || album_name_norm == "unknown album" {
            format!("unknown::{}", strip_feat(track.artist.trim()).to_lowercase())
        } else if let Some(eaa) = &explicit_album_artist {
            format!("{}::{}", strip_feat(eaa.trim()).to_lowercase(), album_name_norm)
        } else {
            let folder = canonical_folder(&audio_path);
            format!("{}::{}", folder.to_lowercase(), album_name_norm)
        };
        let album = albums.entry(album_key.clone()).or_insert_with(|| Album {
            id: album_key,
            title: track.album.trim().to_string(),
            artist: track.album_artist.trim().to_string(),
            year: track.year,
            cover_art: None,
            tracks: Vec::new(),
            total_duration: 0.0,
            search_index: format!(
                "{} {}",
                any_ascii(track.album.trim()).to_lowercase(),
                any_ascii(track.album_artist.trim()).to_lowercase()
            ),
        });

        album.total_duration += track.duration;
        album.tracks.push(track);
    }

    // Phase 4: sort, finalize compilation artists, and resolve cover art (ONCE PER ALBUM)
    let mut album_list: Vec<Album> = albums.into_values().collect();
    for album in &mut album_list {
        album.tracks.sort_by_key(|t| (t.disc_number, t.track_number));

        // Resolve cover art ONCE for the entire album using the first track's path
        if let Some(first_track) = album.tracks.first() {
            let audio_path = Path::new(&first_track.path);
            
            // 1. Check cache first (most frequent case for large libraries)
            let cached_path = covers_dir.join(cover_filename(&album.id, "image/jpeg"));
            if cached_path.exists() {
                album.cover_art = Some(cached_path.to_string_lossy().into_owned());
            }

            // 2. Folder image
            if album.cover_art.is_none() {
                album.cover_art = find_folder_cover(audio_path, &album.id, covers_dir);
            }

            // 3. Embedded tag cover (read on demand from the current track's file)
            if album.cover_art.is_none() {
                if let Some((data, mime)) = read_embedded_cover_from_file(audio_path) {
                    album.cover_art = save_embedded_cover(&data, &mime, &album.id, covers_dir);
                }
            }
        }

        let mut unique_base_artists: Vec<String> = Vec::new();
        let mut display_artists: Vec<String> = Vec::new();

        for t in &album.tracks {
            let base = strip_feat(t.album_artist.trim()).trim().to_string();
            if !unique_base_artists.iter().any(|b| b.eq_ignore_ascii_case(&base)) {
                unique_base_artists.push(base.clone());
                display_artists.push(base);
            }
        }

        if display_artists.len() == 1 {
            album.artist = display_artists[0].clone();
        } else {
            album.artist = display_artists.join(", ");
        }

        // Automatically expand search index
        album.search_index = format!(
            "{} {}",
            any_ascii(&album.title).to_lowercase(),
            any_ascii(&album.artist).to_lowercase()
        );
    }
    album_list.sort_by(|a, b| a.title.cmp(&b.title));
    album_list
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
