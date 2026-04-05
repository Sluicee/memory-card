use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::panic;

// Commands sent to the audio thread
enum Cmd {
    Play { path: String, duration: f64 },
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
    // position tracking
    play_started_at: Option<Instant>,
    elapsed_before_pause: f64,
}

impl Inner {
    fn new() -> Self {
        Inner {
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
        let running = self.play_started_at
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

// AudioPlayer lives in the main thread and is Send+Sync:
// - Sender<Cmd> is Send
// - Arc<Mutex<Inner>> is Send+Sync
pub struct AudioPlayer {
    tx: Sender<Cmd>,
    state: Arc<Mutex<Inner>>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        let state = Arc::new(Mutex::new(Inner::new()));
        let state_thread = Arc::clone(&state);

        std::thread::spawn(move || {
            // OutputStream must stay alive for audio to play
            let Ok((_stream, handle)) = OutputStream::try_default() else {
                return;
            };
            let mut sink: Option<Sink> = None;

            loop {
                // Poll every 500ms to detect natural track end
                let cmd = match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(cmd) => Some(cmd),
                    Err(RecvTimeoutError::Timeout) => {
                        // Check if track finished naturally
                        if let Some(s) = &sink {
                            if s.empty() {
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
                        if let Some(s) = sink.take() {
                            s.stop();
                        }
                        match File::open(&path) {
                            Ok(file) => {
                                // catch_unwind guards against rodio/symphonia panics on
                                // certain M4A files (rodio 0.20 bug)
                                let buf = BufReader::new(file);
                                let decoder_result = panic::catch_unwind(|| Decoder::new(buf));
                                let source = match decoder_result {
                                    Ok(Ok(s)) => Some(s),
                                    Ok(Err(e)) => { eprintln!("Decoder error: {e}"); None }
                                    Err(_)     => { eprintln!("Decoder panic on: {path}"); None }
                                };
                                if let Some(source) = source {
                                    match Sink::try_new(&handle) {
                                        Ok(s) => {
                                            let vol = state_thread.lock().unwrap().volume;
                                            s.set_volume(vol);
                                            s.append(source);
                                            let mut st = state_thread.lock().unwrap();
                                            st.is_playing = true;
                                            st.is_paused = false;
                                            st.current_path = Some(path);
                                            st.duration_secs = duration;
                                            st.play_started_at = Some(Instant::now());
                                            st.elapsed_before_pause = 0.0;
                                            sink = Some(s);
                                        }
                                        Err(_) => {}
                                    }
                                }
                            },
                            Err(_) => {}
                        }
                    }
                    Cmd::Pause => {
                        if let Some(s) = &sink {
                            s.pause();
                            let mut st = state_thread.lock().unwrap();
                            let running = st.play_started_at
                                .map(|t| t.elapsed().as_secs_f64())
                                .unwrap_or(0.0);
                            st.elapsed_before_pause += running;
                            st.play_started_at = None;
                            st.is_playing = false;
                            st.is_paused = true;
                        }
                    }
                    Cmd::Resume => {
                        if let Some(s) = &sink {
                            s.play();
                            let mut st = state_thread.lock().unwrap();
                            st.play_started_at = Some(Instant::now());
                            st.is_playing = true;
                            st.is_paused = false;
                        }
                    }
                    Cmd::Stop => {
                        if let Some(s) = sink.take() {
                            s.stop();
                        }
                        let mut st = state_thread.lock().unwrap();
                        st.is_playing = false;
                        st.is_paused = false;
                        st.current_path = None;
                        st.duration_secs = 0.0;
                        st.play_started_at = None;
                        st.elapsed_before_pause = 0.0;
                    }
                    Cmd::SetVolume(v) => {
                        let vol = v.clamp(0.0, 1.0);
                        if let Some(s) = &sink {
                            s.set_volume(vol);
                        }
                        state_thread.lock().unwrap().volume = vol;
                    }
                }
            } // loop
        });

        AudioPlayer { tx, state }
    }

    pub fn play(&self, path: &str, duration: f64) -> Result<(), String> {
        self.tx
            .send(Cmd::Play {
                path: path.to_string(),
                duration,
            })
            .map_err(|e| e.to_string())
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

pub fn create_player() -> SharedPlayer {
    Arc::new(AudioPlayer::new())
}
