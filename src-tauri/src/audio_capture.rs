use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mp3lame_encoder::{Builder, DualPcm, FlushNoGap, MonoPcm};
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct AudioCapture {
    loopback_stream: cpal::Stream,
    /// Microphone stream — None if no input device is available.
    mic_stream: Option<cpal::Stream>,
    encoder: Arc<Mutex<mp3lame_encoder::Encoder>>,
    file: Arc<Mutex<File>>,
    stop_flag: Arc<AtomicBool>,
    pub path: PathBuf,
}

// cpal::Stream contains a raw pointer that is !Send, but we gate all access
// through Arc<Mutex<Option<AudioCapture>>> and only touch streams from one
// owner at a time, so this is safe.
unsafe impl Send for AudioCapture {}
unsafe impl Sync for AudioCapture {}

impl AudioCapture {
    /// Start capturing system audio (WASAPI loopback) mixed with the local
    /// microphone, encoding to MP3 in real time and writing to `path`.
    pub fn start(path: PathBuf) -> Result<Self, String> {
        let host = cpal::host_from_id(cpal::HostId::Wasapi)
            .map_err(|e| format!("WASAPI unavailable: {e}"))?;

        // ── Output device (loopback — captures remote participants) ──────────
        let out_device = host
            .default_output_device()
            .ok_or("No default output device")?;
        let out_config = out_device
            .default_output_config()
            .map_err(|e| format!("Output config: {e}"))?;

        let sample_rate = out_config.sample_rate().0;
        let channels = out_config.channels();

        // ── MP3 encoder ──────────────────────────────────────────────────────
        let mut builder = Builder::new().ok_or("LAME builder failed")?;
        builder
            .set_num_channels(channels as u8)
            .map_err(|e| format!("LAME channels: {e:?}"))?;
        builder
            .set_sample_rate(sample_rate)
            .map_err(|e| format!("LAME sample_rate: {e:?}"))?;
        builder
            .set_brate(mp3lame_encoder::Bitrate::Kbps128)
            .map_err(|e| format!("LAME brate: {e:?}"))?;
        builder
            .set_quality(mp3lame_encoder::Quality::Best)
            .map_err(|e| format!("LAME quality: {e:?}"))?;
        let encoder = Arc::new(Mutex::new(
            builder.build().map_err(|e| format!("LAME build: {e:?}"))?,
        ));

        let file = Arc::new(Mutex::new(
            File::create(&path).map_err(|e| format!("File create: {e}"))?,
        ));
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Shared ring buffer: mic callback pushes samples here;
        // loopback callback pops and mixes them before encoding.
        let mic_buf: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));

        // ── Microphone stream (captures local speaker) ───────────────────────
        let mic_stream = match host.default_input_device() {
            None => {
                eprintln!("[AudioCapture] No mic device — recording loopback only");
                None
            }
            Some(mic_dev) => match mic_dev.default_input_config() {
                Err(e) => {
                    eprintln!("[AudioCapture] Mic config error: {e} — loopback only");
                    None
                }
                Ok(mic_cfg) => {
                    let mic_channels = mic_cfg.channels();
                    let mic_buf_w = mic_buf.clone();
                    let stop_mic = stop_flag.clone();

                    match mic_dev.build_input_stream(
                        &mic_cfg.into(),
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            if stop_mic.load(Ordering::Relaxed) {
                                return;
                            }
                            let mut buf = mic_buf_w.lock().unwrap();
                            // If mic is mono but loopback is stereo, duplicate
                            // each sample so channel counts align when mixing.
                            if mic_channels == 1 && channels == 2 {
                                for &s in data {
                                    buf.push_back(s);
                                    buf.push_back(s);
                                }
                            } else {
                                buf.extend(data.iter().copied());
                            }
                            // Cap to 2 seconds to prevent unbounded growth
                            // if mic runs faster than loopback drains it.
                            let max = (sample_rate * channels as u32 * 2) as usize;
                            while buf.len() > max {
                                buf.pop_front();
                            }
                        },
                        |err| eprintln!("[AudioCapture] Mic stream error: {err}"),
                        None,
                    ) {
                        Err(e) => {
                            eprintln!("[AudioCapture] build_mic_stream failed: {e}");
                            None
                        }
                        Ok(stream) => {
                            if let Err(e) = stream.play() {
                                eprintln!("[AudioCapture] mic stream play failed: {e}");
                                None
                            } else {
                                eprintln!("[AudioCapture] Mic stream started");
                                Some(stream)
                            }
                        }
                    }
                }
            },
        };

        // ── Loopback stream (output device read as input) ─────────────────────
        let enc_clone = encoder.clone();
        let file_clone = file.clone();
        let stop_clone = stop_flag.clone();
        let mic_buf_r = mic_buf.clone();

        let loopback_stream = out_device
            .build_input_stream(
                &out_config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if stop_clone.load(Ordering::Relaxed) {
                        return;
                    }

                    // Mix loopback + mic: sum samples, clamp to [-1.0, 1.0]
                    let mixed: Vec<f32> = {
                        let mut mic = mic_buf_r.lock().unwrap();
                        data.iter()
                            .map(|&s| {
                                let m = mic.pop_front().unwrap_or(0.0);
                                (s + m).clamp(-1.0, 1.0)
                            })
                            .collect()
                    };

                    let mut enc = enc_clone.lock().unwrap();

                    let mp3_bytes = if channels == 1 {
                        let pcm: Vec<i16> = mixed
                            .iter()
                            .map(|&s| (s * i16::MAX as f32) as i16)
                            .collect();
                        let buf_size = mp3lame_encoder::max_required_buffer_size(pcm.len());
                        let mut out = vec![MaybeUninit::uninit(); buf_size];
                        match enc.encode(MonoPcm(&pcm), &mut out) {
                            Ok(n) => out[..n]
                                .iter()
                                .map(|b| unsafe { b.assume_init() })
                                .collect(),
                            Err(_) => vec![],
                        }
                    } else {
                        // Stereo interleaved LRLR → deinterleave [L...] [R...]
                        let pcm: Vec<i16> = mixed
                            .iter()
                            .map(|&s| (s * i16::MAX as f32) as i16)
                            .collect();
                        let left: Vec<i16> = pcm.iter().step_by(2).copied().collect();
                        let right: Vec<i16> = pcm.iter().skip(1).step_by(2).copied().collect();
                        let buf_size = mp3lame_encoder::max_required_buffer_size(left.len());
                        let mut out = vec![MaybeUninit::uninit(); buf_size];
                        match enc.encode(DualPcm { left: &left, right: &right }, &mut out) {
                            Ok(n) => out[..n]
                                .iter()
                                .map(|b| unsafe { b.assume_init() })
                                .collect(),
                            Err(_) => vec![],
                        }
                    };

                    if !mp3_bytes.is_empty() {
                        let mut f = file_clone.lock().unwrap();
                        let _ = f.write_all(&mp3_bytes);
                    }
                },
                |err| eprintln!("[AudioCapture] Loopback stream error: {err}"),
                None,
            )
            .map_err(|e| format!("build_loopback_stream failed: {e}"))?;

        loopback_stream
            .play()
            .map_err(|e| format!("loopback play() failed: {e}"))?;

        Ok(Self {
            loopback_stream,
            mic_stream,
            encoder,
            file,
            stop_flag,
            path,
        })
    }

    /// Stop recording, flush the MP3 encoder, and return the file path.
    pub fn stop(self) -> Result<PathBuf, String> {
        self.stop_flag.store(true, Ordering::Relaxed);

        // Drop streams — stops audio callbacks
        drop(self.loopback_stream);
        drop(self.mic_stream);
        // Give in-flight callbacks time to finish
        std::thread::sleep(std::time::Duration::from_millis(150));

        let final_bytes = {
            let mut enc = self.encoder.lock().unwrap();
            let mut out = vec![MaybeUninit::uninit(); 7200];
            let n = enc
                .flush::<FlushNoGap>(&mut out)
                .map_err(|e| format!("LAME flush: {e:?}"))?;
            out[..n]
                .iter()
                .map(|b| unsafe { b.assume_init() })
                .collect::<Vec<u8>>()
        };

        {
            let mut f = self.file.lock().unwrap();
            f.write_all(&final_bytes)
                .map_err(|e| format!("Write flush: {e}"))?;
            f.flush().map_err(|e| format!("File flush: {e}"))?;
        }

        Ok(self.path)
    }
}
