use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, Stream, StreamConfig,
};
use tauri::{AppHandle, Emitter};

use crate::hud::{HUD_METER_EVENT, HUD_WINDOW_LABEL};
use crate::media::MediaPlaybackInterruption;
use crate::models::{HudMeterPayload, RecordedAudio};

const HUD_METER_INTERVAL: Duration = Duration::from_millis(60);
const HUD_METER_GAIN: f64 = 6.0;

pub struct RecordingHandle {
    media_playback: MediaPlaybackInterruption,
    meter_broadcaster: HudMeterBroadcaster<AppHudMeterSink>,
    stop_sender: Sender<()>,
    join_handle: JoinHandle<Result<RecordedAudio, String>>,
}

pub fn start_recording(
    slot: &Mutex<Option<RecordingHandle>>,
    pause_background_audio: bool,
    app: &AppHandle,
) -> Result<(), String> {
    let mut guard = slot
        .lock()
        .map_err(|_| "Recording state is poisoned".to_string())?;
    if guard.is_some() {
        return Err("Recording is already in progress.".to_string());
    }

    let media_playback = if pause_background_audio {
        MediaPlaybackInterruption::begin()
    } else {
        MediaPlaybackInterruption::default()
    };
    let meter_broadcaster = HudMeterBroadcaster::new(AppHudMeterSink { app: app.clone() });
    let (stop_sender, stop_receiver) = mpsc::channel();
    let join_handle = thread::spawn({
        let meter_broadcaster = meter_broadcaster.clone();
        move || record_until_stopped(stop_receiver, meter_broadcaster)
    });

    *guard = Some(RecordingHandle {
        media_playback,
        meter_broadcaster,
        stop_sender,
        join_handle,
    });

    Ok(())
}

pub fn stop_recording(slot: &Mutex<Option<RecordingHandle>>) -> Result<RecordedAudio, String> {
    let handle = {
        let mut guard = slot
            .lock()
            .map_err(|_| "Recording state is poisoned".to_string())?;
        guard
            .take()
            .ok_or_else(|| "No active recording is currently running.".to_string())?
    };

    let RecordingHandle {
        stop_sender,
        join_handle,
        mut media_playback,
        meter_broadcaster,
    } = handle;

    meter_broadcaster.stop();
    let stop_result = stop_sender.send(());
    let audio_result = join_handle
        .join()
        .map_err(|_| "Recording thread panicked.".to_string())?;

    media_playback.resume();

    if stop_result.is_err() && audio_result.is_ok() {
        Err("Recording thread stopped unexpectedly.".to_string())
    } else {
        audio_result
    }
}

fn record_until_stopped(
    stop_receiver: Receiver<()>,
    meter_broadcaster: HudMeterBroadcaster<AppHudMeterSink>,
) -> Result<RecordedAudio, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No default microphone was found.".to_string())?;
    let supported_config = device
        .default_input_config()
        .map_err(|error| error.to_string())?;

    let stream_config: StreamConfig = supported_config.config();
    let sample_rate = stream_config.sample_rate.0;
    let channels = usize::from(stream_config.channels);
    let samples = Arc::new(Mutex::new(Vec::new()));

    let stream = match supported_config.sample_format() {
        SampleFormat::F32 => build_input_stream_f32(
            &device,
            &stream_config,
            channels,
            samples.clone(),
            meter_broadcaster.clone(),
        )?,
        SampleFormat::I16 => build_input_stream_i16(
            &device,
            &stream_config,
            channels,
            samples.clone(),
            meter_broadcaster.clone(),
        )?,
        SampleFormat::U16 => build_input_stream_u16(
            &device,
            &stream_config,
            channels,
            samples.clone(),
            meter_broadcaster.clone(),
        )?,
        format => {
            return Err(format!("Unsupported sample format: {format:?}"));
        }
    };

    stream.play().map_err(|error| error.to_string())?;
    let _ = stop_receiver.recv();
    stream.pause().map_err(|error| error.to_string())?;

    let captured_samples = samples
        .lock()
        .map_err(|_| "Recorded sample buffer is poisoned".to_string())?
        .clone();

    let duration_ms = if sample_rate == 0 {
        0
    } else {
        ((captured_samples.len() as f64 / sample_rate as f64) * 1000.0).round() as u32
    };

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer =
            hound::WavWriter::new(&mut cursor, spec).map_err(|error| error.to_string())?;
        for sample in captured_samples {
            writer
                .write_sample(sample)
                .map_err(|error| error.to_string())?;
        }
        writer.finalize().map_err(|error| error.to_string())?;
    }

    Ok(RecordedAudio {
        audio_base64: STANDARD.encode(cursor.into_inner()),
        duration_ms,
        sample_rate,
    })
}

fn build_input_stream_f32(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    samples: Arc<Mutex<Vec<i16>>>,
    meter_broadcaster: HudMeterBroadcaster<AppHudMeterSink>,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[f32], _| {
                let mut guard = samples.lock().expect("sample buffer poisoned");
                let mut sum_squares = 0.0;
                let mut sample_count = 0;
                for frame in data.chunks(channels) {
                    let value = frame.first().copied().unwrap_or_default().clamp(-1.0, 1.0);
                    guard.push((value * i16::MAX as f32) as i16);
                    sum_squares += f64::from(value * value);
                    sample_count += 1;
                }
                drop(guard);
                meter_broadcaster.publish(normalize_meter_level(sum_squares, sample_count));
            },
            move |error| {
                eprintln!("input stream error: {error}");
            },
            None,
        )
        .map_err(|error| error.to_string())
}

fn build_input_stream_i16(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    samples: Arc<Mutex<Vec<i16>>>,
    meter_broadcaster: HudMeterBroadcaster<AppHudMeterSink>,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[i16], _| {
                let mut guard = samples.lock().expect("sample buffer poisoned");
                let mut sum_squares = 0.0;
                let mut sample_count = 0;
                for frame in data.chunks(channels) {
                    let value = frame.first().copied().unwrap_or_default();
                    guard.push(value);
                    let normalized = f64::from(value) / f64::from(i16::MAX);
                    sum_squares += normalized * normalized;
                    sample_count += 1;
                }
                drop(guard);
                meter_broadcaster.publish(normalize_meter_level(sum_squares, sample_count));
            },
            move |error| {
                eprintln!("input stream error: {error}");
            },
            None,
        )
        .map_err(|error| error.to_string())
}

fn build_input_stream_u16(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    samples: Arc<Mutex<Vec<i16>>>,
    meter_broadcaster: HudMeterBroadcaster<AppHudMeterSink>,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[u16], _| {
                let mut guard = samples.lock().expect("sample buffer poisoned");
                let mut sum_squares = 0.0;
                let mut sample_count = 0;
                for frame in data.chunks(channels) {
                    let sample = frame.first().copied().unwrap_or_default();
                    let signed = (sample as i32 - i16::MAX as i32 - 1) as i16;
                    guard.push(signed);
                    let normalized = f64::from(signed) / f64::from(i16::MAX);
                    sum_squares += normalized * normalized;
                    sample_count += 1;
                }
                drop(guard);
                meter_broadcaster.publish(normalize_meter_level(sum_squares, sample_count));
            },
            move |error| {
                eprintln!("input stream error: {error}");
            },
            None,
        )
        .map_err(|error| error.to_string())
}

trait HudMeterSink: Send + Sync + 'static {
    fn emit(&self, level: f64);
}

#[derive(Clone)]
struct AppHudMeterSink {
    app: AppHandle,
}

impl HudMeterSink for AppHudMeterSink {
    fn emit(&self, level: f64) {
        let _ = self
            .app
            .emit_to(HUD_WINDOW_LABEL, HUD_METER_EVENT, HudMeterPayload { level });
    }
}

#[derive(Clone)]
struct HudMeterBroadcaster<S: HudMeterSink> {
    active: Arc<AtomicBool>,
    last_emitted_at: Arc<Mutex<Option<Instant>>>,
    sink: Arc<S>,
}

impl<S: HudMeterSink> HudMeterBroadcaster<S> {
    fn new(sink: S) -> Self {
        Self {
            active: Arc::new(AtomicBool::new(true)),
            last_emitted_at: Arc::new(Mutex::new(None)),
            sink: Arc::new(sink),
        }
    }

    fn publish(&self, level: f64) {
        self.publish_at(level, Instant::now());
    }

    fn publish_at(&self, level: f64, now: Instant) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }

        let mut guard = self
            .last_emitted_at
            .lock()
            .expect("hud meter throttle is poisoned");
        if guard
            .map(|last_emitted_at| now.duration_since(last_emitted_at) < HUD_METER_INTERVAL)
            .unwrap_or(false)
        {
            return;
        }

        *guard = Some(now);
        drop(guard);
        self.sink.emit(level.clamp(0.0, 1.0));
    }

    fn stop(&self) {
        self.active.store(false, Ordering::Relaxed);
    }
}

fn normalize_meter_level(sum_squares: f64, sample_count: usize) -> f64 {
    if sample_count == 0 {
        return 0.0;
    }

    let rms = (sum_squares / sample_count as f64).sqrt();
    (rms * HUD_METER_GAIN).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct CollectingHudMeterSink {
        levels: Arc<Mutex<Vec<f64>>>,
    }

    impl CollectingHudMeterSink {
        fn new() -> Self {
            Self {
                levels: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn levels(&self) -> Vec<f64> {
            self.levels.lock().unwrap().clone()
        }
    }

    impl HudMeterSink for CollectingHudMeterSink {
        fn emit(&self, level: f64) {
            self.levels.lock().unwrap().push(level);
        }
    }

    #[test]
    fn normalize_meter_level_stays_within_bounds() {
        assert_eq!(normalize_meter_level(0.0, 0), 0.0);
        assert!((0.0..=1.0).contains(&normalize_meter_level(0.001, 1)));
        assert_eq!(normalize_meter_level(4.0, 1), 1.0);
    }

    #[test]
    fn meter_broadcaster_throttles_updates() {
        let sink = CollectingHudMeterSink::new();
        let broadcaster = HudMeterBroadcaster::new(sink.clone());
        let start = Instant::now();

        broadcaster.publish_at(0.2, start);
        broadcaster.publish_at(0.4, start + Duration::from_millis(20));
        broadcaster.publish_at(0.6, start + Duration::from_millis(90));

        assert_eq!(sink.levels(), vec![0.2, 0.6]);
    }

    #[test]
    fn meter_broadcaster_stops_after_recording_ends() {
        let sink = CollectingHudMeterSink::new();
        let broadcaster = HudMeterBroadcaster::new(sink.clone());
        let start = Instant::now();

        broadcaster.publish_at(0.3, start);
        broadcaster.stop();
        broadcaster.publish_at(0.7, start + Duration::from_millis(120));

        assert_eq!(sink.levels(), vec![0.3]);
    }
}
