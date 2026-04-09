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
}

impl FFmpegSource {
    pub fn new(app: &AppHandle, path: &str, seek_secs: f64) -> Result<Self, String> {
        let seek_str = format!("{:.3}", seek_secs);

        // Use tauri-plugin-shell to get the sidecar command
        let sidecar_command = app
            .shell()
            .sidecar("ffmpeg")
            .map_err(|e| format!("Failed to create sidecar command: {}", e))?;

        // Configure arguments
        let sidecar_args = vec![
            "-f",
            "f32le",
            "-ac",
            "2",
            "-ar",
            "48000",
            "-af",
            "volume=0.95",
            "-vn",
            "-sn",
            "-map_metadata",
            "-1",
        ];

        // Special case for pre-warming the sidecar: just run -version and exit
        let command;
        if path == "prewarm" {
            command = sidecar_command.arg("-version");
        } else {
            command = sidecar_command
                .args(["-ss", &seek_str])
                .args(["-i", path])
                .args(sidecar_args)
                .arg("pipe:1");
        }

        let mut std_command: Command = command.into();

        let mut child = std_command
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

        let stdout = child.stdout.take().ok_or("Failed to open stdout")?;

        // Create a ring buffer for 0.5 seconds of audio (48000 samples for 2 channels)
        // This decouples the blocking pipe-read from the rodio pull-method.
        let (mut producer, consumer) = RingBuffer::new(48000);

        // Spawn a background thread to fill the buffer
        std::thread::spawn(move || {
            let mut reader = BufReader::with_capacity(64 * 1024, stdout);
            let mut buf = [0u8; 4];

            while reader.read_exact(&mut buf).is_ok() {
                let sample = f32::from_le_bytes(buf);

                // Keep trying to push to the ring buffer.
                // If it's full, wait a tiny bit to avoid CPU spin.
                while producer.push(sample).is_err() {
                    // Stop if the consumer side is dropped (e.g. track stopped)
                    if producer.is_abandoned() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        });

        // Preroll: Wait until we have at least 2048 samples (~20ms) before returning.
        // This ensures the audio thread has data immediately, preventing a "start-up click".
        let mut attempts = 0;
        while consumer.slots() < 2048 && attempts < 50 {
            std::thread::sleep(Duration::from_millis(5));
            attempts += 1;
        }

        Ok(Self {
            _child: child,
            consumer,
            sample_rate: 48000,
            channels: 2,
        })
    }
}

impl Iterator for FFmpegSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        // Non-blocking pop.
        // If FFmpeg is slow to start, we return 0.0 (silence) to keep the stream alive.
        // This is THE key to eliminating pops/clicks on Windows.
        match self.consumer.pop() {
            Ok(sample) => Some(sample),
            Err(_) => {
                // If the producer is gone, it means FFmpeg finished or crashed.
                if self.consumer.is_abandoned() {
                    None
                } else {
                    // Buffer is empty but FFmpeg is still running. Play silence.
                    Some(0.0)
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
