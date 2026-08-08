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
// Frames buffered before the pacing clock is allowed to start ticking (see
// `started` on ActiveDecode). ffmpeg startup + the two-stage seek's
// decode-and-discard can take anywhere from ~0 to ~2s before the first
// frame lands in the ring buffer; starting the wall-clock anchor at command
// time regardless meant `elapsed()` was already ahead of every frame's pts
// by the time frames actually arrived, so playback opened (or resumed after
// a seek) with a visible freeze followed by a jump-cut to whatever frame
// was "due" by then instead of a smooth start. Anchoring the clock only
// once a small buffer exists avoids that.
//
// 3 (~125ms at TARGET_FPS) wasn't enough in practice — some clips still
// showed a stutter/skip in the first handful of frames after the clock
// started. ffmpeg's `fps=` filter (which retimes the source to a constant
// TARGET_FPS by duplicating/dropping frames) doesn't necessarily emit its
// first few output frames at a steady TARGET_FPS cadence right out of the
// gate — its internal reference clock is still settling — so a shallow
// buffer could run dry again microseconds after clearing the gate once.
// 8 (~330ms) gives that settling enough room to happen before playback
// clock start, at the cost of a bit more startup latency on top of the
// ffmpeg spawn/seek time (which usually dominates anyway).
const MIN_BUFFER_FRAMES: usize = 8;

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
        generation: u64,
    },
    Seek {
        seek_secs: f64,
        generation: u64,
        max_w: u32,
        max_h: u32,
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
        generation: u64,
    ) -> Result<(), String> {
        self.tx
            .send(VideoCmd::Start { path, seek_secs, max_w, max_h, channel, generation })
            .map_err(|_| "clip video thread is gone".to_string())
    }

    pub fn seek(&self, seek_secs: f64, generation: u64, max_w: u32, max_h: u32) {
        let _ = self.tx.send(VideoCmd::Seek { seek_secs, generation, max_w, max_h });
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
    paused: bool,
    // Instant-anchored pacing clock — the same clock-splice technique as
    // `audio.rs`'s `Inner::position()`, but driving *when to send* a decoded
    // frame rather than reporting a position. `elapsed_before_pause` doubles
    // as the seek/resync baseline (absolute seconds into the source file).
    play_started_at: Option<Instant>,
    elapsed_before_pause: f64,
    // False until `tick()` has seen MIN_BUFFER_FRAMES buffered and anchored
    // `play_started_at` for the first time since the last Start/Seek. While
    // false, `tick()`/Resume/Resync all leave the clock alone — see
    // MIN_BUFFER_FRAMES.
    started: bool,
    // Opaque tag supplied by the frontend on each Start/Seek (its own local
    // request counter — see ClipPlayerView.svelte), prefixed onto every
    // frame's bytes before it's sent over the channel (see tick()).
    // Deliberately NOT an independent counter kept on this side: this
    // background thread is a single long-lived, app-lifetime object
    // spawned once, so a self-incrementing counter here would keep
    // climbing across every clip ever opened in the session, while the
    // frontend's counter resets per `ClipPlayerView` mount (fresh per
    // clip) — the two would only ever agree for the very first clip played
    // after app start. Echoing whatever the frontend sent sidesteps
    // needing the two sides to independently agree on a shared sequence at
    // all.
    //
    // Tagging the frame bytes themselves (rather than a separate "ready"
    // event) matters for two reasons, both observed as real bugs: frames
    // stream continuously during playback, so one from the *previous*
    // position is essentially always still in flight over IPC at the
    // moment a seek is requested — the frontend needs to actually be able
    // to tell that apart from the new one, not just infer readiness from
    // "a frame arrived" (which cleared a seeking indicator instantly while
    // the real seek was still 1-3s out) or from a same-tick-but-separate
    // event (which could reach the frontend before the frame payload
    // itself — revealing the canvas before real pixels for the new
    // position had actually landed).
    generation: u64,
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
        VideoCmd::Start { path, seek_secs, max_w, max_h, channel, generation } => {
            *active = None; // drop old decode first (ChildGuard kills the previous ffmpeg)
            match spawn_decode(app, &path, seek_secs, max_w, max_h) {
                Ok((child, consumer)) => {
                    *active = Some(ActiveDecode {
                        child,
                        consumer,
                        channel,
                        path,
                        paused: false,
                        play_started_at: None,
                        elapsed_before_pause: seek_secs,
                        started: false,
                        generation,
                    });
                }
                Err(e) => eprintln!("clip_video: failed to start decode: {}", e),
            }
        }
        VideoCmd::Seek { seek_secs, generation, max_w, max_h } => {
            if let Some(state) = active.take() {
                // Partial move: `child`/`consumer` are dropped here (ChildGuard kills ffmpeg).
                // max_w/max_h come from the command, NOT the old state — the
                // frontend re-supplies them on every seek (including a
                // same-position "seek" it issues purely to switch decode
                // resolution when entering/leaving fullscreen mid-clip), so
                // reusing the previous decode's size here would silently
                // ignore that.
                let ActiveDecode { channel, path, .. } = state;
                match spawn_decode(app, &path, seek_secs, max_w, max_h) {
                    Ok((child, consumer)) => {
                        *active = Some(ActiveDecode {
                            child,
                            consumer,
                            channel,
                            path,
                            paused: false,
                            play_started_at: None,
                            elapsed_before_pause: seek_secs,
                            started: false,
                            generation,
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
                state.paused = false;
                // If still buffering (started == false), leave play_started_at
                // at None — tick()'s buffering gate will anchor it once
                // MIN_BUFFER_FRAMES is reached, same as a fresh Start/Seek.
                if state.started {
                    state.play_started_at = Some(Instant::now());
                }
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
            //
            // Skipped entirely while still buffering (started == false) —
            // otherwise this would just re-derive a play_started_at from a
            // (possibly stale) audio_pos ahead of any frame actually being
            // ready, reintroducing the same premature-clock problem the
            // buffering gate exists to avoid.
            if let Some(state) = active {
                if state.started {
                    let drift = (audio_pos - state.elapsed()).abs();
                    if drift > RESYNC_DEADZONE_SECS {
                        state.elapsed_before_pause = audio_pos;
                        state.play_started_at = if state.paused { None } else { Some(Instant::now()) };
                    }
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

    if !state.started {
        if state.consumer.slots() < MIN_BUFFER_FRAMES {
            return;
        }
        // Anchor now, with the buffer already primed: elapsed() immediately
        // evaluates to elapsed_before_pause (== the seek target the newest
        // decode was spawned at), which is exactly frame 0's pts — so the
        // loop below finds it due right away instead of waiting for real
        // time to catch up to a clock that had already been running.
        state.play_started_at = Some(Instant::now());
        state.started = true;
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
        // 8-byte little-endian generation prefix ahead of the raw RGBA
        // payload — lets the frontend tell "the first frame of the seek I
        // just requested" apart from "a frame from whatever was playing
        // before, still in flight over IPC" (see `generation` on
        // ActiveDecode). Frames stream continuously during playback, so at
        // the moment a seek is requested there's essentially always one
        // already dispatched from the *previous* position; without a way
        // to tell them apart the frontend has no reliable way to know when
        // the picture it's showing actually belongs to the seek it asked
        // for, and would either clear a "seeking" indicator on a stale
        // frame or reveal the canvas before real pixels for the new
        // position had actually arrived (both were observed bugs — see
        // ClipPlayerView.svelte's channel.onmessage).
        let mut payload = Vec::with_capacity(8 + frame.data.len());
        payload.extend_from_slice(&state.generation.to_le_bytes());
        payload.extend_from_slice(&frame.data);
        let _ = state.channel.send(Response::new(payload));
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
