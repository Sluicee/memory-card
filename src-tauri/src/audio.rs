use crate::ffmpeg_source::FFmpegSource;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;

enum Cmd {
    Play { path: String, duration: f64 },
    Preload { _path: String },
    Seek { position: f64 },
    Pause,
    Resume,
    Stop,
    SetVolume(f32),
}

#[derive(Debug, Clone)]
struct Inner {
    is_playing: bool,
    is_paused: bool,
    current_path: Option<String>,
    duration_secs: f64,
    volume: f32,
    play_started_at: Option<Instant>,
    elapsed_before_pause: f64,
}

impl Inner {
    fn new() -> Self {
        Self {
            is_playing: false,
            is_paused: false,
            current_path: None,
            duration_secs: 0.0,
            volume: 1.0,
            play_started_at: None,
            elapsed_before_pause: 0.0,
        }
    }

    fn position(&self) -> f64 {
        let running = self
            .play_started_at
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        (self.elapsed_before_pause + running).min(self.duration_secs)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub is_paused: bool,
    pub current_path: Option<String>,
    pub duration_secs: f64,
    pub volume: f32,
    pub position_secs: f64,
}

fn build_sink(
    app_handle: &AppHandle,
    handle: &OutputStreamHandle,
    path: &str,
    volume: f32,
    seek_secs: f64,
) -> Result<Sink, String> {
    let source =
        FFmpegSource::new(app_handle, path, seek_secs)?.fade_in(Duration::from_millis(100)); // Smooth start
    let sink = Sink::try_new(handle).map_err(|e| e.to_string())?;
    sink.set_volume(volume);
    sink.append(source);
    Ok(sink)
}

fn gently_stop(sink: Option<Sink>) {
    if let Some(s) = sink {
        if s.empty() {
            s.stop();
            return;
        }
        // Spawn a thread to fade out the old sink to avoid the "pop"
        std::thread::spawn(move || {
            let start_vol = s.volume();
            let steps = 20;
            for i in (0..steps).rev() {
                s.set_volume(start_vol * (i as f32 / steps as f32));
                std::thread::sleep(Duration::from_millis(5));
            }
            s.stop();
        });
    }
}

pub struct AudioPlayer {
    tx: Sender<Cmd>,
    state: Arc<Mutex<Inner>>,
}

impl AudioPlayer {
    pub fn new(app_handle: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        let state = Arc::new(Mutex::new(Inner::new()));
        let state_thread = Arc::clone(&state);

        std::thread::spawn(move || {
            let Ok((_stream, handle)) = OutputStream::try_default() else {
                return;
            };

            // Pre-warm the sidecar in a SEPARATE thread so it doesn't block the main audio loop.
            // Spawning FFmpeg for the first time on Windows can take 1s+ due to AV scanning.
            let ah = app_handle.clone();
            let h = handle.clone();
            std::thread::spawn(move || {
                let _ = build_sink(&ah, &h, "prewarm", 0.0, 0.0);
            });

            let mut sink: Option<Sink> = None;

            loop {
                let cmd = match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(cmd) => Some(cmd),
                    Err(RecvTimeoutError::Timeout) => {
                        if let Some(current_sink) = &sink {
                            if current_sink.empty() {
                                let mut st = state_thread.lock().unwrap();
                                if st.is_playing {
                                    st.is_playing = false;
                                    st.is_paused = false;
                                }
                            }
                        }
                        None
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                };

                let Some(cmd) = cmd else { continue };

                match cmd {
                    Cmd::Play { path, duration } => {
                        eprintln!("[audio] Play: {path}");
                        gently_stop(sink.take()); // Fade out old track immediately

                        let volume = state_thread.lock().unwrap().volume;

                        match build_sink(&app_handle, &handle, &path, volume, 0.0) {
                            Ok(new_sink) => {
                                sink = Some(new_sink);

                                let mut st = state_thread.lock().unwrap();
                                st.is_playing = true;
                                st.is_paused = false;
                                st.current_path = Some(path);
                                st.duration_secs = duration;
                                st.play_started_at = Some(Instant::now());
                                st.elapsed_before_pause = 0.0;
                            }
                            Err(e) => eprintln!("[audio] Play failed: {e}"),
                        }
                    }
                    Cmd::Preload { _path: _ } => {
                        // Preloading raw pipes is complex; skipping for now to keep it robust.
                    }
                    Cmd::Seek { position } => {
                        gently_stop(sink.take()); // Fade out current track immediately

                        let (path, duration, volume, was_paused) = {
                            let st = state_thread.lock().unwrap();
                            (
                                st.current_path.clone(),
                                st.duration_secs,
                                st.volume,
                                st.is_paused,
                            )
                        };

                        let Some(path) = path else { continue };
                        let target = position.clamp(0.0, duration.max(0.0));

                        match build_sink(&app_handle, &handle, &path, volume, target) {
                            Ok(new_sink) => {
                                if was_paused {
                                    new_sink.pause();
                                }

                                sink = Some(new_sink);

                                let mut st = state_thread.lock().unwrap();
                                st.is_playing = !was_paused;
                                st.is_paused = was_paused;
                                st.current_path = Some(path);
                                st.duration_secs = duration;
                                st.play_started_at = if was_paused {
                                    None
                                } else {
                                    Some(Instant::now())
                                };
                                st.elapsed_before_pause = target;
                                eprintln!("[audio] Seek to {}", target);
                            }
                            Err(e) => eprintln!("[audio] Seek failed: {e}"),
                        }
                    }
                    Cmd::Pause => {
                        if let Some(current_sink) = &sink {
                            current_sink.pause();
                            let mut st = state_thread.lock().unwrap();
                            let running = st
                                .play_started_at
                                .map(|t| t.elapsed().as_secs_f64())
                                .unwrap_or(0.0);
                            st.elapsed_before_pause += running;
                            st.play_started_at = None;
                            st.is_playing = false;
                            st.is_paused = true;
                        }
                    }
                    Cmd::Resume => {
                        if let Some(current_sink) = &sink {
                            current_sink.play();
                            let mut st = state_thread.lock().unwrap();
                            st.play_started_at = Some(Instant::now());
                            st.is_playing = true;
                            st.is_paused = false;
                        }
                    }
                    Cmd::Stop => {
                        gently_stop(sink.take());

                        let mut st = state_thread.lock().unwrap();
                        st.is_playing = false;
                        st.is_paused = false;
                        st.current_path = None;
                        st.duration_secs = 0.0;
                        st.play_started_at = None;
                        st.elapsed_before_pause = 0.0;
                    }
                    Cmd::SetVolume(volume) => {
                        let clamped = volume.clamp(0.0, 1.0);
                        if let Some(current_sink) = &sink {
                            current_sink.set_volume(clamped);
                        }
                        state_thread.lock().unwrap().volume = clamped;
                    }
                }
            }
        });

        Self { tx, state }
    }

    pub fn play(&self, path: &str, duration: f64) -> Result<(), String> {
        self.tx
            .send(Cmd::Play {
                path: path.to_string(),
                duration,
            })
            .map_err(|e| e.to_string())
    }

    pub fn preload(&self, path: &str) {
        let _ = self.tx.send(Cmd::Preload {
            _path: path.to_string(),
        });
    }

    pub fn seek(&self, position: f64) {
        let _ = self.tx.send(Cmd::Seek { position });
    }

    pub fn pause(&self) {
        let _ = self.tx.send(Cmd::Pause);
    }

    pub fn resume(&self) {
        let _ = self.tx.send(Cmd::Resume);
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self.tx.send(Cmd::SetVolume(volume));
    }

    pub fn is_finished(&self) -> bool {
        let st = self.state.lock().unwrap();
        !st.is_playing && !st.is_paused && st.current_path.is_some()
    }

    pub fn get_position(&self) -> f64 {
        self.state.lock().unwrap().position()
    }

    pub fn get_state(&self) -> PlaybackState {
        let st = self.state.lock().unwrap();
        PlaybackState {
            is_playing: st.is_playing,
            is_paused: st.is_paused,
            current_path: st.current_path.clone(),
            duration_secs: st.duration_secs,
            volume: st.volume,
            position_secs: st.position(),
        }
    }
}

pub type SharedPlayer = Arc<AudioPlayer>;

pub fn create_player(app_handle: AppHandle) -> SharedPlayer {
    Arc::new(AudioPlayer::new(app_handle))
}
