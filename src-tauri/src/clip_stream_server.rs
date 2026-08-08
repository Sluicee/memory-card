use std::collections::HashMap;
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use crate::ffmpeg_util::{build_ffmpeg_command, probe_file};

pub struct ClipStreamServer {
    pub port: u16,
    pub token: String,
}

/// Kills the ffmpeg child once the HTTP response is done streaming (client
/// disconnect, clip end, or seek-triggered reload). `kill()` alone leaves a
/// zombie in the process table until the parent exits — same fix as
/// ffmpeg_source.rs.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts a local HTTP server on 127.0.0.1 (OS-assigned port) that serves
/// clip files to a native `<video>` element, either stream-copied (remuxed)
/// or live-transcoded to VP9/Opus WebM depending on whether the source
/// codec is something the requesting webview says it can actually play —
/// see `is_remux_safe`.
///
/// This replaced an earlier raw-frame-over-IPC pipeline (ffmpeg -> RGBA ->
/// Tauri Channel -> WebGL2 canvas): that architecture copies every decoded
/// frame through several process/IPC boundaries at full resolution, which
/// hits a real bandwidth wall well before actual screen/source resolution
/// (measured: ffmpeg + WebKitWebProcess + this app's own backend thread all
/// pegged simultaneously once frames grew past ~1920x1440, despite the same
/// source decoding at 100+ fps standalone with nowhere near that output
/// size). A compressed stream doesn't have that problem, and when the
/// source is already in a codec the webview can decode itself, a plain
/// remux (stream copy) is nearly free — no re-encode at all.
///
/// Seeking is handled by the frontend requesting a *fresh* stream with a
/// `seek` offset (killing the previous ffmpeg process) rather than real
/// HTTP Range support — a live transcode has no stable byte-to-time
/// mapping to serve Range requests against, and remux doesn't get real
/// Range support either (see spawn_stream's seek comment).
pub fn start(app: AppHandle) -> Arc<ClipStreamServer> {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("failed to bind clip stream server");
    let port = server.server_addr().to_ip().expect("clip stream server must bind to an IP").port();
    let token = generate_token();

    eprintln!("clip_stream_server: listening on 127.0.0.1:{}", port);
    let info = Arc::new(ClipStreamServer { port, token: token.clone() });

    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let app = app.clone();
            let token = token.clone();
            std::thread::spawn(move || handle_request(&app, &token, request));
        }
    });

    info
}

fn handle_request(app: &AppHandle, expected_token: &str, request: tiny_http::Request) {
    let params = parse_query(request.url());

    if params.get("token").map(String::as_str) != Some(expected_token) {
        let _ = request.respond(tiny_http::Response::empty(403));
        return;
    }

    let Some(path) = params.get("path") else {
        let _ = request.respond(tiny_http::Response::empty(400));
        return;
    };

    if !std::path::Path::new(path).exists() {
        let _ = request.respond(tiny_http::Response::empty(404));
        return;
    }

    let seek_secs: f64 = params
        .get("seek")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
        .max(0.0);

    // Target box for the *transcode* path only — a remux can't be scaled
    // (stream copy means no filtering), so it always sends the source's
    // own resolution. That's fine: compressed-video bandwidth scales with
    // bitrate, not raw pixel count the way the old canvas pipeline's
    // bandwidth did, and letting the browser's own decode+display handle
    // downscaling is exactly how a normal video player does it.
    let w: u32 = params.get("w").and_then(|s| s.parse().ok()).unwrap_or(1280);
    let h: u32 = params.get("h").and_then(|s| s.parse().ok()).unwrap_or(720);

    // Codec family names (ffmpeg's naming: "vp9", "av1", ...) the
    // requesting webview has already confirmed via HTMLMediaElement's
    // `canPlayType()` that it can decode natively — see clips.ts's
    // getPlayableVideoCodecs. Determined client-side because "can this
    // engine actually play this codec" is exactly what that API answers,
    // correctly and portably, without us needing separate Linux
    // (GStreamer plugin presence) vs Windows (WebView2/Media Foundation)
    // detection logic on the backend.
    let playable: Vec<String> = params
        .get("playable")
        .map(|s| s.split(',').map(|c| c.to_lowercase()).collect())
        .unwrap_or_default();

    let mut child = match spawn_stream(app, path, seek_secs, w, h, &playable) {
        Ok(c) => ChildGuard(c),
        Err(e) => {
            eprintln!("clip_stream_server: failed to spawn ffmpeg: {}", e);
            let _ = request.respond(tiny_http::Response::empty(500));
            return;
        }
    };

    let Some(stdout) = child.0.stdout.take() else {
        let _ = request.respond(tiny_http::Response::empty(500));
        return;
    };

    let content_type = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"video/webm"[..]).unwrap();
    let response = tiny_http::Response::new(200.into(), vec![content_type], stdout, None, None);

    // Blocks until the client disconnects (or the stream ends on its own)
    // — the response streams straight from ffmpeg's stdout. Dropping
    // `child` afterward (ChildGuard) kills+reaps it either way.
    let _ = request.respond(response);
}

/// True when the source's own video/audio codecs need no re-encoding: the
/// video codec is one the requesting webview already confirmed it can
/// decode, and the audio is Opus (or absent) — WebM's audio side isn't
/// gated by `playable` since Opus support has been reliable everywhere
/// this app has actually been tested, unlike video codecs.
fn is_remux_safe(probe: &crate::ffmpeg_util::ProbeInfo, playable: &[String]) -> bool {
    let video_ok = probe
        .video_codec
        .as_deref()
        .map(|c| playable.iter().any(|p| p == &c.to_lowercase()))
        .unwrap_or(false);
    let audio_ok = matches!(probe.audio_codec.as_deref(), Some("opus") | None);
    video_ok && audio_ok
}

fn spawn_stream(
    app: &AppHandle,
    path: &str,
    seek_secs: f64,
    w: u32,
    h: u32,
    playable: &[String],
) -> Result<Child, String> {
    let probe = probe_file(app, path);
    let remux = is_remux_safe(&probe, playable);

    let mut cmd = build_ffmpeg_command(app)?;

    // Two-stage seek, same reasoning as the old clip_video.rs: a coarse
    // pre-input `-ss` gets close fast (keyframe-snapped), then a fine
    // post-input `-ss` decodes-and-discards the remainder for an exact
    // start point instead of jumping to the nearest keyframe. For remux
    // (stream copy) this fine seek can only land on the nearest keyframe
    // too — copy mode can't decode-and-discard — but that's an accepted,
    // inherent limitation of not re-encoding, not a bug: still bounded to
    // within ~2s of the requested position.
    let coarse_seek = (seek_secs - 2.0).max(0.0);
    let fine_seek = seek_secs - coarse_seek;

    if coarse_seek > 0.0 {
        cmd.args(["-ss", &format!("{:.3}", coarse_seek)]);
    }
    cmd.arg("-i").arg(path);
    if fine_seek > 0.0 {
        cmd.args(["-ss", &format!("{:.3}", fine_seek)]);
    }

    cmd.args(["-map", "0:v:0", "-map", "0:a:0?"]);

    if remux {
        cmd.args(["-c:v", "copy", "-c:a", "copy"]);
    } else {
        let vf = format!(
            "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black"
        );
        cmd.args(["-vf", &vf])
            .args(["-c:v", "libvpx-vp9", "-deadline", "realtime", "-cpu-used", "5"])
            .args(["-crf", "32", "-b:v", "0"])
            .args(["-c:a", "libopus", "-b:a", "128k"]);
    }

    cmd.args(["-f", "webm"]).arg("pipe:1");

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg (clip stream): {}", e))
}

fn parse_query(url: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(idx) = url.find('?') {
        for pair in url[idx + 1..].split('&') {
            let mut it = pair.splitn(2, '=');
            let k = it.next().unwrap_or("");
            let v = it.next().unwrap_or("");
            map.insert(percent_decode(k), percent_decode(v));
        }
    }
    map
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        // The frontend builds query strings with URLSearchParams, which
        // serializes as application/x-www-form-urlencoded — spaces become
        // literal `+`, not `%20`. A real `+` in a path would itself have
        // been escaped to `%2B` by the encoder, so any raw `+` seen here
        // unambiguously means space.
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Not cryptographically strong — just enough that another local process
/// can't casually guess the URL. The real access boundary is binding to
/// 127.0.0.1 (no remote reachability at all).
fn generate_token() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let pid = std::process::id() as u128;
    let stack_addr = &nanos as *const _ as u128;

    let mut x = nanos ^ pid.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ stack_addr.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let mut out = String::with_capacity(32);
    for _ in 0..32 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push(char::from_digit((x % 16) as u32, 16).unwrap());
    }
    out
}
