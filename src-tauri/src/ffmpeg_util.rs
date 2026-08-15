use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use tauri::AppHandle;

/// Resolves ffmpeg to either the bundled production binary (`<exe_dir>/bin/ffmpeg[.exe]`)
/// or falls back to the Tauri sidecar (dev mode / standard installs).
pub fn build_ffmpeg_command(app: &AppHandle) -> Result<Command, String> {
    use tauri::Manager;
    use tauri_plugin_shell::ShellExt;

    let exe_dir = app.path().executable_dir().ok();
    let bin_ffmpeg = exe_dir
        .as_ref()
        .map(|d| d.join("bin").join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX)));

    if bin_ffmpeg.as_ref().map(|p| p.exists()).unwrap_or(false) {
        Ok(Command::new(bin_ffmpeg.unwrap()))
    } else {
        let sidecar_command = app
            .shell()
            .sidecar("ffmpeg")
            .map_err(|e| format!("Failed to create sidecar command: {}", e))?;
        Ok(sidecar_command.into())
    }
}

#[derive(Debug, Default, Clone)]
pub struct ProbeInfo {
    pub duration_secs: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
}

/// Probe results, keyed by path + mtime + size so an edited file re-probes.
///
/// A probe costs a whole ffmpeg process spawn plus waiting for it to exit, and
/// the clip player asks for the same file repeatedly — once per open and again
/// on every seek, since a seek re-requests the stream. Caching turns all but
/// the first of those into a map lookup.
static PROBE_CACHE: Mutex<Option<HashMap<ProbeKey, ProbeInfo>>> = Mutex::new(None);

#[derive(PartialEq, Eq, Hash, Clone)]
struct ProbeKey {
    path: String,
    mtime: Option<u64>,
    len: Option<u64>,
}

fn probe_key(path: &str) -> ProbeKey {
    let meta = std::fs::metadata(path).ok();
    ProbeKey {
        path: path.to_string(),
        mtime: meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs()),
        len: meta.as_ref().map(|m| m.len()),
    }
}

/// Runs `ffmpeg -i <path>` just to capture its header-analysis stderr, with
/// no output file (errors out immediately after printing it) — the same
/// zero-extra-dependency probing trick as `parse_probe_stderr`, just doing
/// the spawn+capture itself rather than parsing stderr someone else already
/// captured. Cached; see PROBE_CACHE.
pub fn probe_file(app: &AppHandle, path: &str) -> ProbeInfo {
    let key = probe_key(path);
    if let Ok(guard) = PROBE_CACHE.lock() {
        if let Some(hit) = guard.as_ref().and_then(|m| m.get(&key)) {
            return hit.clone();
        }
    }

    let info = probe_file_uncached(app, path);

    // Only cache a probe that found something. A failed spawn returns the
    // default, and caching that would make one transient failure permanent for
    // the rest of the session.
    if info.video_codec.is_some() || info.duration_secs.is_some() {
        if let Ok(mut guard) = PROBE_CACHE.lock() {
            guard.get_or_insert_with(HashMap::new).insert(key, info.clone());
        }
    }
    info
}

fn probe_file_uncached(app: &AppHandle, path: &str) -> ProbeInfo {
    let Ok(mut cmd) = build_ffmpeg_command(app) else { return ProbeInfo::default() };
    cmd.args(["-hide_banner", "-i", path]);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let Ok(child) = cmd.spawn() else { return ProbeInfo::default() };
    let Ok(output) = child.wait_with_output() else { return ProbeInfo::default() };
    parse_probe_stderr(&String::from_utf8_lossy(&output.stderr))
}

/// Parses `Duration: HH:MM:SS.ss` and the first `Video: ..., WxH, ... fps` line out of
/// ffmpeg's stderr header-analysis output — the same text `ffmpeg -i <path>` prints
/// before erroring with "At least one output file must be specified". Used as a
/// zero-extra-dependency substitute for ffprobe, which isn't bundled.
pub fn parse_probe_stderr(stderr: &str) -> ProbeInfo {
    let mut info = ProbeInfo::default();

    for line in stderr.lines() {
        let trimmed = line.trim();

        if info.duration_secs.is_none() {
            if let Some(rest) = trimmed.strip_prefix("Duration:") {
                let ts = rest.trim().split(',').next().unwrap_or("").trim();
                if ts != "N/A" {
                    info.duration_secs = parse_timestamp(ts);
                }
            }
        }

        // Only take the FIRST "Video:" stream line — that's the input's own
        // description. A later ffmpeg invocation with an output (e.g. the
        // clip scanner's thumbnail extraction) prints a second "Video:" line
        // for the *output* stream (already scaled to the thumbnail size),
        // which must not overwrite the source resolution.
        if info.width.is_none() && trimmed.starts_with("Stream") && trimmed.contains("Video:") {
            if let Some((w, h)) = parse_resolution(trimmed) {
                info.width = Some(w);
                info.height = Some(h);
            }
            info.fps = parse_fps(trimmed);
            info.video_codec = parse_codec_name(trimmed, "Video: ");
        }

        if info.audio_codec.is_none() && trimmed.starts_with("Stream") && trimmed.contains("Audio:") {
            info.audio_codec = parse_codec_name(trimmed, "Audio: ");
        }
    }

    info
}

/// Pulls the bare codec name (e.g. "av1", "vp9", "opus") out of a
/// `Stream #0:0: Video: av1 (libdav1d) (Main), ...` / `Audio: opus, ...`
/// line — the token right after the marker, up to the first space, comma,
/// or parenthesis (profile/decoder-name annotations that follow it).
fn parse_codec_name(line: &str, marker: &str) -> Option<String> {
    let idx = line.find(marker)?;
    let rest = &line[idx + marker.len()..];
    let token = rest.split([',', ' ', '(']).next()?.trim();
    if token.is_empty() { None } else { Some(token.to_string()) }
}

fn parse_timestamp(ts: &str) -> Option<f64> {
    let parts: Vec<&str> = ts.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: f64 = parts[0].parse().ok()?;
    let minutes: f64 = parts[1].parse().ok()?;
    let seconds: f64 = parts[2].parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

fn parse_resolution(line: &str) -> Option<(u32, u32)> {
    // Look for a token like "1920x1080" among comma-separated fields.
    for field in line.split(',') {
        let field = field.trim();
        if let Some(idx) = field.find('x') {
            let (w_str, h_str) = (&field[..idx], &field[idx + 1..]);
            // h_str may have trailing junk like "1080 [SAR 1:1 DAR 16:9]"
            let h_str = h_str.split_whitespace().next().unwrap_or(h_str);
            if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) {
                if w > 0 && h > 0 {
                    return Some((w, h));
                }
            }
        }
    }
    None
}

fn parse_fps(line: &str) -> Option<f64> {
    for field in line.split(',') {
        let field = field.trim();
        if let Some(rest) = field.strip_suffix("fps") {
            if let Ok(fps) = rest.trim().parse::<f64>() {
                return Some(fps);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_probe_stderr_typical() {
        let stderr = "Input #0, matroska,webm, from 'clip.mkv':\n  Duration: 00:03:41.25, start: 0.000000, bitrate: 5432 kb/s\n  Stream #0:0: Video: h264 (High), yuv420p(progressive), 1920x1080 [SAR 1:1 DAR 16:9], 30 fps, 30 tbr, 1k tbn\n  Stream #0:1: Audio: aac (LC), 44100 Hz, stereo, fltp, 128 kb/s\n";
        let info = parse_probe_stderr(stderr);
        assert_eq!(info.duration_secs, Some(221.25));
        assert_eq!(info.width, Some(1920));
        assert_eq!(info.height, Some(1080));
        assert_eq!(info.fps, Some(30.0));
    }

    #[test]
    fn test_parse_probe_stderr_unknown_duration() {
        let stderr = "Duration: N/A, bitrate: N/A\n  Stream #0:0: Video: vp9, yuv420p, 640x360, 24 fps\n";
        let info = parse_probe_stderr(stderr);
        assert_eq!(info.duration_secs, None);
        assert_eq!(info.width, Some(640));
        assert_eq!(info.height, Some(360));
    }

    /// Regression test: a thumbnail-extraction invocation prints a second
    /// "Video:" line for the *output* (already scaled) stream. The parser
    /// must keep the first (input/source) resolution, not the last one.
    #[test]
    fn test_parse_probe_stderr_ignores_output_stream_resolution() {
        let stderr = "Input #0, matroska,webm, from 'clip.mkv':\n  Duration: 00:02:00.00, start: 0.000000, bitrate: 8000 kb/s\n  Stream #0:0: Video: av1 (libdav1d) (Main), yuv420p(tv, bt709), 2560x1440 [SAR 1:1 DAR 16:9], 25 fps, 25 tbr, 1k tbn (default)\n  Stream #0:1(eng): Audio: opus, 48000 Hz, stereo, fltp (default)\nStream mapping:\n  Stream #0:0 -> #0:0 (av1 (libdav1d) -> mjpeg (native))\n  Stream #0:0: Video: mjpeg, yuv420p(pc, bt709, progressive), 480x270 [SAR 1:1 DAR 16:9], q=2-31, 200 kb/s, 25 fps, 25 tbn (default)\n";
        let info = parse_probe_stderr(stderr);
        assert_eq!(info.duration_secs, Some(120.0));
        assert_eq!(info.width, Some(2560));
        assert_eq!(info.height, Some(1440));
        assert_eq!(info.fps, Some(25.0));
    }
}
