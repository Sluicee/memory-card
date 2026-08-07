use any_ascii::any_ascii;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use walkdir::WalkDir;

use crate::ffmpeg_util::{build_ffmpeg_command, parse_probe_stderr};
use crate::scanner::cover_filename;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Clip {
    pub id: String,
    pub path: String,
    pub title: String,
    pub thumbnail: Option<String>,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub search_index: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClipScanProgress {
    pub files_scanned: u32,
}

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "webm", "avi", "mov", "wmv", "flv", "mpg", "mpeg", "ts", "m2ts",
];
const THUMB_SEEK_SECS: f64 = 0.5;
const THUMB_MAX_W: u32 = 480;
// How many frames the `thumbnail` filter scans (starting at THUMB_SEEK_SECS)
// before picking the most representative one. ~30 frames covers roughly
// 1-1.5s of content at typical frame rates — enough to skip past a black
// open/logo card without decoding much extra.
const THUMB_CANDIDATE_FRAMES: u32 = 30;

/// One ffmpeg spawn does triple duty: thumbnail extraction, duration, and
/// resolution. ffmpeg always prints the input header analysis (`Duration:`,
/// `Video: ..., WxH, ... fps`) to stderr before it even gets to the output
/// stage — including when `-n` causes it to refuse an already-cached
/// thumbnail and exit early (verified empirically). That means a warm
/// re-scan still gets accurate duration/resolution without re-encoding any
/// thumbnails, and there's no need for a separate ffprobe binary (not
/// bundled) or a second spawn per file.
fn probe_and_thumbnail(app: &tauri::AppHandle, path: &Path, thumbs_dir: &Path) -> Option<Clip> {
    let path_str = path.to_string_lossy().to_string();
    let id = path_str.clone();

    let file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Unknown".to_string());

    let thumb_path = thumbs_dir.join(cover_filename(&id, "image/jpeg"));

    let mut cmd = build_ffmpeg_command(app).ok()?;
    // `thumbnail=N` picks the most "representative" frame out of the next N
    // (an internal frame-difference/histogram heuristic), rather than
    // blindly grabbing whatever's at a fixed timestamp — many music videos
    // open on a black screen or logo card for a few seconds, and a fixed
    // -ss landed on that far more often than not.
    let vf = format!(
        "thumbnail={n},scale='min({w},iw)':-2",
        n = THUMB_CANDIDATE_FRAMES,
        w = THUMB_MAX_W,
    );
    cmd.args(["-n", "-ss", &format!("{:.3}", THUMB_SEEK_SECS)])
        .args(["-i", &path_str])
        .args(["-frames:v", "1", "-vf", &vf, "-q:v", "4"])
        .arg(&thumb_path);

    let output = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let info = parse_probe_stderr(&stderr);

    let title = file_stem;
    let search_index = any_ascii(&title).to_lowercase();

    Some(Clip {
        id,
        path: path_str,
        title,
        thumbnail: thumb_path.exists().then(|| thumb_path.to_string_lossy().into_owned()),
        // Unprobeable (`Duration: N/A`, e.g. some live-remuxed streams) becomes
        // 0.0 — frontend treats that as "unknown length" and relies on
        // end-of-stream detection rather than a clamp, same tolerance
        // FFmpegSource's silence-timeout already has for audio.
        duration: info.duration_secs.unwrap_or(0.0),
        width: info.width.unwrap_or(0),
        height: info.height.unwrap_or(0),
        search_index,
    })
}

/// Scan a folder for video clips — flat list, no artist/album grouping
/// (unlike `scanner::scan_folder`, clips aren't organized that way).
pub fn scan_clip_folder(folder_path: &str, app: &tauri::AppHandle, thumbs_dir: &Path) -> Vec<Clip> {
    let paths: Vec<PathBuf> = WalkDir::new(folder_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file()
                && e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| VIDEO_EXTENSIONS.contains(&x.to_lowercase().as_str()))
                    .unwrap_or(false)
        })
        .map(|e| e.into_path())
        .collect();

    let total = paths.len() as u32;
    app.emit("clip:scan:start", serde_json::json!({ "total": total })).ok();

    let counter = Arc::new(AtomicU32::new(0));
    let app_ref = app.clone();
    let cnt = Arc::clone(&counter);

    let mut clips: Vec<Clip> = paths
        .par_iter()
        .filter_map(|path| {
            let result = probe_and_thumbnail(&app_ref, path, thumbs_dir);
            let n = cnt.fetch_add(1, Ordering::Relaxed) + 1;
            // Emitted more often than the audio scanner's every-200 — a per-file
            // ffmpeg spawn is much heavier than an in-process tag read, so scans
            // are smaller/slower and users benefit from finer-grained feedback.
            if n % 10 == 0 || n == total {
                app_ref.emit("clip:scan:progress", ClipScanProgress { files_scanned: n }).ok();
            }
            result
        })
        .collect();

    clips.sort_by(|a, b| a.title.cmp(&b.title));
    clips
}
