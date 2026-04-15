use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mp3lame_encoder::{Builder, DualPcm, FlushNoGap, MonoPcm};
use std::fs::File;
use std::io::Write;
use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct AudioCapture {
    stream: cpal::Stream,
    encoder: Arc<Mutex<mp3lame_encoder::Encoder>>,
    file: Arc<Mutex<File>>,
    stop_flag: Arc<AtomicBool>,
    pub path: PathBuf,
    channels: u16,
}

// cpal::Stream contains a raw pointer that is !Send, but we gate all cross-thread
// access through Arc<Mutex<Option<AudioCapture>>> and only touch the stream from
// a single owner at a time, so this is safe.
unsafe impl Send for AudioCapture {}
unsafe impl Sync for AudioCapture {}

impl AudioCapture {
    /// Start capturing system audio (WASAPI loopback) and encoding to MP3.
    /// Audio is written to `path` in real time.
    pub fn start(path: PathBuf) -> Result<Self, String> {
        // Use WASAPI host on Windows
        let host = cpal::host_from_id(cpal::HostId::Wasapi)
            .map_err(|e| format!("WASAPI unavailable: {e}"))?;

        // Default output device — we capture it as loopback input
        let device = host
            .default_output_device()
            .ok_or("No default output device found")?;

        let config = device
            .default_output_config()
            .map_err(|e| format!("Cannot get output config: {e}"))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();

        // Build the LAME MP3 encoder
        let mut builder = Builder::new().ok_or("Failed to create LAME builder")?;
        builder
            .set_num_channels(channels as u8)
            .map_err(|e| format!("LAME set_num_channels: {e:?}"))?;
        builder
            .set_sample_rate(sample_rate)
            .map_err(|e| format!("LAME set_sample_rate: {e:?}"))?;
        builder
            .set_brate(mp3lame_encoder::Bitrate::Kbps128)
            .map_err(|e| format!("LAME set_brate: {e:?}"))?;
        builder
            .set_quality(mp3lame_encoder::Quality::Best)
            .map_err(|e| format!("LAME set_quality: {e:?}"))?;
        let encoder = builder.build().map_err(|e| format!("LAME build: {e:?}"))?;

        let encoder = Arc::new(Mutex::new(encoder));
        let file = Arc::new(Mutex::new(
            File::create(&path).map_err(|e| format!("Cannot create file: {e}"))?,
        ));
        let stop_flag = Arc::new(AtomicBool::new(false));

        let enc_clone = encoder.clone();
        let file_clone = file.clone();
        let stop_clone = stop_flag.clone();

        // Build loopback input stream on the output device
        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if stop_clone.load(Ordering::Relaxed) {
                        return;
                    }

                    let mut enc = enc_clone.lock().unwrap();

                    let mp3_bytes = if channels == 1 {
                        // Mono: convert f32 to i16
                        let pcm: Vec<i16> = data
                            .iter()
                            .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                            .collect();
                        let num_samples = pcm.len();
                        let buf_size = mp3lame_encoder::max_required_buffer_size(num_samples);
                        let mut out_buf: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); buf_size];
                        match enc.encode(MonoPcm(&pcm), &mut out_buf) {
                            Ok(written) => {
                                let bytes: Vec<u8> = out_buf[..written]
                                    .iter()
                                    .map(|b| unsafe { b.assume_init() })
                                    .collect();
                                bytes
                            }
                            Err(_) => vec![],
                        }
                    } else {
                        // Stereo interleaved: deinterleave L R L R → [L L L] [R R R]
                        let pcm: Vec<i16> = data
                            .iter()
                            .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                            .collect();
                        let left: Vec<i16> = pcm.iter().step_by(2).copied().collect();
                        let right: Vec<i16> = pcm.iter().skip(1).step_by(2).copied().collect();
                        let num_samples = left.len();
                        let buf_size = mp3lame_encoder::max_required_buffer_size(num_samples);
                        let mut out_buf: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); buf_size];
                        match enc.encode(DualPcm { left: &left, right: &right }, &mut out_buf) {
                            Ok(written) => {
                                let bytes: Vec<u8> = out_buf[..written]
                                    .iter()
                                    .map(|b| unsafe { b.assume_init() })
                                    .collect();
                                bytes
                            }
                            Err(_) => vec![],
                        }
                    };

                    if !mp3_bytes.is_empty() {
                        let mut f = file_clone.lock().unwrap();
                        let _ = f.write_all(&mp3_bytes);
                    }
                },
                |err| eprintln!("[AudioCapture] Stream error: {err}"),
                None,
            )
            .map_err(|e| format!("build_input_stream failed: {e}"))?;

        stream.play().map_err(|e| format!("stream.play() failed: {e}"))?;

        Ok(Self {
            stream,
            encoder,
            file,
            stop_flag,
            path,
            channels,
        })
    }

    /// Stop recording, flush the MP3 encoder, and return the file path.
    pub fn stop(self) -> Result<PathBuf, String> {
        self.stop_flag.store(true, Ordering::Relaxed);

        // Drop stream first — this stops the audio callback
        drop(self.stream);
        // Give in-flight callbacks time to finish
        std::thread::sleep(std::time::Duration::from_millis(150));

        // Flush LAME encoder (writes final MP3 frames)
        let final_bytes = {
            let mut enc = self.encoder.lock().unwrap();
            // Need at least 7200 bytes for flush
            let buf_size = 7200;
            let mut out_buf: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); buf_size];
            let written = enc
                .flush::<FlushNoGap>(&mut out_buf)
                .map_err(|e| format!("LAME flush failed: {e:?}"))?;
            let bytes: Vec<u8> = out_buf[..written]
                .iter()
                .map(|b| unsafe { b.assume_init() })
                .collect();
            bytes
        };

        {
            let mut f = self.file.lock().unwrap();
            f.write_all(&final_bytes)
                .map_err(|e| format!("Write flush failed: {e}"))?;
            f.flush().map_err(|e| format!("File flush failed: {e}"))?;
        }

        Ok(self.path)
    }
}
