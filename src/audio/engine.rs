use crate::audio::format::{Event, SoundData};
use crate::audio::notes::{pulse_period, wave_period, NOTE_MAX, NOTE_OFF};
use crate::audio::synth::ChannelSynth;

pub const TICK_RATE: f64 = 60.0;
pub const FRAME_SEQUENCER_RATE: f64 = 512.0;
pub const DEFAULT_SAMPLE_RATE: u32 = 48000;
pub const MASTER_GAIN: f32 = 0.25;

struct ChannelState {
    note: u8,
    period: u16,
    volume: u8,
    shape: u8,
    gate: bool,
    sweep_pace: u8,
    sweep_dir_up: bool,
    sweep_step: u8,
    sweep_timer: u8,
    sweep_shadow: u16,
    sweep_enabled: bool,
    synth: ChannelSynth,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self {
            note: 0,
            period: 0,
            volume: 0,
            shape: 0,
            gate: false,
            sweep_pace: 0,
            sweep_dir_up: true,
            sweep_step: 0,
            sweep_timer: 0,
            sweep_shadow: 0,
            sweep_enabled: false,
            synth: ChannelSynth::new(),
        }
    }
}

struct PreviewState {
    song: u8,
    frame: u16,
    samples_to_next_frame: f64,
    speed_hz: u8,
}

pub struct SoundEngine {
    data: SoundData,
    sample_rate: u32,
    samples_per_frame_step: f64,
    samples_to_frame_step: f64,
    frame_step: u8,
    channels: [ChannelState; 4],
    master_left: u8,
    master_right: u8,
    pan_mask: u8,
    wave_ram: [u8; 16],
    preview: Option<PreviewState>,
    wave_preview_slot: Option<u8>,
}

impl SoundEngine {
    pub fn new(sample_rate: u32, data: SoundData) -> Self {
        let sample_rate = sample_rate.max(8000);
        let wave_ram = data.waves[0];
        Self {
            data,
            sample_rate,
            samples_per_frame_step: sample_rate as f64 / FRAME_SEQUENCER_RATE,
            samples_to_frame_step: sample_rate as f64 / FRAME_SEQUENCER_RATE,
            frame_step: 0,
            channels: std::array::from_fn(|_| ChannelState::default()),
            master_left: 15,
            master_right: 15,
            pan_mask: 0xFF,
            wave_ram,
            preview: None,
            wave_preview_slot: None,
        }
    }

    pub fn set_data(&mut self, data: SoundData) {
        if let Some(preview) = &mut self.preview {
            if let Some(song) = data.songs.get(preview.song as usize) {
                preview.speed_hz = song.preview_speed.max(1);
            }
        }
        self.data = data;
        if let Some(slot) = self.wave_preview_slot {
            self.wave_ram = self.data.waves[slot.min(15) as usize];
        }
    }

    pub fn set_apu_state(
        &mut self,
        master_left: u8,
        master_right: u8,
        pan_mask: u8,
        wave_ram: [u8; 16],
    ) {
        self.master_left = master_left & 0x0F;
        self.master_right = master_right & 0x0F;
        self.pan_mask = pan_mask;
        self.wave_ram = wave_ram;
    }

    pub fn wave_ram(&self) -> [u8; 16] {
        self.wave_ram
    }

    pub fn is_previewing(&self) -> bool {
        self.preview.is_some() || self.wave_preview_slot.is_some()
    }

    pub fn start_preview(&mut self, song: u8, frame: u16) {
        self.clear_channels();
        let speed = self
            .data
            .songs
            .get(song as usize)
            .map(|song| song.preview_speed.max(1))
            .unwrap_or(60);
        self.preview = Some(PreviewState {
            song,
            frame,
            samples_to_next_frame: 0.0,
            speed_hz: speed,
        });
    }

    pub fn preview_wave(&mut self, slot: u8) {
        self.clear_channels();
        let slot = slot.min(15);
        self.wave_ram = self.data.waves[slot as usize];
        self.wave_preview_slot = Some(slot);
        let channel = &mut self.channels[2];
        channel.note = 34;
        channel.period = wave_period(34).unwrap_or(0);
        channel.volume = 15;
        channel.shape = slot;
        channel.gate = true;
        channel.synth.reset();
    }

    pub fn update_wave_preview(&mut self, slot: u8) {
        if self.wave_preview_slot == Some(slot) {
            self.wave_ram = self.data.waves[slot.min(15) as usize];
        }
    }

    pub fn apply_song_frame(&mut self, song: i16, frame: i16) {
        if song < 0 {
            self.clear_channels();
            return;
        }
        let Some(song_data) = self.data.songs.get(song as usize) else {
            return;
        };
        if song_data.frames.is_empty() {
            return;
        }
        let frame = (frame.max(0) as usize) % song_data.frames.len();
        let cells = song_data.frames[frame];
        for (channel, event) in cells.iter().enumerate() {
            if !event.is_empty() {
                self.apply_event(channel, *event);
            }
        }
    }

    pub fn stop_all(&mut self) {
        self.clear_channels();
    }

    fn clear_channels(&mut self) {
        self.preview = None;
        self.wave_preview_slot = None;
        for channel in &mut self.channels {
            *channel = ChannelState::default();
        }
    }

    fn apply_event(&mut self, channel: usize, event: Event) {
        match channel {
            0 | 1 => self.apply_pulse_event(channel, event),
            2 => self.apply_wave_event(event),
            _ => self.apply_noise_event(event),
        }
    }

    fn apply_pulse_event(&mut self, channel: usize, event: Event) {
        let note = event.param;
        if (1..=NOTE_MAX).contains(&note) {
            let period = pulse_period(note).unwrap_or(0);
            let state = &mut self.channels[channel];
            state.note = note;
            state.period = period;
            state.volume = event.volume();
            state.shape = event.timbre();
            state.gate = true;
            state.synth.reset();
            if channel == 0 {
                state.sweep_pace = event.extra & 0x07;
                state.sweep_dir_up = event.extra & 0x08 == 0;
                state.sweep_step = (event.extra >> 4) & 0x07;
                state.sweep_shadow = period;
                state.sweep_timer = if state.sweep_pace == 0 {
                    8
                } else {
                    state.sweep_pace
                };
                state.sweep_enabled = state.sweep_pace != 0 || state.sweep_step != 0;
            }
            return;
        }
        let state = &mut self.channels[channel];
        if note == NOTE_OFF {
            state.gate = false;
            state.volume = 0;
            return;
        }
        if note == 0 {
            state.volume = event.volume();
            state.shape = event.timbre();
            if channel == 0 {
                state.sweep_pace = event.extra & 0x07;
                state.sweep_dir_up = event.extra & 0x08 == 0;
                state.sweep_step = (event.extra >> 4) & 0x07;
                state.sweep_shadow = state.period;
                state.sweep_timer = if state.sweep_pace == 0 {
                    8
                } else {
                    state.sweep_pace
                };
                state.sweep_enabled = state.sweep_pace != 0 || state.sweep_step != 0;
            }
        }
    }

    fn apply_wave_event(&mut self, event: Event) {
        let note = event.param;
        let slot = event.extra;
        if slot <= 15 {
            self.wave_ram = self.data.waves[slot as usize];
        }
        let state = &mut self.channels[2];
        state.volume = event.volume();
        state.shape = slot;
        if (1..=NOTE_MAX).contains(&note) {
            state.note = note;
            state.period = wave_period(note).unwrap_or(0);
            state.gate = true;
            state.synth.reset();
        } else if note == NOTE_OFF {
            state.gate = false;
            state.volume = 0;
        }
    }

    fn apply_noise_event(&mut self, event: Event) {
        let state = &mut self.channels[3];
        state.volume = event.volume();
        state.shape = event.extra;
        match event.param {
            1 => {
                state.gate = true;
                state.synth.reset();
            }
            2 => {
                state.gate = false;
                state.volume = 0;
            }
            _ => {}
        }
    }

    pub fn advance(&mut self, seconds: f64, out: &mut Vec<f32>) {
        let samples = (seconds.max(0.0) * self.sample_rate as f64).round() as usize;
        self.render_samples(samples, out);
    }

    pub fn render_samples(&mut self, count: usize, out: &mut Vec<f32>) {
        for _ in 0..count {
            self.preview_step();
            if self.samples_to_frame_step <= 0.0 {
                self.frame_step();
                self.samples_to_frame_step += self.samples_per_frame_step;
            }
            self.samples_to_frame_step -= 1.0;
            let (left, right) = self.render_sample();
            out.push(left);
            out.push(right);
        }
    }

    fn preview_step(&mut self) {
        let mut apply: Option<(u8, u16)> = None;
        if let Some(preview) = &mut self.preview {
            preview.samples_to_next_frame -= 1.0;
            if preview.samples_to_next_frame <= 0.0 {
                apply = Some((preview.song, preview.frame));
                preview.frame = preview.frame.wrapping_add(1);
                preview.samples_to_next_frame +=
                    self.sample_rate as f64 / preview.speed_hz.max(1) as f64;
            }
        }
        if let Some((song, frame)) = apply {
            self.apply_song_frame(song as i16, frame as i16);
        }
    }

    fn frame_step(&mut self) {
        let step = self.frame_step;
        if step == 2 || step == 6 {
            self.sweep_step(0);
        }
        self.frame_step = (self.frame_step + 1) & 7;
    }

    fn sweep_step(&mut self, channel: usize) {
        let state = &mut self.channels[channel];
        if !state.sweep_enabled || state.sweep_pace == 0 || !state.gate {
            return;
        }
        if state.sweep_timer > 0 {
            state.sweep_timer -= 1;
        }
        if state.sweep_timer != 0 {
            return;
        }
        state.sweep_timer = state.sweep_pace;
        let delta = state.sweep_shadow >> state.sweep_step;
        let next = if state.sweep_dir_up {
            state.sweep_shadow as i32 + delta as i32
        } else {
            state.sweep_shadow as i32 - delta as i32
        };
        if next > 2047 {
            state.gate = false;
            state.volume = 0;
            return;
        }
        if state.sweep_step != 0 {
            state.sweep_shadow = next as u16;
            state.period = next as u16;
        }
    }

    fn render_sample(&mut self) -> (f32, f32) {
        let mut left = 0.0f32;
        let mut right = 0.0f32;

        for channel in 0..4 {
            let state = &mut self.channels[channel];
            if !state.gate {
                continue;
            }
            let period = state.period.min(2047);
            let raw = match channel {
                0 | 1 => state
                    .synth
                    .pulse_sample(period, state.shape, self.sample_rate),
                2 => state
                    .synth
                    .wave_sample(period, &self.wave_ram, self.sample_rate),
                _ => {
                    let divider = state.shape & 0x07;
                    let shift = (state.shape >> 3) & 0x0F;
                    let width = state.shape & 0x80 != 0;
                    state
                        .synth
                        .noise_sample(divider, shift, width, self.sample_rate)
                }
            };
            let value = raw * ((state.volume as f32 + 1.0) / 16.0);
            if self.pan_mask & (1 << (4 + channel)) != 0 {
                left += value;
            }
            if self.pan_mask & (1 << channel) != 0 {
                right += value;
            }
        }

        let master_left = (self.master_left as f32 + 1.0) / 16.0;
        let master_right = (self.master_right as f32 + 1.0) / 16.0;
        (
            (left * master_left * MASTER_GAIN).clamp(-1.0, 1.0),
            (right * master_right * MASTER_GAIN).clamp(-1.0, 1.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::Song;

    fn channel(engine: &SoundEngine, index: usize) -> &ChannelState {
        &engine.channels[index]
    }

    fn test_data() -> SoundData {
        let mut data = SoundData::default();
        let mut song = Song::new();
        song.frames.push([Event::default(); 4]);
        song.frames[0][0] = Event {
            param: 34,
            volume_timbre: 0xF2,
            extra: 0,
        };
        song.frames.push([Event::default(); 4]);
        song.frames[1][0].param = 73;
        data.songs[0] = song;
        data
    }

    #[test]
    fn applies_pulse_note_and_off() {
        let mut engine = SoundEngine::new(48000, test_data());
        engine.apply_song_frame(0, 0);
        assert_eq!(channel(&engine, 0).period, 1750);
        assert!(channel(&engine, 0).gate);
        engine.apply_song_frame(0, 1);
        assert!(!channel(&engine, 0).gate);
    }

    #[test]
    fn multiple_calls_merge_by_channel() {
        let mut data = test_data();
        let mut other = Song::new();
        other.frames.push([Event::default(); 4]);
        other.frames[0][1].param = 40;
        other.frames[0][1].volume_timbre = 0xF1;
        data.songs[1] = other;
        let mut engine = SoundEngine::new(48000, data);
        engine.apply_song_frame(0, 0);
        engine.apply_song_frame(1, 0);
        assert!(channel(&engine, 0).gate);
        assert!(channel(&engine, 1).gate);
    }

    #[test]
    fn stop_all_clears_channels() {
        let mut engine = SoundEngine::new(48000, test_data());
        engine.apply_song_frame(0, 0);
        engine.apply_song_frame(-1, 0);
        assert!(!channel(&engine, 0).gate);
    }

    #[test]
    fn wave_event_copies_table_and_uses_wave_period() {
        let mut data = test_data();
        data.waves[1][0] = 0x0F;
        let mut song = Song::new();
        song.frames.push([Event::default(); 4]);
        song.frames[0][2].param = 34;
        song.frames[0][2].volume_timbre = 0xF0;
        song.frames[0][2].extra = 1;
        data.songs[1] = song;
        let mut engine = SoundEngine::new(48000, data);
        engine.apply_song_frame(1, 0);
        assert_eq!(channel(&engine, 2).period, 1899);
        assert_eq!(engine.wave_ram()[0], 0x0F);
    }

    #[test]
    fn noise_gate() {
        let mut data = test_data();
        let mut song = Song::new();
        song.frames.push([Event::default(); 4]);
        song.frames[0][3].param = 1;
        song.frames[0][3].volume_timbre = 0xF0;
        song.frames[0][3].extra = 0x81;
        data.songs[1] = song;
        let mut engine = SoundEngine::new(48000, data);
        engine.apply_song_frame(1, 0);
        assert!(channel(&engine, 3).gate);
        assert_eq!(channel(&engine, 3).shape, 0x81);
    }

    #[test]
    fn volume_zero_is_not_muted() {
        let mut data = test_data();
        data.songs[0].frames[0][0].volume_timbre = 0x02;
        let mut engine = SoundEngine::new(48000, data);
        engine.apply_song_frame(0, 0);
        let mut out = Vec::new();
        engine.advance(1.0 / 60.0, &mut out);
        assert!(out.iter().any(|sample| *sample != 0.0));
    }

    #[test]
    fn master_and_panning_are_applied() {
        let mut engine = SoundEngine::new(48000, test_data());
        engine.set_apu_state(0, 15, 0x01, [0u8; 16]);
        engine.apply_song_frame(0, 0);
        let mut out = Vec::new();
        engine.advance(1.0 / 60.0, &mut out);
        let left: f32 = out.iter().step_by(2).map(|s| s.abs()).sum();
        let right: f32 = out.iter().skip(1).step_by(2).map(|s| s.abs()).sum();
        assert_eq!(left, 0.0);
        assert!(right > 0.0);
    }

    #[test]
    fn preview_wave_triggers_wave_channel() {
        let mut data = test_data();
        data.waves[1][0] = 0x0F;
        let mut engine = SoundEngine::new(48000, data);
        engine.preview_wave(1);
        assert!(channel(&engine, 2).gate);
        assert_eq!(engine.wave_ram()[0], 0x0F);
    }

    #[test]
    fn preview_song_applies_frame_zero() {
        let mut engine = SoundEngine::new(48000, test_data());
        engine.start_preview(0, 0);
        let mut out = Vec::new();
        engine.render_samples(1, &mut out);
        assert!(channel(&engine, 0).gate);
    }

    #[test]
    fn render_produces_stereo_samples() {
        let mut engine = SoundEngine::new(48000, test_data());
        engine.apply_song_frame(0, 0);
        let mut out = Vec::new();
        engine.advance(1.0 / 60.0, &mut out);
        assert_eq!(out.len(), 800 * 2);
        assert!(out.iter().any(|sample| *sample != 0.0));
    }
}
