use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};

use crate::config::{SFX_CELLS, SFX_COUNT, SFX_HEADER_BYTES, SFX_SIZE};

const SAMPLE_RATE: u32 = 22050;
const SLOWEST_CELL_SECS: f32 = 0.320;
const FASTEST_CELL_SECS: f32 = 0.020;
const MASTER_GAIN: f32 = 0.4;
const MAX_LOOP_SECS: f32 = 5.0;
const MAX_LOOP_ITERS: usize = 64;

pub const MAX_SPEED: u8 = 16;

pub const CH_PULSE1: u8 = 0;
pub const CH_PULSE2: u8 = 1;
pub const CH_WAVE: u8 = 2;
pub const CH_NOISE: u8 = 3;

pub fn sfx_channel(data: &[u8], idx: u8) -> u8 {
    data[idx as usize * SFX_SIZE] & 0x03
}

pub fn sfx_speed(data: &[u8], idx: u8) -> u8 {
    data[idx as usize * SFX_SIZE + 1]
}

pub fn sfx_loop_start(data: &[u8], idx: u8) -> u8 {
    data[idx as usize * SFX_SIZE + 2]
}

pub fn sfx_loop_end(data: &[u8], idx: u8) -> u8 {
    data[idx as usize * SFX_SIZE + 3]
}

pub fn set_sfx_header(data: &mut [u8], idx: u8, channel: u8, speed: u8, lstart: u8, lend: u8) {
    let off = idx as usize * SFX_SIZE;
    data[off] = channel & 0x03;
    data[off + 1] = speed;
    data[off + 2] = lstart;
    data[off + 3] = lend;
}

pub fn cell_at(data: &[u8], sfx: u8, cell: u8) -> u16 {
    let off = sfx as usize * SFX_SIZE + SFX_HEADER_BYTES + cell as usize * 2;
    u16::from_le_bytes([data[off], data[off + 1]])
}

pub fn set_cell(data: &mut [u8], sfx: u8, cell: u8, value: u16) {
    let off = sfx as usize * SFX_SIZE + SFX_HEADER_BYTES + cell as usize * 2;
    let bytes = value.to_le_bytes();
    data[off] = bytes[0];
    data[off + 1] = bytes[1];
}

pub fn pack_cell(pitch: u8, timbre: u8, volume: u8, detune: u8) -> u16 {
    (pitch as u16 & 0x3F)
        | ((timbre as u16 & 0x07) << 6)
        | ((volume as u16 & 0x07) << 9)
        | ((detune as u16 & 0x07) << 12)
}

pub fn cell_pitch(c: u16) -> u8 {
    (c & 0x3F) as u8
}

pub fn cell_timbre(c: u16) -> u8 {
    ((c >> 6) & 0x07) as u8
}

pub fn cell_volume(c: u16) -> u8 {
    ((c >> 9) & 0x07) as u8
}

pub fn cell_detune(c: u16) -> u8 {
    ((c >> 12) & 0x07) as u8
}

pub fn is_empty(data: &[u8], idx: u8) -> bool {
    let off = idx as usize * SFX_SIZE;
    data[off..off + SFX_SIZE].iter().all(|&b| b == 0)
}

fn midi_to_freq(note: i32) -> f32 {
    440.0 * 2f32.powf((note - 69) as f32 / 12.0)
}

fn duty_from_timbre(timbre: u8) -> f32 {
    match timbre & 0x03 {
        0 => 0.125,
        1 => 0.25,
        2 => 0.5,
        _ => 0.75,
    }
}

fn sample_wave(bank: u8, phase: f32) -> f32 {
    let p = phase - phase.floor();
    match bank & 0x07 {
        0 => (p * std::f32::consts::TAU).sin(),
        1 => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        2 => {
            if p < 0.25 {
                4.0 * p
            } else if p < 0.75 {
                2.0 - 4.0 * p
            } else {
                4.0 * p - 4.0
            }
        }
        3 => 2.0 * p - 1.0,
        4 => 1.0 - 2.0 * p,
        5 => {
            if p < 0.25 {
                1.0
            } else {
                -1.0
            }
        }
        6 => {
            if p < 0.125 {
                1.0
            } else {
                -1.0
            }
        }
        _ => {
            let s = (p * std::f32::consts::PI).sin();
            2.0 * s - 1.0
        }
    }
}

pub fn render_sfx(data: &[u8], idx: u8) -> Vec<u8> {
    let channel = sfx_channel(data, idx);
    let speed = sfx_speed(data, idx).clamp(1, MAX_SPEED);
    let t = (speed - 1) as f32 / (MAX_SPEED - 1) as f32;
    let cell_secs = SLOWEST_CELL_SECS * (FASTEST_CELL_SECS / SLOWEST_CELL_SECS).powf(t);
    let cell_samples = (cell_secs * SAMPLE_RATE as f32).max(1.0) as usize;
    let total = cell_samples * SFX_CELLS;

    let mut samples = vec![0i16; total];
    let mut phase: f32 = 0.0;
    let mut lfsr: u16 = 0x7FFF;

    for ci in 0..SFX_CELLS {
        let c = cell_at(data, idx, ci as u8);
        let pitch = cell_pitch(c);
        let timbre = cell_timbre(c);
        let volume = cell_volume(c);
        let detune = cell_detune(c);

        if volume == 0 {
            phase = 0.0;
            continue;
        }

        let freq = midi_to_freq(36 + pitch as i32) * 2f32.powf(detune as f32 / 96.0);
        let vol = volume as f32 / 7.0;

        for s in 0..cell_samples {
            let raw = match channel {
                CH_PULSE1 | CH_PULSE2 => {
                    phase += freq / SAMPLE_RATE as f32;
                    phase -= phase.floor();
                    let duty = duty_from_timbre(timbre);
                    if phase < duty {
                        1.0
                    } else {
                        -1.0
                    }
                }
                CH_WAVE => {
                    phase += freq / SAMPLE_RATE as f32;
                    phase -= phase.floor();
                    sample_wave(timbre, phase)
                }
                CH_NOISE => {
                    phase += freq / SAMPLE_RATE as f32;
                    while phase >= 1.0 {
                        let bit = (lfsr ^ (lfsr >> 1)) & 1;
                        lfsr = (lfsr >> 1) | (bit << 14);
                        if timbre & 1 != 0 {
                            lfsr = (lfsr & !(1 << 6)) | (bit << 6);
                        }
                        phase -= 1.0;
                    }
                    if lfsr & 1 == 0 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                _ => 0.0,
            };

            let out = raw * vol * MASTER_GAIN;
            samples[ci * cell_samples + s] = (out.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        }
    }

    apply_loop(&mut samples, data, idx, cell_samples);
    make_wav(&samples)
}

fn apply_loop(samples: &mut Vec<i16>, data: &[u8], idx: u8, cell_samples: usize) {
    let lstart = sfx_loop_start(data, idx);
    let lend = sfx_loop_end(data, idx);
    if lstart >= SFX_CELLS as u8 || lend <= lstart || lend > SFX_CELLS as u8 {
        return;
    }
    let lead_end = lstart as usize * cell_samples;
    let body_end = lend as usize * cell_samples;
    let body_len = body_end - lead_end;
    samples.truncate(body_end);
    if body_len == 0 {
        return;
    }
    let max_extra = ((MAX_LOOP_SECS * SAMPLE_RATE as f32) as usize) / body_len;
    let extra = max_extra.min(MAX_LOOP_ITERS.saturating_sub(1));
    samples.reserve(body_len * extra);
    let body_range = lead_end..body_end;
    for _ in 0..extra {
        samples.extend_from_within(body_range.clone());
    }
}

fn make_wav(samples: &[i16]) -> Vec<u8> {
    let data_size = samples.len() * 2;
    let mut buf = Vec::with_capacity(44 + data_size);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&((36 + data_size) as u32).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    buf.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&(data_size as u32).to_le_bytes());
    for s in samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    buf
}

pub struct SfxPlayer {
    sounds: [Option<Sound>; SFX_COUNT],
    dirty: [bool; SFX_COUNT],
    channel_handles: [Option<Sound>; 4],
}

impl Default for SfxPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl SfxPlayer {
    pub fn new() -> Self {
        Self {
            sounds: std::array::from_fn(|_| None),
            dirty: [true; SFX_COUNT],
            channel_handles: std::array::from_fn(|_| None),
        }
    }

    pub fn mark_dirty(&mut self, idx: u8) {
        if (idx as usize) < SFX_COUNT {
            self.dirty[idx as usize] = true;
        }
    }

    pub fn mark_all_dirty(&mut self) {
        for d in &mut self.dirty {
            *d = true;
        }
    }

    pub async fn ensure_loaded(&mut self, data: &[u8], idx: u8) -> bool {
        let i = idx as usize;
        if i >= SFX_COUNT {
            return false;
        }
        if !self.dirty[i] && self.sounds[i].is_some() {
            return true;
        }
        if is_empty(data, idx) {
            self.sounds[i] = None;
            self.dirty[i] = false;
            return false;
        }
        if let Some(old) = self.sounds[i].take() {
            stop_sound(&old);
        }
        let wav = render_sfx(data, idx);
        match load_sound_from_bytes(&wav).await {
            Ok(s) => {
                self.sounds[i] = Some(s);
                self.dirty[i] = false;
                true
            }
            Err(_) => false,
        }
    }

    pub fn play(&mut self, data: &[u8], idx: u8) {
        let i = idx as usize;
        if i >= SFX_COUNT {
            return;
        }
        let Some(sound) = self.sounds[i].clone() else {
            return;
        };
        let channel = sfx_channel(data, idx) as usize;
        if channel >= self.channel_handles.len() {
            return;
        }
        if let Some(prev) = self.channel_handles[channel].take() {
            stop_sound(&prev);
        }
        play_sound(
            &sound,
            PlaySoundParams {
                looped: false,
                volume: 1.0,
            },
        );
        self.channel_handles[channel] = Some(sound);
    }

    pub fn stop_all(&mut self) {
        for slot in &mut self.channel_handles {
            if let Some(s) = slot.take() {
                stop_sound(&s);
            }
        }
    }
}
