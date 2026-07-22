use rodio::Source;
use rtrb::{Consumer, RingBuffer};
use std::io::{BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

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

impl FFmpegSource {
    pub fn new(app: &AppHandle, path: &str, seek_secs: f64) -> Result<Self, String> {
        let seek_str = format!("{:.3}", seek_secs);

        // Configure arguments
        let sidecar_args = vec![
            "-f",
            "f32le",
            "-ac",
            "2",
            "-ar",
            "44100",
            "-af",
            "volume=0.90,alimiter=limit=0.95",
            "-vn",
            "-sn",
            "-map_metadata",
            "-1",
        ];

        // 1. Try to find the organized production binary in bin/ffmpeg[.exe]
        // 2. Fallback to standard sidecar (for Dev mode or standard installs)
        use tauri::Manager;
        let exe_dir = app.path().executable_dir().ok();
        let bin_ffmpeg = exe_dir.as_ref().map(|d| d.join("bin").join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX)));

        let mut std_command: Command = if bin_ffmpeg.as_ref().map(|p| p.exists()).unwrap_or(false) {
            let mut cmd = Command::new(bin_ffmpeg.unwrap());
            if path == "prewarm" {
                cmd.arg("-version");
            } else {
                cmd.args(["-ss", &seek_str])
                    .args(["-i", path])
                    .args(sidecar_args)
                    .arg("pipe:1");
            }
            cmd
        } else {
            let sidecar_command = app
                .shell()
                .sidecar("ffmpeg")
                .map_err(|e| format!("Failed to create sidecar command: {}", e))?;

            let command = if path == "prewarm" {
                sidecar_command.arg("-version")
            } else {
                sidecar_command
                    .args(["-ss", &seek_str])
                    .args(["-i", path])
                    .args(sidecar_args)
                    .arg("pipe:1")
            };
            command.into()
        };

        let mut child = std_command
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
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
            std::thread::sleep(Duration::from_millis(5));
            attempts += 1;
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
    }
}
