use rtrb::{Consumer, PopError, PushError, RingBuffer};
use std::io::{BufReader, Read};
use std::process::{Child, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::ipc::{Channel, Response};
use tauri::AppHandle;

use crate::ffmpeg_util::build_ffmpeg_command;

const RING_CAPACITY: usize = 12;
const TARGET_FPS: f64 = 24.0;
const RESYNC_DEADZONE_SECS: f64 = 0.08;

pub struct VideoFrame {
    pub pts: f64,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

enum VideoCmd {
    Start {
        path: String,
        seek_secs: f64,
        max_w: u32,
        max_h: u32,
        channel: Channel<Response>,
    },
    Seek {
        seek_secs: f64,
    },
    Pause,
    Resume,
    Resync {
        audio_pos: f64,
    },
    Stop,
}

pub struct ClipVideoPlayer {
    tx: Sender<VideoCmd>,
}

pub type SharedClipVideo = Arc<ClipVideoPlayer>;

impl ClipVideoPlayer {
    pub fn start(
        &self,
        path: String,
        seek_secs: f64,
        max_w: u32,
        max_h: u32,
        channel: Channel<Response>,
    ) -> Result<(), String> {
        self.tx
            .send(VideoCmd::Start { path, seek_secs, max_w, max_h, channel })
            .map_err(|_| "clip video thread is gone".to_string())
    }

    pub fn seek(&self, seek_secs: f64) {
        let _ = self.tx.send(VideoCmd::Seek { seek_secs });
    }

    pub fn pause(&self) {
        let _ = self.tx.send(VideoCmd::Pause);
    }

    pub fn resume(&self) {
        let _ = self.tx.send(VideoCmd::Resume);
    }

    pub fn resync(&self, audio_pos: f64) {
        let _ = self.tx.send(VideoCmd::Resync { audio_pos });
    }

    pub fn stop(&self) {
        let _ = self.tx.send(VideoCmd::Stop);
    }
}

/// Kills the ffmpeg child on drop. Kept as its own type (rather than a `Drop`
/// impl directly on `ActiveDecode`) so `ActiveDecode` can still be partially
/// destructured on `Seek` — Rust forbids destructuring a type that implements
/// `Drop` itself.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        // kill() only sends the signal — without wait(), the process stays
        // a zombie in the process table until the parent exits. Every
        // Seek/Stop/Start-while-playing hits this path, so over a testing
        // session this accumulated dozens of zombie ffmpeg processes.
        let _ = self.0.wait();
    }
}

struct ActiveDecode {
    child: ChildGuard,
    consumer: Consumer<VideoFrame>,
    channel: Channel<Response>,
    path: String,
    max_w: u32,
    max_h: u32,
    paused: bool,
    // Instant-anchored pacing clock — the same clock-splice technique as
    // `audio.rs`'s `Inner::position()`, but driving *when to send* a decoded
    // frame rather than reporting a position. `elapsed_before_pause` doubles
    // as the seek/resync baseline (absolute seconds into the source file).
    play_started_at: Option<Instant>,
    elapsed_before_pause: f64,
}

impl ActiveDecode {
    fn elapsed(&self) -> f64 {
        self.elapsed_before_pause
            + self
                .play_started_at
                .map(|t| t.elapsed().as_secs_f64())
                .unwrap_or(0.0)
    }
}

pub fn create_clip_video_player(app: AppHandle) -> SharedClipVideo {
    let (tx, rx) = mpsc::channel::<VideoCmd>();

    std::thread::spawn(move || {
        let mut active: Option<ActiveDecode> = None;
        loop {
            match rx.recv_timeout(Duration::from_millis(8)) {
                Ok(cmd) => handle_cmd(&app, cmd, &mut active),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            tick(&mut active);
        }
    });

    Arc::new(ClipVideoPlayer { tx })
}

fn handle_cmd(app: &AppHandle, cmd: VideoCmd, active: &mut Option<ActiveDecode>) {
    match cmd {
        VideoCmd::Start { path, seek_secs, max_w, max_h, channel } => {
            *active = None; // drop old decode first (ChildGuard kills the previous ffmpeg)
            match spawn_decode(app, &path, seek_secs, max_w, max_h) {
                Ok((child, consumer)) => {
                    *active = Some(ActiveDecode {
                        child,
                        consumer,
                        channel,
                        path,
                        max_w,
                        max_h,
                        paused: false,
                        play_started_at: Some(Instant::now()),
                        elapsed_before_pause: seek_secs,
                    });
                }
                Err(e) => eprintln!("clip_video: failed to start decode: {}", e),
            }
        }
        VideoCmd::Seek { seek_secs } => {
            if let Some(state) = active.take() {
                // Partial move: `child`/`consumer` are dropped here (ChildGuard kills ffmpeg).
                let ActiveDecode { channel, path, max_w, max_h, .. } = state;
                match spawn_decode(app, &path, seek_secs, max_w, max_h) {
                    Ok((child, consumer)) => {
                        *active = Some(ActiveDecode {
                            child,
                            consumer,
                            channel,
                            path,
                            max_w,
                            max_h,
                            paused: false,
                            play_started_at: Some(Instant::now()),
                            elapsed_before_pause: seek_secs,
                        });
                    }
                    Err(e) => eprintln!("clip_video: seek failed: {}", e),
                }
            }
        }
        VideoCmd::Pause => {
            if let Some(state) = active {
                if state.play_started_at.is_some() {
                    state.elapsed_before_pause = state.elapsed();
                    state.play_started_at = None;
                }
                state.paused = true;
            }
        }
        VideoCmd::Resume => {
            if let Some(state) = active {
                state.play_started_at = Some(Instant::now());
                state.paused = false;
            }
        }
        VideoCmd::Resync { audio_pos } => {
            // Nudge (hard-snap, not gradual) the pacing clock to the true
            // audio-master-clock position. Frontend calls this periodically
            // with the same `audio_get_position` value it already polls for
            // the progress bar, correcting drift between the two
            // independently-launched, independently-buffered ffmpeg processes.
            //
            // Dead zone: only correct when drift exceeds ~2 frames' worth of
            // time. Snapping on every call regardless of actual drift caused
            // a small but visible jump/stutter every resync interval even
            // when already in sync — most calls found drift well under one
            // frame, so the "correction" itself was the main source of judder.
            if let Some(state) = active {
                let drift = (audio_pos - state.elapsed()).abs();
                if drift > RESYNC_DEADZONE_SECS {
                    state.elapsed_before_pause = audio_pos;
                    state.play_started_at = if state.paused { None } else { Some(Instant::now()) };
                }
            }
        }
        VideoCmd::Stop => {
            *active = None;
        }
    }
}

/// Paces frame delivery against `ActiveDecode`'s wall-clock anchor: sends at
/// most the newest buffered frame whose `pts` has come due, discarding any
/// older ones still sitting in the ring buffer (catch-up / frame-drop), and
/// does nothing if the front frame is still ahead of the clock (the frontend
/// simply keeps displaying whatever it last drew — no explicit "hold"
/// message needed).
fn tick(active: &mut Option<ActiveDecode>) {
    let Some(state) = active else { return };
    if state.paused {
        return;
    }

    let elapsed = state.elapsed();
    let mut due_frame: Option<VideoFrame> = None;

    loop {
        match state.consumer.peek() {
            Ok(frame) if frame.pts <= elapsed => match state.consumer.pop() {
                Ok(frame) => due_frame = Some(frame),
                Err(PopError::Empty) => break, // raced with the peek; nothing more to do
            },
            _ => break,
        }
    }

    if let Some(frame) = due_frame {
        let _ = state.channel.send(Response::new(frame.data));
    }
}

fn spawn_decode(
    app: &AppHandle,
    path: &str,
    seek_secs: f64,
    max_w: u32,
    max_h: u32,
) -> Result<(ChildGuard, Consumer<VideoFrame>), String> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(format!("File does not exist: {}", path));
    }

    let mut cmd = build_ffmpeg_command(app)?;

    // Two-stage seek for frame-accurate positioning on long-GOP encodes:
    // a coarse pre-input `-ss` (fast, demuxer-level, keyframe-snapped) gets
    // us to within 2s of the target, then a fine post-input `-ss` decodes
    // and discards the small remainder to land exactly on the requested
    // frame. Bounded to at most ~2s of wasted decode. Audio deliberately
    // keeps its single pre-input seek (ffmpeg_source.rs) — imprecision
    // there is inaudible, but the same imprecision in video is visibly a
    // "wrong frame" jump, which two-stage seeking avoids.
    let coarse_seek = (seek_secs - 2.0).max(0.0);
    let fine_seek = seek_secs - coarse_seek;

    if coarse_seek > 0.0 {
        cmd.args(["-ss", &format!("{:.3}", coarse_seek)]);
    }
    cmd.arg("-i").arg(path);
    if fine_seek > 0.0 {
        cmd.args(["-ss", &format!("{:.3}", fine_seek)]);
    }

    let vf = format!(
        "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black,fps={fps}",
        w = max_w,
        h = max_h,
        fps = TARGET_FPS,
    );

    cmd.args(["-map", "0:v:0", "-an", "-sn"])
        .args(["-vf", &vf])
        .args(["-pix_fmt", "rgba"])
        .args(["-f", "rawvideo"])
        .arg("pipe:1");

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg (video): {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to open ffmpeg stdout")?;
    let frame_bytes = (max_w as usize) * (max_h as usize) * 4;
    let (mut producer, consumer) = RingBuffer::<VideoFrame>::new(RING_CAPACITY);

    std::thread::spawn(move || {
        let mut reader = BufReader::with_capacity(frame_bytes.max(64 * 1024), stdout);
        let mut index: u64 = 0;

        loop {
            let mut buf = vec![0u8; frame_bytes];
            if reader.read_exact(&mut buf).is_err() {
                // EOF or decode error — ffmpeg exited, nothing more to read.
                return;
            }

            // `-ss` seeks the ffmpeg process's own output to start at
            // seek_secs, so frame 0 of *this* process's stdout corresponds to
            // roughly seek_secs into the source file — offset pts so it's
            // directly comparable against `elapsed()`'s absolute clock.
            let mut frame = VideoFrame {
                pts: seek_secs + index as f64 / TARGET_FPS,
                width: max_w,
                height: max_h,
                data: buf,
            };
            index += 1;

            loop {
                match producer.push(frame) {
                    Ok(()) => break,
                    Err(PushError::Full(f)) => {
                        if producer.is_abandoned() {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                        frame = f;
                    }
                }
            }
        }
    });

    Ok((ChildGuard(child), consumer))
}
