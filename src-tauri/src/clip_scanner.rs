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
    // Codec names as ffmpeg reports them ("av1", "vp9", "opus"), from the same
    // probe that already yields duration and resolution — no extra work at
    // scan time. They let the player tell in advance whether a clip will be
    // stream-copied or transcoded, which decides whether re-requesting the
    // stream at a new size means anything (a stream copy cannot be scaled, so
    // for those it is a pipeline restart that buys nothing).
    //
    // `default` so caches written before these existed still load; a clip
    // without them simply cannot be identified as remuxable and keeps the
    // older behaviour until it is rescanned.
    #[serde(default)]
    pub video_codec: Option<String>,
    #[serde(default)]
    pub audio_codec: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClipScanProgress {
    pub files_scanned: u32,
}

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "webm", "avi", "mov", "wmv", "flv", "mpg", "mpeg", "ts", "m2ts",
];
const THUMB_SEEK_SECS: f64 = 0.5;
/// Threads a single thumbnail decode may use; see scan_clip_folder's pool.
const THUMB_DECODE_THREADS: usize = 2;
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
    // Cap the decoder's own threads. Left to itself ffmpeg will happily use
    // every core for a single file — measured at 570% CPU for one 4K AV1
    // thumbnail — which is fine alone and ruinous in parallel; see
    // scan_clip_folder's pool.
    cmd.args(["-threads", &THUMB_DECODE_THREADS.to_string()])
        .args(["-n", "-ss", &format!("{:.3}", THUMB_SEEK_SECS)])
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
        video_codec: info.video_codec,
        audio_codec: info.audio_codec,
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

    // Bounded on purpose. `par_iter` on the global pool runs one task per
    // core, and each task is an ffmpeg that is itself multi-threaded: twelve
    // cores' worth of workers each asking for roughly six cores' worth of
    // decoding is a machine that stops responding, which is exactly what a
    // scan of 4K sources did. Workers times THUMB_DECODE_THREADS is kept under
    // the core count so the desktop still has something to run on.
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .div_ceil(3)
        .clamp(1, 4);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .ok();

    let scan = || paths
        .par_iter()
        .filter_map(|path| {
            let result = probe_and_thumbnail(&app_ref, path, thumbs_dir);
            let n = cnt.fetch_add(1, Ordering::Relaxed) + 1;
            // Emitted more often than the audio scanner's every-200 — a per-file
            // ffmpeg spawn is much heavier than an in-process tag read, so scans
            // are smaller/slower and users benefit from finer-grained feedback.
            if n.is_multiple_of(10) || n == total {
                app_ref.emit("clip:scan:progress", ClipScanProgress { files_scanned: n }).ok();
            }
            result
        })
        .collect::<Vec<Clip>>();

    let mut clips = match &pool {
        Some(pool) => pool.install(scan),
        // A pool that failed to build is not a reason to refuse to scan; the
        // global one is oversubscribed, not broken.
        None => scan(),
    };

    clips.sort_by(|a, b| a.title.cmp(&b.title));
    clips
}
