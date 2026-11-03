#![allow(dead_code)]

use std::path::Path;

use crate::audio::engine::{SoundEngine, TICK_RATE};
use crate::audio::format::SoundData;

pub fn render_song(data: SoundData, sample_rate: u32, seconds: f64, song: u8) -> Vec<f32> {
    let mut engine = SoundEngine::new(sample_rate, data);
    if !engine.play_song(song) {
        return Vec::new();
    }
    let mut samples = Vec::new();
    let ticks = (seconds.max(0.0) * TICK_RATE).ceil() as usize;
    for _ in 0..ticks {
        engine.advance(1.0 / TICK_RATE, &mut samples);
    }
    samples
}

pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> std::io::Result<()> {
    let data_size = samples.len() * 2;
    let mut bytes = Vec::with_capacity(44 + data_size);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_size) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_size as u32).to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::write(path, bytes)
}
