use rodio::Source;
use rtrb::{Consumer, RingBuffer};
use std::io::{BufReader, Read};
use std::process::{Child, Stdio};
use std::time::Duration;
use tauri::AppHandle;

use crate::audio::EqualizerSettings;
use crate::ffmpeg_util::build_ffmpeg_command;

pub struct FFmpegSource {
    _child: Child, // Keep it to kill on drop
    consumer: Consumer<f32>,
    sample_rate: u32,
    channels: u16,
    // Counts consecutive silence samples returned when the ring buffer is empty.
    // If FFmpeg stalls (hangs without exiting), we treat it as end-of-stream after
    // ~2 seconds of silence so the track can advance normally.
    silence_count: u32,
}

pub fn build_af_filter_string(eq: &EqualizerSettings) -> String {
    let preamp_factor = 0.90 * 10.0f32.powf(eq.preamp.clamp(-12.0, 12.0) / 20.0);
    let mut af_parts = vec![format!("volume={:.4}", preamp_factor)];

    let freqs = [31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
    for (i, &f) in freqs.iter().enumerate() {
        let gain = eq.gains.get(i).copied().unwrap_or(0.0).clamp(-12.0, 12.0);
        if gain.abs() > 0.01 {
            af_parts.push(format!("equalizer=f={:.2}:width_type=q:width=1.41:g={:.2}", f, gain));
        }
    }

    // Auto-gain limiter to prevent digital clipping
    af_parts.push("alimiter=limit=0.95".to_string());
    af_parts.join(",")
}

impl FFmpegSource {
    pub fn new(app: &AppHandle, path: &str, seek_secs: f64, eq: &EqualizerSettings) -> Result<Self, String> {
        if path != "prewarm" {
            let p = std::path::Path::new(path);
            if !p.exists() {
                return Err(format!("File does not exist: {}", path));
            }
            if !p.is_file() {
                return Err(format!("Path is not a file: {}", path));
            }
        }

        let seek_str = format!("{:.3}", seek_secs);
        let af_string = build_af_filter_string(eq);

        // Configure arguments
        let sidecar_args = vec![
            "-f",
            "f32le",
            "-ac",
            "2",
            "-ar",
            "44100",
            "-af",
            &af_string,
            "-vn",
            "-sn",
            "-map_metadata",
            "-1",
        ];

        let mut std_command = build_ffmpeg_command(app)?;
        if path == "prewarm" {
            std_command.arg("-version");
        } else {
            std_command
                .args(["-ss", &seek_str])
                .args(["-i", path])
                .args(sidecar_args)
                .arg("pipe:1");
        }

        let mut child = std_command
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

        let stdout = child.stdout.take().ok_or("Failed to open stdout")?;

        // Create a ring buffer for 2.0 seconds of audio (192000 samples for 2 channels)
        // This decouples the blocking pipe-read from the rodio pull-method.
        let (mut producer, consumer) = RingBuffer::new(192000);

        // Spawn a background thread to fill the buffer
        std::thread::spawn(move || {
            let mut reader = BufReader::with_capacity(64 * 1024, stdout);
            let mut chunk = [0u8; 4096]; // 1024 samples

            // Read in large chunks as long as we can fill them
            while reader.read_exact(&mut chunk).is_ok() {
                for i in 0..1024 {
                    let mut sample_bytes = [0u8; 4];
                    sample_bytes.copy_from_slice(&chunk[i * 4..i * 4 + 4]);
                    let sample = f32::from_le_bytes(sample_bytes).clamp(-1.0, 1.0);

                    while producer.push(sample).is_err() {
                        if producer.is_abandoned() {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(15));
                    }
                }
            }

            // Read the remaining bytes (less than 4096) at the end of the file
            let mut small_buf = [0u8; 4];
            while reader.read_exact(&mut small_buf).is_ok() {
                let sample = f32::from_le_bytes(small_buf).clamp(-1.0, 1.0);
                while producer.push(sample).is_err() {
                    if producer.is_abandoned() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
            }
        });

        // Preroll: Wait until we have at least 44100 samples (~500ms) before returning.
        // This ensures the audio thread has data immediately, preventing a "start-up click" or quick starvation.
        let mut attempts = 0;
        while consumer.slots() < 44100 && attempts < 150 {
            if consumer.is_abandoned() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
            attempts += 1;
        }

        if consumer.slots() == 0 && consumer.is_abandoned() {
            return Err("Failed to decode any audio samples (file empty or unsupported format)".to_string());
        }

        Ok(Self {
            _child: child,
            consumer,
            sample_rate: 44100,
            channels: 2,
            silence_count: 0,
        })
    }
}

impl Iterator for FFmpegSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let mut attempts = 0;
        loop {
            match self.consumer.pop() {
                Ok(sample) => {
                    self.silence_count = 0;
                    return Some(sample);
                }
                Err(_) => {
                    // If the producer is abandoned, it means FFmpeg finished or crashed.
                    if self.consumer.is_abandoned() {
                        return None;
                    }
                    
                    // Buffer is empty but FFmpeg is still running.
                    // Wait a tiny bit (up to 10 attempts * 250us = 2.5ms) before playing silence.
                    // This mitigates CPU spikes/thread scheduling delays without causing xruns.
                    if attempts < 10 {
                        std::thread::sleep(Duration::from_micros(250));
                        attempts += 1;
                        continue;
                    }
                    
                    if self.silence_count == 0 {
                        println!("DEBUG: FFmpegSource buffer starved! Playing silence.");
                    }
                    
                    // Guard against FFmpeg hanging: if we've emitted more than ~2 seconds
                    // of silence (96_000 samples at 48kHz stereo), treat it as end-of-stream.
                    self.silence_count += 1;
                    if self.silence_count > 96_000 {
                        return None;
                    }
                    return Some(0.0);
                }
            }
        }
    }
}

impl Source for FFmpegSource {
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
        None
    }
}

impl Drop for FFmpegSource {
    fn drop(&mut self) {
        let _ = self._child.kill();
        // See ChildGuard::drop in clip_video.rs — kill() alone leaves a zombie.
        let _ = self._child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equalizer_flat_filter_string() {
        let eq = EqualizerSettings::default();
        let af = build_af_filter_string(&eq);
        assert_eq!(af, "volume=0.9000,alimiter=limit=0.95");
    }

    #[test]
    fn test_equalizer_custom_gains_filter_string() {
        let eq = EqualizerSettings {
            enabled: true,
            preamp: 3.0,
            gains: vec![6.0, 0.0, -3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 5.0],
        };
        let af = build_af_filter_string(&eq);
        assert!(af.contains("volume=1.2713"));
        assert!(af.contains("equalizer=f=31.25:width_type=q:width=1.41:g=6.00"));
        assert!(af.contains("equalizer=f=125.00:width_type=q:width=1.41:g=-3.00"));
        assert!(af.contains("equalizer=f=16000.00:width_type=q:width=1.41:g=5.00"));
        assert!(af.contains("alimiter=limit=0.95"));
    }
}
