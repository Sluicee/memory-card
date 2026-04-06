use rodio::{source::SeekError, OutputStream, OutputStreamHandle, Sink, Source};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder as SymphoniaDecoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, SeekedTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

const MAX_DECODE_RETRIES: usize = 3;

enum Cmd {
    Play { path: String, duration: f64 },
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

struct SeekableSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    track_id: u32,
    buffer: Vec<f32>,
    buf_pos: usize,
    channels: u16,
    sample_rate: u32,
    total_duration: Option<Duration>,
    done: bool,
}

impl SeekableSource {
    fn open(path: &str) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = Path::new(path).extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| e.to_string())?;

        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
            .cloned()
            .or_else(|| format.default_track().cloned())
            .ok_or_else(|| "No supported audio track found".to_string())?;

        let track_id = track.id;
        let channels = track
            .codec_params
            .channels
            .map(|c| c.count() as u16)
            .unwrap_or(2);
        let sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);
        let total_duration = track
            .codec_params
            .time_base
            .zip(track.codec_params.n_frames)
            .map(|(base, frames)| time_to_duration(base.calc_time(frames)));

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| e.to_string())?;

        Ok(Self {
            format,
            decoder,
            track_id,
            buffer: Vec::new(),
            buf_pos: 0,
            channels,
            sample_rate,
            total_duration,
            done: false,
        })
    }

    fn decode_packet(&mut self, packet: &symphonia::core::formats::Packet) -> Result<(), SymphoniaError> {
        let decoded = self.decoder.decode(packet)?;
        let spec = *decoded.spec();
        self.channels = spec.channels.count() as u16;
        self.sample_rate = spec.rate;

        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        self.buffer.clear();
        self.buffer.extend_from_slice(buf.samples());
        self.buf_pos = 0;
        Ok(())
    }

    fn fill_buffer(&mut self) {
        self.buffer.clear();
        self.buf_pos = 0;

        while self.buffer.is_empty() && !self.done {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(_) => {
                    self.done = true;
                    break;
                }
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decode_packet(&packet) {
                Ok(()) => {}
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(_) => {
                    self.done = true;
                    break;
                }
            }
        }
    }

    fn refine_position(&mut self, seek_res: SeekedTo) -> Result<(), SeekError> {
        let mut samples_to_skip = seek_res.required_ts.saturating_sub(seek_res.actual_ts);

        let packet = loop {
            let packet = self
                .format
                .next_packet()
                .map_err(|e| SeekError::Other(Box::new(e)))?;

            if packet.track_id() != self.track_id {
                continue;
            }

            if packet.dur() > samples_to_skip {
                break packet;
            }

            samples_to_skip = samples_to_skip.saturating_sub(packet.dur());
        };

        let mut decoded = self.decoder.decode(&packet);
        for _ in 0..MAX_DECODE_RETRIES {
            if decoded.is_ok() {
                break;
            }

            let retry_packet = loop {
                let packet = self
                    .format
                    .next_packet()
                    .map_err(|e| SeekError::Other(Box::new(e)))?;

                if packet.track_id() == self.track_id {
                    break packet;
                }
            };

            decoded = self.decoder.decode(&retry_packet);
        }

        let decoded = decoded.map_err(|e| SeekError::Other(Box::new(e)))?;
        let spec = *decoded.spec();
        self.channels = spec.channels.count() as u16;
        self.sample_rate = spec.rate;

        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        self.buffer.clear();
        self.buffer.extend_from_slice(buf.samples());
        self.buf_pos = samples_to_skip as usize * self.channels as usize;
        self.done = false;
        Ok(())
    }
}

impl Iterator for SeekableSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.buf_pos >= self.buffer.len() {
            if self.done {
                return None;
            }

            self.fill_buffer();
            if self.buffer.is_empty() {
                return None;
            }
        }

        let sample = self.buffer[self.buf_pos];
        self.buf_pos += 1;
        Some(sample)
    }
}

impl Source for SeekableSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        let target = match self.total_duration {
            Some(total) if total.saturating_sub(pos).as_millis() < 1 => {
                skip_back_a_tiny_bit(duration_to_time(total))
            }
            _ => duration_to_time(pos),
        };

        let seek_res = self
            .format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: target,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| SeekError::Other(Box::new(e)))?;

        self.refine_position(seek_res)
    }
}

fn time_to_duration(time: Time) -> Duration {
    Duration::from_secs_f64(time.seconds as f64 + time.frac)
}

fn duration_to_time(duration: Duration) -> Time {
    Time::from(duration.as_secs_f64())
}

fn skip_back_a_tiny_bit(
    Time {
        mut seconds,
        mut frac,
    }: Time,
) -> Time {
    frac -= 0.0001;
    if frac < 0.0 {
        seconds = seconds.saturating_sub(1);
        frac = 1.0 + frac;
    }
    Time { seconds, frac }
}

fn build_sink(
    handle: &OutputStreamHandle,
    path: &str,
    volume: f32,
    seek_secs: f64,
) -> Result<Sink, String> {
    let sink = Sink::try_new(handle).map_err(|e| e.to_string())?;
    sink.set_volume(volume);

    let mut source = SeekableSource::open(path)?;
    if seek_secs > 0.0 {
        source
            .try_seek(Duration::from_secs_f64(seek_secs))
            .map_err(|e| e.to_string())?;
    }

    sink.append(source);
    Ok(sink)
}

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
            let Ok((_stream, handle)) = OutputStream::try_default() else {
                return;
            };

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
                        let volume = state_thread.lock().unwrap().volume;

                        match build_sink(&handle, &path, volume, 0.0) {
                            Ok(new_sink) => {
                                if let Some(old_sink) = sink.replace(new_sink) {
                                    old_sink.stop();
                                }

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
                    Cmd::Seek { position } => {
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

                        let seek_result = sink
                            .as_ref()
                            .ok_or_else(|| "No active sink".to_string())
                            .and_then(|current_sink| {
                                current_sink
                                    .try_seek(Duration::from_secs_f64(target))
                                    .map_err(|e| e.to_string())
                            });

                        match seek_result {
                            Ok(()) => {
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
                            }
                            Err(seek_err) => match build_sink(&handle, &path, volume, target) {
                                Ok(new_sink) => {
                                    if was_paused {
                                        new_sink.pause();
                                    }

                                    if let Some(old_sink) = sink.replace(new_sink) {
                                        old_sink.stop();
                                    }

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
                                    eprintln!(
                                        "[audio] Seek fallback after try_seek failure: {seek_err}"
                                    );
                                }
                                Err(e) => {
                                    eprintln!(
                                        "[audio] Seek failed: {seek_err}; fallback failed: {e}"
                                    )
                                }
                            },
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
                        if let Some(current_sink) = sink.take() {
                            current_sink.stop();
                        }

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

pub fn create_player() -> SharedPlayer {
    Arc::new(AudioPlayer::new())
}
