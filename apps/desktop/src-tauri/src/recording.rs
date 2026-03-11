use std::{
    io::Cursor,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, Stream, StreamConfig,
};

use crate::media::MediaPlaybackInterruption;
use crate::models::RecordedAudio;

pub struct RecordingHandle {
    media_playback: MediaPlaybackInterruption,
    stop_sender: Sender<()>,
    join_handle: JoinHandle<Result<RecordedAudio, String>>,
}

pub fn start_recording(
    slot: &Mutex<Option<RecordingHandle>>,
    pause_background_audio: bool,
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
    let (stop_sender, stop_receiver) = mpsc::channel();
    let join_handle = thread::spawn(move || record_until_stopped(stop_receiver));

    *guard = Some(RecordingHandle {
        media_playback,
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
    } = handle;

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

fn record_until_stopped(stop_receiver: Receiver<()>) -> Result<RecordedAudio, String> {
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
        SampleFormat::F32 => {
            build_input_stream_f32(&device, &stream_config, channels, samples.clone())?
        }
        SampleFormat::I16 => {
            build_input_stream_i16(&device, &stream_config, channels, samples.clone())?
        }
        SampleFormat::U16 => {
            build_input_stream_u16(&device, &stream_config, channels, samples.clone())?
        }
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
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[f32], _| {
                let mut guard = samples.lock().expect("sample buffer poisoned");
                for frame in data.chunks(channels) {
                    let value = frame.first().copied().unwrap_or_default().clamp(-1.0, 1.0);
                    guard.push((value * i16::MAX as f32) as i16);
                }
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
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[i16], _| {
                let mut guard = samples.lock().expect("sample buffer poisoned");
                for frame in data.chunks(channels) {
                    guard.push(frame.first().copied().unwrap_or_default());
                }
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
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[u16], _| {
                let mut guard = samples.lock().expect("sample buffer poisoned");
                for frame in data.chunks(channels) {
                    let sample = frame.first().copied().unwrap_or_default();
                    guard.push((sample as i32 - i16::MAX as i32 - 1) as i16);
                }
            },
            move |error| {
                eprintln!("input stream error: {error}");
            },
            None,
        )
        .map_err(|error| error.to_string())
}
