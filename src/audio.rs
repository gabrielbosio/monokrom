use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};
use macroquad::time::get_time;

use crate::config::{
    MUSIC_CHANNELS, MUSIC_COUNT, MUSIC_END_LOOP, MUSIC_HEADER_BYTES, MUSIC_PATTERN_SIZE,
    MUSIC_ROWS, SFX_CELLS, SFX_COUNT, SFX_HEADER_BYTES, SFX_SIZE,
};

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

pub const SFX_PULSE: u8 = 0;
pub const SFX_WAVE: u8 = 1;
pub const SFX_NOISE: u8 = 2;

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

pub fn music_pattern_speed(data: &[u8], idx: u8) -> u8 {
    data[idx as usize * MUSIC_PATTERN_SIZE]
}

pub fn music_pattern_end_flag(data: &[u8], idx: u8) -> u8 {
    data[idx as usize * MUSIC_PATTERN_SIZE + 1]
}

pub fn music_pattern_loop_start(data: &[u8], idx: u8) -> u8 {
    data[idx as usize * MUSIC_PATTERN_SIZE + 2]
}

pub fn music_pattern_loop_end(data: &[u8], idx: u8) -> u8 {
    data[idx as usize * MUSIC_PATTERN_SIZE + 3]
}

pub fn set_music_pattern_header(
    data: &mut [u8],
    idx: u8,
    speed: u8,
    end_flag: u8,
    lstart: u8,
    lend: u8,
) {
    let off = idx as usize * MUSIC_PATTERN_SIZE;
    data[off] = speed;
    data[off + 1] = end_flag;
    data[off + 2] = lstart;
    data[off + 3] = lend;
}

pub fn music_cell_at(data: &[u8], pattern: u8, row: u8, channel: u8) -> u16 {
    let off = pattern as usize * MUSIC_PATTERN_SIZE
        + MUSIC_HEADER_BYTES
        + (row as usize * MUSIC_CHANNELS + channel as usize) * 2;
    u16::from_le_bytes([data[off], data[off + 1]])
}

pub fn set_music_cell(data: &mut [u8], pattern: u8, row: u8, channel: u8, value: u16) {
    let off = pattern as usize * MUSIC_PATTERN_SIZE
        + MUSIC_HEADER_BYTES
        + (row as usize * MUSIC_CHANNELS + channel as usize) * 2;
    let bytes = value.to_le_bytes();
    data[off] = bytes[0];
    data[off + 1] = bytes[1];
}

pub fn music_pattern_is_empty(data: &[u8], idx: u8) -> bool {
    let off = idx as usize * MUSIC_PATTERN_SIZE;
    data[off..off + MUSIC_PATTERN_SIZE].iter().all(|&b| b == 0)
}

pub fn music_pack_cell(pitch: u8, sfx: u8, vol: u8, det: u8) -> u16 {
    (pitch as u16 & 0x3F)
        | ((sfx as u16 & 0x0F) << 6)
        | ((vol as u16 & 0x07) << 10)
        | ((det as u16 & 0x07) << 13)
}

pub fn music_cell_pitch(c: u16) -> u8 {
    (c & 0x3F) as u8
}

pub fn music_cell_sfx(c: u16) -> u8 {
    ((c >> 6) & 0x0F) as u8
}

pub fn music_cell_volume(c: u16) -> u8 {
    ((c >> 10) & 0x07) as u8
}

pub fn music_cell_detune(c: u16) -> u8 {
    ((c >> 13) & 0x07) as u8
}

pub fn music_column_sfx_type(channel: u8) -> u8 {
    match channel {
        0 | 1 => SFX_PULSE,
        2 => SFX_WAVE,
        _ => SFX_NOISE,
    }
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
    let sfx_type = sfx_channel(data, idx);
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
            let raw = match sfx_type {
                SFX_PULSE => {
                    phase += freq / SAMPLE_RATE as f32;
                    phase -= phase.floor();
                    let duty = duty_from_timbre(timbre);
                    if phase < duty {
                        1.0
                    } else {
                        -1.0
                    }
                }
                SFX_WAVE => {
                    phase += freq / SAMPLE_RATE as f32;
                    phase -= phase.floor();
                    sample_wave(timbre, phase)
                }
                SFX_NOISE => {
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

pub fn render_music(music_data: &[u8], sfx_data: &[u8], idx: u8) -> Vec<u8> {
    let m_speed = music_pattern_speed(music_data, idx).clamp(1, MAX_SPEED);
    let t = (m_speed - 1) as f32 / (MAX_SPEED - 1) as f32;
    let row_secs = SLOWEST_CELL_SECS * (FASTEST_CELL_SECS / SLOWEST_CELL_SECS).powf(t);
    let row_samples = (row_secs * SAMPLE_RATE as f32).max(1.0) as usize;
    let total = row_samples * MUSIC_ROWS;

    let mut mix = vec![0f32; total];

    for ch in 0..MUSIC_CHANNELS {
        let column_type = music_column_sfx_type(ch as u8);
        let mut phase: f32 = 0.0;
        let mut lfsr: u16 = 0x7FFF;
        let mut prev: Option<(u8, u8)> = None;

        for row in 0..MUSIC_ROWS {
            let m_cell = music_cell_at(music_data, idx, row as u8, ch as u8);
            let m_pitch = music_cell_pitch(m_cell);
            let m_sfx = music_cell_sfx(m_cell);
            let m_vol = music_cell_volume(m_cell);
            let m_det = music_cell_detune(m_cell);

            if m_vol == 0
                || is_empty(sfx_data, m_sfx)
                || sfx_channel(sfx_data, m_sfx) != column_type
            {
                phase = 0.0;
                lfsr = 0x7FFF;
                prev = None;
                continue;
            }

            if prev != Some((m_sfx, m_pitch)) {
                phase = 0.0;
                lfsr = 0x7FFF;
            }
            prev = Some((m_sfx, m_pitch));

            let s_speed = sfx_speed(sfx_data, m_sfx).clamp(1, MAX_SPEED);
            let st = (s_speed - 1) as f32 / (MAX_SPEED - 1) as f32;
            let sfx_cell_secs =
                SLOWEST_CELL_SECS * (FASTEST_CELL_SECS / SLOWEST_CELL_SECS).powf(st);
            let sfx_cell_samples = (sfx_cell_secs * SAMPLE_RATE as f32).max(1.0) as usize;

            let sfx_base_pitch = cell_pitch(cell_at(sfx_data, m_sfx, 0)) as i32;
            let transpose = m_pitch as i32 - sfx_base_pitch;
            let music_vol = m_vol as f32 / 7.0;

            let row_offset = row * row_samples;

            for s in 0..row_samples {
                let sfx_cell_idx = s / sfx_cell_samples;
                if sfx_cell_idx >= SFX_CELLS {
                    break;
                }
                let sfx_cell = cell_at(sfx_data, m_sfx, sfx_cell_idx as u8);
                let s_vol = cell_volume(sfx_cell);
                if s_vol == 0 {
                    phase = 0.0;
                    continue;
                }
                let s_pitch = cell_pitch(sfx_cell) as i32;
                let s_timbre = cell_timbre(sfx_cell);
                let s_det = cell_detune(sfx_cell);

                let effective_pitch = (s_pitch + transpose).clamp(0, 63);
                let total_detune = (s_det + m_det).min(7);
                let freq =
                    midi_to_freq(36 + effective_pitch) * 2f32.powf(total_detune as f32 / 96.0);
                let combined_vol = (s_vol as f32 / 7.0) * music_vol;

                let raw = match column_type {
                    SFX_PULSE => {
                        phase += freq / SAMPLE_RATE as f32;
                        phase -= phase.floor();
                        let duty = duty_from_timbre(s_timbre);
                        if phase < duty {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    SFX_WAVE => {
                        phase += freq / SAMPLE_RATE as f32;
                        phase -= phase.floor();
                        sample_wave(s_timbre, phase)
                    }
                    SFX_NOISE => {
                        phase += freq / SAMPLE_RATE as f32;
                        while phase >= 1.0 {
                            let bit = (lfsr ^ (lfsr >> 1)) & 1;
                            lfsr = (lfsr >> 1) | (bit << 14);
                            if s_timbre & 1 != 0 {
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

                mix[row_offset + s] += raw * combined_vol * MASTER_GAIN;
            }
        }
    }

    let mut samples: Vec<i16> = mix
        .into_iter()
        .map(|v| (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect();

    apply_music_loop(&mut samples, music_data, idx, row_samples);
    make_wav(&samples)
}

fn apply_music_loop(samples: &mut Vec<i16>, data: &[u8], idx: u8, row_samples: usize) {
    if music_pattern_end_flag(data, idx) != MUSIC_END_LOOP {
        return;
    }
    let lstart = music_pattern_loop_start(data, idx);
    let lend = music_pattern_loop_end(data, idx);
    if lstart >= MUSIC_ROWS as u8 || lend <= lstart || lend > MUSIC_ROWS as u8 {
        return;
    }
    let lead_end = lstart as usize * row_samples;
    let body_end = lend as usize * row_samples;
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

pub struct SfxPlayer {
    sounds: [Option<Sound>; SFX_COUNT],
    dirty: [bool; SFX_COUNT],
    channel_handles: [Option<Sound>; 4],
    channel_last_play: [f64; 4],
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
            channel_last_play: [0.0; 4],
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
        let channel = match sfx_channel(data, idx) {
            SFX_PULSE => {
                if self.channel_last_play[CH_PULSE1 as usize]
                    <= self.channel_last_play[CH_PULSE2 as usize]
                {
                    CH_PULSE1 as usize
                } else {
                    CH_PULSE2 as usize
                }
            }
            SFX_WAVE => CH_WAVE as usize,
            SFX_NOISE => CH_NOISE as usize,
            _ => return,
        };
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
        self.channel_last_play[channel] = get_time();
    }

    pub fn stop_all(&mut self) {
        for slot in &mut self.channel_handles {
            if let Some(s) = slot.take() {
                stop_sound(&s);
            }
        }
    }
}

pub struct MusicPlayer {
    sounds: [Option<Sound>; MUSIC_COUNT],
    dirty: [bool; MUSIC_COUNT],
    handle: Option<Sound>,
}

impl Default for MusicPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl MusicPlayer {
    pub fn new() -> Self {
        Self {
            sounds: std::array::from_fn(|_| None),
            dirty: [true; MUSIC_COUNT],
            handle: None,
        }
    }

    pub fn mark_dirty(&mut self, idx: u8) {
        if (idx as usize) < MUSIC_COUNT {
            self.dirty[idx as usize] = true;
        }
    }

    pub fn mark_all_dirty(&mut self) {
        for d in &mut self.dirty {
            *d = true;
        }
    }

    pub async fn ensure_loaded(&mut self, music_data: &[u8], sfx_data: &[u8], idx: u8) -> bool {
        let i = idx as usize;
        if i >= MUSIC_COUNT {
            return false;
        }
        if !self.dirty[i] && self.sounds[i].is_some() {
            return true;
        }
        if music_pattern_is_empty(music_data, idx) {
            self.sounds[i] = None;
            self.dirty[i] = false;
            return false;
        }
        if let Some(old) = self.sounds[i].take() {
            stop_sound(&old);
        }
        let wav = render_music(music_data, sfx_data, idx);
        match load_sound_from_bytes(&wav).await {
            Ok(s) => {
                self.sounds[i] = Some(s);
                self.dirty[i] = false;
                true
            }
            Err(_) => false,
        }
    }

    pub fn play(&mut self, idx: u8) {
        let i = idx as usize;
        if i >= MUSIC_COUNT {
            return;
        }
        let Some(sound) = self.sounds[i].clone() else {
            return;
        };
        if let Some(prev) = self.handle.take() {
            stop_sound(&prev);
        }
        play_sound(
            &sound,
            PlaySoundParams {
                looped: false,
                volume: 1.0,
            },
        );
        self.handle = Some(sound);
    }

    pub fn stop(&mut self) {
        if let Some(s) = self.handle.take() {
            stop_sound(&s);
        }
    }
}
