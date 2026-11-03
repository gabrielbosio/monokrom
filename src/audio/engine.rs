use crate::audio::format::{ChannelType, Instrument, MusicCell, SfxEntry, SoundData};
use crate::audio::notes::{pulse_period, wave_period, NOTE_MAX, NOTE_MIN, NOTE_OFF};
use crate::audio::synth::ChannelSynth;
use crate::config::{
    AUDIO_INSTRUMENT_COUNT, AUDIO_TABLE_COUNT, AUDIO_TABLE_STEPS, AUDIO_WAVE_COUNT, MUSIC_CHANNELS,
    MUSIC_ROWS, SONG_ORDER_COUNT, SONG_PATTERN_COUNT,
};

pub const TICK_RATE: f64 = 60.0;
pub const FRAME_SEQUENCER_RATE: f64 = 512.0;
pub const DEFAULT_SAMPLE_RATE: u32 = 48000;
pub const MASTER_GAIN: f32 = 0.25;

pub const EFFECT_NONE: u8 = 0;
pub const EFFECT_ARPEGGIO: u8 = 1;
pub const EFFECT_PORTA_UP: u8 = 2;
pub const EFFECT_PORTA_DOWN: u8 = 3;
pub const EFFECT_TONE_PORTA: u8 = 4;
pub const EFFECT_VIBRATO: u8 = 5;
pub const EFFECT_NOTE_DELAY: u8 = 6;
pub const EFFECT_NOTE_CUT: u8 = 7;
pub const EFFECT_RETRIGGER: u8 = 8;
pub const EFFECT_SET_VOLUME: u8 = 9;
pub const EFFECT_SET_SHAPE: u8 = 10;
pub const EFFECT_TABLE: u8 = 11;
pub const EFFECT_SET_SPEED: u8 = 12;
pub const EFFECT_SET_PANNING: u8 = 13;
pub const EFFECT_SET_MASTER_VOLUME: u8 = 14;

const TABLE_COMMAND_END: u8 = 1;
const TABLE_COMMAND_LOOP: u8 = 2;
const TABLE_COMMAND_RESTART: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChannelOwner {
    None,
    Music,
    Sfx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TableState {
    index: u8,
    step: u8,
    tick: u8,
}

struct ChannelState {
    owner: ChannelOwner,
    instrument_index: u8,
    instrument: Instrument,
    note: u8,
    base_note: u8,
    period: u16,
    detune: i16,
    volume: u8,
    shape: u8,
    table: Option<TableState>,
    effect: u8,
    effect_value: u8,
    effect_tick: u8,
    tone_porta: Option<(u8, u16)>,
    pending_note: Option<(u8, u8)>,
    priority: u8,
    sfx_remaining: Option<u8>,
    sfx_age: u32,
    gate: bool,
    left: bool,
    right: bool,
    envelope_volume: u8,
    envelope_timer: u8,
    sweep_timer: u8,
    sweep_shadow: u16,
    sweep_enabled: bool,
    length_remaining: u16,
    synth: ChannelSynth,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self {
            owner: ChannelOwner::None,
            instrument_index: 0,
            instrument: Instrument::default(),
            note: 0,
            base_note: 0,
            period: 0,
            detune: 0,
            volume: 0,
            shape: 0,
            table: None,
            effect: EFFECT_NONE,
            effect_value: 0,
            effect_tick: 0,
            tone_porta: None,
            pending_note: None,
            priority: 0,
            sfx_remaining: None,
            sfx_age: 0,
            gate: false,
            left: true,
            right: true,
            envelope_volume: 0,
            envelope_timer: 0,
            sweep_timer: 0,
            sweep_shadow: 0,
            sweep_enabled: false,
            length_remaining: 0,
            synth: ChannelSynth::new(),
        }
    }
}

pub struct SoundEngine {
    data: SoundData,
    sample_rate: u32,
    samples_per_tick: f64,
    samples_to_tick: f64,
    samples_per_frame_step: f64,
    samples_to_frame_step: f64,
    frame_step: u8,
    channels: [ChannelState; MUSIC_CHANNELS],
    last_music_instrument: [u8; MUSIC_CHANNELS],
    song: Option<u8>,
    order: u8,
    row: u8,
    tick: u8,
    ticks_per_row: u8,
    order_count: u8,
    loop_start: u8,
    loop_end: u8,
    loop_enabled: bool,
    master_volume: u8,
    sfx_age: u32,
}

impl SoundEngine {
    pub fn new(sample_rate: u32, data: SoundData) -> Self {
        let sample_rate = sample_rate.max(8000);
        Self {
            data,
            sample_rate,
            samples_per_tick: sample_rate as f64 / TICK_RATE,
            samples_to_tick: 0.0,
            samples_per_frame_step: sample_rate as f64 / FRAME_SEQUENCER_RATE,
            samples_to_frame_step: sample_rate as f64 / FRAME_SEQUENCER_RATE,
            frame_step: 0,
            channels: std::array::from_fn(|_| ChannelState::default()),
            last_music_instrument: [1; MUSIC_CHANNELS],
            song: None,
            order: 0,
            row: 0,
            tick: 0,
            ticks_per_row: 6,
            order_count: 0,
            loop_start: 0,
            loop_end: 0,
            loop_enabled: false,
            master_volume: 15,
            sfx_age: 0,
        }
    }

    pub fn set_data(&mut self, data: SoundData) {
        self.data = data;
    }

    pub fn play_song(&mut self, index: u8) -> bool {
        let Some(song) = self.data.songs.get(index as usize).copied() else {
            self.stop_song();
            return false;
        };
        if song.loop_end == 0
            || song.loop_end > self.data.header.order_count
            || song.order_start >= song.loop_end
        {
            self.stop_song();
            return false;
        }
        if song.loop_enabled && song.loop_start >= song.loop_end {
            self.stop_song();
            return false;
        }
        self.song = Some(index);
        self.order = song.order_start;
        self.row = 0;
        self.tick = 0;
        self.ticks_per_row = self.data.header.ticks_per_row.max(1);
        self.order_count = self.data.header.order_count.min(SONG_ORDER_COUNT as u8);
        self.loop_start = song.loop_start;
        self.loop_end = song.loop_end;
        self.loop_enabled = song.loop_enabled;
        self.samples_to_tick = 0.0;
        for channel in &mut self.channels {
            if channel.owner == ChannelOwner::Music {
                *channel = ChannelState::default();
            }
        }
        true
    }

    pub fn play_song_from(&mut self, index: u8, order: u8) -> bool {
        if !self.play_song(index) {
            return false;
        }
        if order >= self.loop_end {
            return true;
        }
        self.order = order;
        self.row = 0;
        self.tick = 0;
        self.samples_to_tick = 0.0;
        true
    }

    pub fn stop_song(&mut self) {
        self.song = None;
        for channel in &mut self.channels {
            if channel.owner == ChannelOwner::Music {
                *channel = ChannelState::default();
            }
        }
    }

    pub fn stop_all(&mut self) {
        self.song = None;
        for channel in &mut self.channels {
            *channel = ChannelState::default();
        }
    }

    pub fn preview_instrument(&mut self, instrument_index: u8, note: u8) {
        if instrument_index == 0 || instrument_index as usize >= AUDIO_INSTRUMENT_COUNT {
            return;
        }
        let instrument = self.data.instruments[instrument_index as usize];
        let channel = self.preview_channel(instrument.channel);
        self.trigger_note_instrument(
            channel,
            note,
            instrument,
            instrument_index,
            instrument.table,
            ChannelOwner::Sfx,
        );
        self.channels[channel].priority = 3;
        self.channels[channel].sfx_remaining = Some(30);
        self.channels[channel].sfx_age = self.sfx_age;
        self.sfx_age = self.sfx_age.wrapping_add(1);
    }

    pub fn preview_table(&mut self, table: u8) {
        if table == 0 || table as usize >= AUDIO_TABLE_COUNT {
            return;
        }
        let instrument = Instrument {
            channel: ChannelType::Pulse,
            volume: 15,
            duty: 2,
            panning: 0b11,
            table,
            ..Instrument::default()
        };
        let channel = 1;
        self.trigger_note_instrument(channel, 34, instrument, 0, table, ChannelOwner::Sfx);
        self.channels[channel].priority = 3;
        self.channels[channel].sfx_remaining = Some(60);
        self.channels[channel].sfx_age = self.sfx_age;
        self.sfx_age = self.sfx_age.wrapping_add(1);
    }

    pub fn preview_wave(&mut self, wave_slot: u8) {
        let instrument = Instrument {
            channel: ChannelType::Wave,
            volume: 15,
            wave_slot: wave_slot.min(AUDIO_WAVE_COUNT as u8 - 1),
            wave_volume: 3,
            panning: 0b11,
            ..Instrument::default()
        };
        let channel = 2;
        self.trigger_note_instrument(channel, 34, instrument, 0, 0, ChannelOwner::Sfx);
        self.channels[channel].priority = 3;
        self.channels[channel].sfx_remaining = Some(30);
        self.channels[channel].sfx_age = self.sfx_age;
        self.sfx_age = self.sfx_age.wrapping_add(1);
    }

    fn preview_channel(&self, channel_type: ChannelType) -> usize {
        match channel_type {
            ChannelType::Pulse => {
                if self.channels[1].owner != ChannelOwner::Music {
                    1
                } else {
                    0
                }
            }
            ChannelType::Wave => 2,
            ChannelType::Noise => 3,
        }
    }

    pub fn trigger_sfx(&mut self, index: u8) {
        let Some(entry) = self.data.sfx.get(index as usize).copied() else {
            return;
        };
        if !(NOTE_MIN..=NOTE_MAX).contains(&entry.note) {
            return;
        }
        let instrument_index = entry.instrument;
        if instrument_index == 0 || instrument_index as usize >= AUDIO_INSTRUMENT_COUNT {
            return;
        }
        let instrument = self.data.instruments[instrument_index as usize];
        if instrument.channel != entry.channel {
            return;
        }

        let mut candidates: Vec<usize> = match entry.channel {
            ChannelType::Pulse => vec![0, 1],
            ChannelType::Wave => vec![2],
            ChannelType::Noise => vec![3],
        };

        // Prefer a free channel.
        if let Some(&channel) = candidates
            .iter()
            .find(|&&channel| self.channels[channel].owner == ChannelOwner::None)
        {
            self.start_sfx(channel, entry, instrument_index);
            return;
        }

        // Replace a lower priority SFX.
        let sfx_candidate = candidates
            .iter()
            .copied()
            .filter(|&channel| self.channels[channel].owner == ChannelOwner::Sfx)
            .filter(|&channel| self.channels[channel].priority <= entry.priority)
            .min_by_key(|&channel| {
                (
                    self.channels[channel].priority,
                    self.channels[channel].sfx_age,
                )
            });
        if let Some(channel) = sfx_candidate {
            self.start_sfx(channel, entry, instrument_index);
            return;
        }

        // Steal a music channel only when the SFX has priority.
        if entry.priority > 0 {
            if entry.channel == ChannelType::Pulse {
                candidates.reverse();
            }
            for channel in candidates {
                if self.channels[channel].owner == ChannelOwner::Music {
                    self.start_sfx(channel, entry, instrument_index);
                    return;
                }
            }
        }
    }

    fn start_sfx(&mut self, channel: usize, entry: SfxEntry, instrument_index: u8) {
        let table = if entry.table != 0 {
            entry.table
        } else {
            self.data.instruments[instrument_index as usize].table
        };
        self.trigger_note(
            channel,
            entry.note,
            instrument_index,
            table,
            ChannelOwner::Sfx,
        );
        let state = &mut self.channels[channel];
        state.priority = entry.priority & 0x03;
        state.sfx_remaining = if entry.duration == 0 {
            None
        } else {
            Some(entry.duration)
        };
        state.sfx_age = self.sfx_age;
        self.sfx_age = self.sfx_age.wrapping_add(1);
    }

    pub fn tick(&mut self) {
        self.expire_sfx();
        self.process_tables();

        if self.song.is_some() {
            if self.tick == 0 {
                self.process_row();
            }
            self.process_effects();
            self.tick = self.tick.wrapping_add(1);
            if self.tick >= self.ticks_per_row {
                self.tick = 0;
                self.advance_row();
            }
        }
    }

    pub fn advance(&mut self, seconds: f64, out: &mut Vec<f32>) {
        let samples = (seconds.max(0.0) * self.sample_rate as f64).round() as usize;
        let mut remaining = samples;
        while remaining > 0 {
            if self.samples_to_tick <= 0.0 {
                self.tick();
                self.samples_to_tick += self.samples_per_tick;
            }
            let chunk = remaining
                .min(self.samples_to_tick.ceil().max(1.0) as usize)
                .max(1);
            self.render_samples(chunk, out);
            self.samples_to_tick -= chunk as f64;
            remaining -= chunk;
        }
    }

    pub fn render_samples(&mut self, count: usize, out: &mut Vec<f32>) {
        for _ in 0..count {
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

    fn expire_sfx(&mut self) {
        for channel in &mut self.channels {
            if channel.owner != ChannelOwner::Sfx {
                continue;
            }
            match channel.sfx_remaining {
                Some(0) => {
                    *channel = ChannelState::default();
                }
                Some(remaining) => channel.sfx_remaining = Some(remaining - 1),
                None => {}
            }
        }
    }

    fn process_row(&mut self) {
        let order = self.order as usize;
        if order >= SONG_ORDER_COUNT {
            return;
        }
        let patterns = self.data.orders[order].patterns;

        for (ch, &pattern) in patterns.iter().enumerate() {
            let pattern = pattern as usize;
            if pattern >= SONG_PATTERN_COUNT {
                continue;
            }
            let cell = self.data.patterns[ch][pattern][self.row as usize];

            if cell.instrument != 0 && self.channels[ch].owner == ChannelOwner::Music {
                self.last_music_instrument[ch] = cell.instrument;
            }

            if cell.note == NOTE_OFF {
                if self.channels[ch].owner == ChannelOwner::Music {
                    self.release_channel(ch);
                }
                continue;
            }

            if (NOTE_MIN..=NOTE_MAX).contains(&cell.note) {
                let instrument_index = if cell.instrument != 0 {
                    cell.instrument
                } else {
                    self.last_music_instrument[ch]
                }
                .min(AUDIO_INSTRUMENT_COUNT as u8 - 1);
                self.last_music_instrument[ch] = instrument_index;

                if cell.effect == EFFECT_TONE_PORTA
                    && self.channels[ch].owner == ChannelOwner::Music
                    && self.channels[ch].gate
                {
                    if let Some(target_period) = self.note_period(ch, cell.note) {
                        self.channels[ch].tone_porta = Some((cell.note, target_period));
                        self.set_effect(ch, cell);
                        continue;
                    }
                }

                if cell.effect == EFFECT_NOTE_DELAY && cell.value > 0 {
                    self.channels[ch].effect = cell.effect;
                    self.channels[ch].effect_value = cell.value;
                    self.channels[ch].effect_tick = 0;
                    self.channels[ch].pending_note = Some((cell.note, instrument_index));
                    continue;
                }

                let instrument = self.data.instruments[instrument_index as usize];
                self.trigger_note(
                    ch,
                    cell.note,
                    instrument_index,
                    instrument.table,
                    ChannelOwner::Music,
                );
                self.set_effect(ch, cell);
                continue;
            }

            if self.channels[ch].owner == ChannelOwner::Music {
                self.set_effect(ch, cell);
            }
        }
    }

    fn set_effect(&mut self, ch: usize, cell: MusicCell) {
        self.channels[ch].effect = cell.effect;
        self.channels[ch].effect_value = cell.value;
        self.channels[ch].effect_tick = 0;
        if cell.effect != EFFECT_TONE_PORTA {
            self.channels[ch].tone_porta = None;
        }
    }

    fn process_effects(&mut self) {
        for ch in 0..MUSIC_CHANNELS {
            let waiting_for_delayed_note = self.channels[ch].effect == EFFECT_NOTE_DELAY
                && self.channels[ch].pending_note.is_some();
            if self.channels[ch].owner != ChannelOwner::Music && !waiting_for_delayed_note {
                continue;
            }
            let effect = self.channels[ch].effect;
            let value = self.channels[ch].effect_value;
            let tick = self.channels[ch].effect_tick;

            match effect {
                EFFECT_ARPEGGIO => {
                    let offset = match tick % 3 {
                        0 => 0,
                        1 => value & 0x0F,
                        _ => value >> 4,
                    };
                    let note = (self.channels[ch].note as u16 + offset as u16).min(72) as u8;
                    if let Some(period) = self.note_period(ch, note) {
                        self.channels[ch].period = period;
                    }
                }
                EFFECT_PORTA_UP => {
                    let period = self.channels[ch].period as i32 + value.max(1) as i32;
                    self.channels[ch].period = period.clamp(0, 2047) as u16;
                }
                EFFECT_PORTA_DOWN => {
                    let period = self.channels[ch].period as i32 - value.max(1) as i32;
                    self.channels[ch].period = period.clamp(0, 2047) as u16;
                }
                EFFECT_TONE_PORTA => {
                    if let Some((target_note, target_period)) = self.channels[ch].tone_porta {
                        let speed = value.max(1) as i32;
                        let current = self.channels[ch].period as i32;
                        let target = target_period as i32;
                        let next = if current < target {
                            (current + speed).min(target)
                        } else {
                            (current - speed).max(target)
                        };
                        self.channels[ch].period = next.clamp(0, 2047) as u16;
                        if next == target {
                            self.channels[ch].note = target_note;
                            self.channels[ch].base_note = target_note;
                            self.channels[ch].tone_porta = None;
                        }
                    }
                }
                EFFECT_VIBRATO => {
                    let speed = (value >> 4).max(1);
                    let depth = value & 0x0F;
                    let phase = tick % (speed * 2);
                    let offset = if phase < speed {
                        depth as i32
                    } else {
                        -(depth as i32)
                    };
                    let note = self.channels[ch].note;
                    if let Some(base) = self.note_period(ch, note) {
                        self.channels[ch].period = (base as i32 + offset).clamp(0, 2047) as u16;
                    }
                }
                EFFECT_NOTE_DELAY => {
                    if tick == value {
                        if let Some((note, instrument_index)) =
                            self.channels[ch].pending_note.take()
                        {
                            let instrument = self.data.instruments[instrument_index as usize];
                            self.trigger_note(
                                ch,
                                note,
                                instrument_index,
                                instrument.table,
                                ChannelOwner::Music,
                            );
                        }
                    }
                }
                EFFECT_NOTE_CUT => {
                    if tick == value {
                        self.channels[ch].gate = false;
                        self.channels[ch].volume = 0;
                    }
                }
                EFFECT_RETRIGGER => {
                    if value > 0 && tick > 0 && tick.is_multiple_of(value) {
                        self.retrigger_channel(ch);
                    }
                }
                EFFECT_SET_VOLUME if tick == 0 => {
                    self.channels[ch].volume = value.min(15);
                }
                EFFECT_SET_SHAPE if tick == 0 => {
                    self.channels[ch].shape = value & 0x0F;
                }
                EFFECT_TABLE if tick == 0 => {
                    if value == 0 {
                        self.channels[ch].table = None;
                    } else {
                        self.start_table(ch, value);
                    }
                }
                EFFECT_SET_SPEED if tick == 0 => {
                    self.ticks_per_row = value.max(1);
                }
                EFFECT_SET_PANNING if tick == 0 => {
                    self.channels[ch].left = value & 1 != 0;
                    self.channels[ch].right = value & 2 != 0;
                }
                EFFECT_SET_MASTER_VOLUME if tick == 0 => {
                    self.master_volume = value.min(15);
                }
                _ => {}
            }

            self.channels[ch].effect_tick = tick.saturating_add(1);
        }
    }

    fn process_tables(&mut self) {
        for ch in 0..MUSIC_CHANNELS {
            if self.channels[ch].table.is_some() {
                self.advance_table(ch);
            }
        }
    }

    fn start_table(&mut self, ch: usize, index: u8) {
        if index == 0 || index as usize >= AUDIO_TABLE_COUNT {
            self.channels[ch].table = None;
            return;
        }
        self.channels[ch].table = Some(TableState {
            index,
            step: 0,
            tick: 0,
        });
        self.enter_table_step(ch, 0);
    }

    fn advance_table(&mut self, ch: usize) {
        let Some(mut state) = self.channels[ch].table else {
            return;
        };
        let speed = self.data.tables[state.index as usize].speed.max(1);
        state.tick = state.tick.saturating_add(1);
        if state.tick < speed {
            self.channels[ch].table = Some(state);
            return;
        }
        state.tick = 0;
        self.enter_table_step(ch, state.step.saturating_add(1));
    }

    fn enter_table_step(&mut self, ch: usize, mut step: u8) {
        let Some(state) = self.channels[ch].table else {
            return;
        };
        let table = self.data.tables[state.index as usize];
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > AUDIO_TABLE_STEPS + 1 {
                self.channels[ch].table = None;
                return;
            }
            if step as usize >= AUDIO_TABLE_STEPS {
                if table.loop_enabled && table.loop_end > table.loop_start {
                    step = table.loop_start.min(AUDIO_TABLE_STEPS as u8 - 1);
                } else {
                    self.channels[ch].table = None;
                    self.on_table_end(ch);
                    return;
                }
            }
            let command = table.steps[step as usize].command;
            match command {
                TABLE_COMMAND_END => {
                    self.channels[ch].table = None;
                    self.on_table_end(ch);
                    return;
                }
                TABLE_COMMAND_LOOP => {
                    step = table.loop_start.min(AUDIO_TABLE_STEPS as u8 - 1);
                }
                TABLE_COMMAND_RESTART => {
                    step = 0;
                }
                _ => break,
            }
        }
        self.channels[ch].table = Some(TableState {
            index: state.index,
            step,
            tick: 0,
        });
        self.apply_table_step(ch);
    }

    fn apply_table_step(&mut self, ch: usize) {
        let Some(state) = self.channels[ch].table else {
            return;
        };
        let table = self.data.tables[state.index as usize];
        let step = table.steps[state.step as usize];
        self.channels[ch].volume = step.volume.min(15);
        if step.shape != 0 {
            self.channels[ch].shape = step.shape;
        }
        if self.channel_type(ch) != ChannelType::Noise {
            let note = (self.channels[ch].note as i16 + step.pitch as i16)
                .clamp(NOTE_MIN as i16, NOTE_MAX as i16) as u8;
            self.channels[ch].base_note = note;
            if let Some(period) = self.note_period(ch, note) {
                self.channels[ch].period = period;
            }
            let detune = self.channels[ch].period as i32 * step.detune as i32 / 1600;
            self.channels[ch].detune = self.channels[ch].instrument.finetune as i16 + detune as i16;
        }
    }

    fn on_table_end(&mut self, ch: usize) {
        if self.channels[ch].owner == ChannelOwner::Sfx {
            self.release_channel(ch);
        }
    }

    fn advance_row(&mut self) {
        for channel in &mut self.channels {
            if channel.effect == EFFECT_NOTE_DELAY {
                channel.effect = EFFECT_NONE;
                channel.effect_value = 0;
                channel.effect_tick = 0;
                channel.pending_note = None;
            }
        }
        self.row = self.row.wrapping_add(1);
        if self.row < MUSIC_ROWS as u8 {
            return;
        }
        self.row = 0;
        self.order = self.order.wrapping_add(1);
        if self.order >= self.loop_end {
            if self.loop_enabled {
                self.order = self.loop_start;
            } else {
                self.stop_song();
            }
        } else if self.order >= self.order_count {
            self.stop_song();
        }
    }

    fn trigger_note(
        &mut self,
        ch: usize,
        note: u8,
        instrument_index: u8,
        table_override: u8,
        owner: ChannelOwner,
    ) {
        if instrument_index == 0 || instrument_index as usize >= AUDIO_INSTRUMENT_COUNT {
            return;
        }
        let instrument = self.data.instruments[instrument_index as usize];
        self.trigger_note_instrument(
            ch,
            note,
            instrument,
            instrument_index,
            table_override,
            owner,
        );
    }

    fn trigger_note_instrument(
        &mut self,
        ch: usize,
        note: u8,
        instrument: Instrument,
        instrument_index: u8,
        table_override: u8,
        owner: ChannelOwner,
    ) {
        if ch >= MUSIC_CHANNELS || !(NOTE_MIN..=NOTE_MAX).contains(&note) {
            return;
        }
        if instrument.channel != self.channel_type(ch) {
            return;
        }
        let period = self.note_period(ch, note).unwrap_or(0);
        let channel_type = self.channel_type(ch);
        let channel = &mut self.channels[ch];
        channel.owner = owner;
        channel.instrument_index = instrument_index;
        channel.instrument = instrument;
        channel.note = note;
        channel.base_note = note;
        channel.period = period;
        channel.detune = instrument.finetune as i16;
        channel.volume = if channel_type == ChannelType::Wave {
            instrument.wave_volume * 4
        } else {
            instrument.volume
        };
        channel.shape = match channel_type {
            ChannelType::Pulse => instrument.duty,
            ChannelType::Wave => instrument.wave_slot,
            ChannelType::Noise => {
                ((instrument.noise_width_7bit as u8) << 3) | instrument.noise_divider
            }
        };
        channel.envelope_volume = instrument.volume;
        channel.envelope_timer = instrument.envelope_period;
        channel.sweep_timer = if instrument.sweep_pace == 0 {
            8
        } else {
            instrument.sweep_pace
        };
        channel.sweep_shadow = period;
        channel.sweep_enabled = instrument.sweep_pace != 0 || instrument.sweep_step != 0;
        channel.length_remaining = length_remaining(ch, &instrument);
        channel.left = instrument.panning & 1 != 0;
        channel.right = instrument.panning & 2 != 0;
        channel.effect = EFFECT_NONE;
        channel.effect_value = 0;
        channel.effect_tick = 0;
        channel.tone_porta = None;
        channel.pending_note = None;
        channel.table = None;
        channel.gate = true;
        channel.synth.reset();

        let table = if table_override != 0 {
            table_override
        } else {
            instrument.table
        };
        if table != 0 {
            self.start_table(ch, table);
        }
    }

    fn retrigger_channel(&mut self, ch: usize) {
        let instrument = self.channels[ch].instrument;
        self.channels[ch].volume = if self.channel_type(ch) == ChannelType::Wave {
            instrument.wave_volume * 4
        } else {
            instrument.volume
        };
        self.channels[ch].envelope_volume = instrument.volume;
        self.channels[ch].envelope_timer = instrument.envelope_period;
        self.channels[ch].sweep_timer = if instrument.sweep_pace == 0 {
            8
        } else {
            instrument.sweep_pace
        };
        self.channels[ch].sweep_shadow = self.channels[ch].period;
        self.channels[ch].sweep_enabled = instrument.sweep_pace != 0 || instrument.sweep_step != 0;
        self.channels[ch].length_remaining = length_remaining(ch, &instrument);
        self.channels[ch].tone_porta = None;
        self.channels[ch].gate = true;
        self.channels[ch].synth.reset();
    }

    fn release_channel(&mut self, ch: usize) {
        self.channels[ch] = ChannelState::default();
    }

    fn note_period(&self, ch: usize, note: u8) -> Option<u16> {
        match self.channel_type(ch) {
            ChannelType::Pulse => pulse_period(note),
            ChannelType::Wave => wave_period(note),
            ChannelType::Noise => Some(0),
        }
    }

    fn channel_type(&self, ch: usize) -> ChannelType {
        match ch {
            0 | 1 => ChannelType::Pulse,
            2 => ChannelType::Wave,
            _ => ChannelType::Noise,
        }
    }

    fn frame_step(&mut self) {
        let step = self.frame_step;
        for ch in 0..MUSIC_CHANNELS {
            if !self.channels[ch].gate {
                continue;
            }
            if step.is_multiple_of(2) {
                let channel = &mut self.channels[ch];
                if channel.instrument.length_enabled && channel.length_remaining > 0 {
                    channel.length_remaining -= 1;
                    if channel.length_remaining == 0 {
                        channel.gate = false;
                        channel.volume = 0;
                    }
                }
            }
            if (step == 2 || step == 6) && ch == 0 {
                self.sweep_step(ch);
            }
            if step == 7 {
                self.envelope_step(ch);
            }
        }
        self.frame_step = (self.frame_step + 1) & 7;
    }

    fn envelope_step(&mut self, ch: usize) {
        if self.channels[ch].table.is_some() {
            return;
        }
        let instrument = self.channels[ch].instrument;
        if instrument.envelope_period == 0 {
            return;
        }
        if self.channel_type(ch) == ChannelType::Wave {
            return;
        }
        let channel = &mut self.channels[ch];
        if channel.envelope_timer > 0 {
            channel.envelope_timer -= 1;
        }
        if channel.envelope_timer == 0 {
            channel.envelope_timer = instrument.envelope_period;
            if instrument.envelope_dir_up {
                channel.envelope_volume = (channel.envelope_volume + 1).min(15);
            } else {
                channel.envelope_volume = channel.envelope_volume.saturating_sub(1);
            }
            channel.volume = channel.envelope_volume;
        }
    }

    fn sweep_step(&mut self, ch: usize) {
        let instrument = self.channels[ch].instrument;
        if !self.channels[ch].sweep_enabled || instrument.sweep_pace == 0 {
            return;
        }
        let channel = &mut self.channels[ch];
        if channel.sweep_timer > 0 {
            channel.sweep_timer -= 1;
        }
        if channel.sweep_timer != 0 {
            return;
        }
        channel.sweep_timer = instrument.sweep_pace;
        let delta = channel.sweep_shadow >> instrument.sweep_step;
        let next = if instrument.sweep_dir_up {
            channel.sweep_shadow as i32 + delta as i32
        } else {
            channel.sweep_shadow as i32 - delta as i32
        };
        if next > 2047 {
            channel.gate = false;
            channel.volume = 0;
            return;
        }
        if instrument.sweep_step != 0 {
            channel.sweep_shadow = next as u16;
            channel.period = next as u16;
        }
    }

    fn render_sample(&mut self) -> (f32, f32) {
        let mut left = 0.0f32;
        let mut right = 0.0f32;

        for ch in 0..MUSIC_CHANNELS {
            if !self.channels[ch].gate || self.channels[ch].volume == 0 {
                continue;
            }
            let period = (self.channels[ch].period as i32 + self.channels[ch].detune as i32)
                .clamp(0, 2047) as u16;
            let raw = match self.channel_type(ch) {
                ChannelType::Pulse => self.channels[ch].synth.pulse_sample(
                    period,
                    self.channels[ch].shape,
                    self.sample_rate,
                ),
                ChannelType::Wave => {
                    let slot = (self.channels[ch].shape as usize).min(AUDIO_WAVE_COUNT - 1);
                    let wave = self.data.waves[slot];
                    self.channels[ch]
                        .synth
                        .wave_sample(period, &wave, self.sample_rate)
                }
                ChannelType::Noise => {
                    let noise_shape = self.channels[ch].shape;
                    self.channels[ch].synth.noise_sample(
                        noise_shape & 0x07,
                        self.channels[ch].instrument.noise_shift,
                        noise_shape & 0x08 != 0,
                        self.sample_rate,
                    )
                }
            };
            let scale = if self.channel_type(ch) == ChannelType::Wave {
                wave_volume_scale(self.channels[ch].volume)
            } else {
                self.channels[ch].volume as f32 / 15.0
            };
            let value = raw * scale;
            if self.channels[ch].left {
                left += value;
            }
            if self.channels[ch].right {
                right += value;
            }
        }

        let master = self.master_volume as f32 / 15.0 * MASTER_GAIN;
        (
            (left * master).clamp(-1.0, 1.0),
            (right * master).clamp(-1.0, 1.0),
        )
    }

    #[cfg(test)]
    fn channel(&self, index: usize) -> &ChannelState {
        &self.channels[index]
    }
}

fn wave_volume_scale(volume: u8) -> f32 {
    match volume {
        0 => 0.0,
        1..=4 => 0.25,
        5..=9 => 0.5,
        _ => 1.0,
    }
}

fn length_remaining(ch: usize, instrument: &Instrument) -> u16 {
    if ch == 2 {
        (256u16 - instrument.length as u16).max(1)
    } else {
        (64u16 - (instrument.length as u16 & 0x3F)).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::{MusicCell, Order, Song, Table, TableStep};
    use crate::audio::notes::note_parts;
    use crate::config::{AUDIO_SFX_SIZE, MUSIC_ROWS};

    fn test_data() -> SoundData {
        let mut data = SoundData::default();
        data.header.ticks_per_row = 1;
        data.header.order_count = 2;
        data.instruments[1] = Instrument {
            channel: ChannelType::Pulse,
            volume: 15,
            duty: 2,
            panning: 0b11,
            ..Instrument::default()
        };
        data.instruments[2] = Instrument {
            channel: ChannelType::Wave,
            volume: 15,
            wave_slot: 1,
            wave_volume: 3,
            panning: 0b11,
            ..Instrument::default()
        };
        data.instruments[3] = Instrument {
            channel: ChannelType::Noise,
            volume: 15,
            panning: 0b11,
            ..Instrument::default()
        };
        data.waves[1][0] = 0x0F;
        data.songs[0] = Song {
            order_start: 0,
            loop_start: 0,
            loop_end: 2,
            loop_enabled: false,
        };
        data.orders[0].patterns = [1, 0, 0, 0];
        data.orders[1].patterns = [0, 0, 0, 0];
        data.patterns[0][1][0] = MusicCell {
            note: 34,
            instrument: 1,
            effect: 0,
            value: 0,
        };
        data
    }

    #[test]
    fn plays_a_note_and_advances_rows() {
        let mut engine = SoundEngine::new(48000, test_data());
        assert!(engine.play_song(0));
        engine.tick();
        assert_eq!(engine.channel(0).owner, ChannelOwner::Music);
        assert_eq!(engine.channel(0).period, 1750);
        assert!(engine.channel(0).gate);
        assert_eq!(engine.row, 1);
    }

    #[test]
    fn song_stops_at_end_without_loop() {
        let mut engine = SoundEngine::new(48000, test_data());
        assert!(engine.play_song(0));
        for _ in 0..MUSIC_ROWS * 2 {
            engine.tick();
        }
        assert!(engine.song.is_none());
    }

    #[test]
    fn song_loops() {
        let mut data = test_data();
        data.songs[0].loop_enabled = true;
        data.songs[0].loop_start = 0;
        let mut engine = SoundEngine::new(48000, data);
        assert!(engine.play_song(0));
        for _ in 0..MUSIC_ROWS * 2 {
            engine.tick();
        }
        assert!(engine.song.is_some());
        assert_eq!(engine.order, 0);
    }

    #[test]
    fn note_delay_is_cancelled_at_row_end() {
        let mut data = test_data();
        data.header.ticks_per_row = 6;
        data.patterns[0][1][0] = MusicCell {
            note: 34,
            instrument: 1,
            effect: EFFECT_NOTE_DELAY,
            value: 6,
        };
        let mut engine = SoundEngine::new(48000, data);
        assert!(engine.play_song(0));
        for _ in 0..12 {
            engine.tick();
        }
        assert_eq!(engine.channel(0).owner, ChannelOwner::None);
        assert!(!engine.channel(0).gate);
    }

    #[test]
    fn tone_porta_slides_to_note_without_retrigger() {
        let mut data = test_data();
        data.header.ticks_per_row = 32;
        data.patterns[0][1][1] = MusicCell {
            note: 46,
            instrument: 1,
            effect: EFFECT_TONE_PORTA,
            value: 8,
        };
        let mut engine = SoundEngine::new(48000, data);
        assert!(engine.play_song(0));
        for _ in 0..32 {
            engine.tick();
        }
        engine.tick();
        assert_eq!(engine.channel(0).period, 1758);
        assert_eq!(engine.channel(0).note, 34);
        for _ in 0..20 {
            engine.tick();
        }
        assert_eq!(engine.channel(0).period, 1899);
        assert_eq!(engine.channel(0).note, 46);
    }

    #[test]
    fn table_controls_volume() {
        let mut data = test_data();
        data.tables[1] = Table {
            speed: 1,
            loop_start: 0,
            loop_end: 2,
            ..Table::default()
        };
        data.tables[1].steps[0] = TableStep {
            pitch: 0,
            volume: 15,
            shape: 0,
            detune: 0,
            command: 0,
        };
        data.tables[1].steps[1] = TableStep {
            pitch: 0,
            volume: 0,
            shape: 0,
            detune: 0,
            command: 0,
        };
        data.instruments[1].table = 1;
        let mut engine = SoundEngine::new(48000, data);
        assert!(engine.play_song(0));
        engine.tick();
        assert_eq!(engine.channel(0).volume, 15);
        engine.tick();
        assert_eq!(engine.channel(0).volume, 0);
    }

    #[test]
    fn sfx_steals_a_pulse_channel() {
        let mut data = test_data();
        data.sfx[1] = SfxEntry {
            channel: ChannelType::Pulse,
            instrument: 1,
            table: 0,
            note: 40,
            priority: 2,
            duration: 2,
        };
        let mut engine = SoundEngine::new(48000, data);
        assert!(engine.play_song(0));
        engine.tick();
        assert_eq!(engine.channel(0).owner, ChannelOwner::Music);
        engine.trigger_sfx(1);
        assert_eq!(engine.channel(1).owner, ChannelOwner::Sfx);
        engine.tick();
        assert_eq!(engine.channel(1).owner, ChannelOwner::Sfx);
        engine.tick();
        assert_eq!(engine.channel(1).owner, ChannelOwner::Sfx);
        engine.tick();
        assert_eq!(engine.channel(1).owner, ChannelOwner::None);
    }

    #[test]
    fn render_produces_stereo_samples() {
        let mut engine = SoundEngine::new(48000, test_data());
        assert!(engine.play_song(0));
        let mut out = Vec::new();
        engine.advance(1.0 / 60.0, &mut out);
        assert_eq!(out.len(), 800 * 2);
        assert!(out.iter().any(|sample| *sample != 0.0));
    }

    #[test]
    fn wave_uses_wave_period() {
        let mut data = test_data();
        data.orders[0].patterns = [0, 0, 1, 0];
        data.patterns[2][1][0] = MusicCell {
            note: 34,
            instrument: 2,
            effect: 0,
            value: 0,
        };
        let mut engine = SoundEngine::new(48000, data);
        assert!(engine.play_song(0));
        engine.tick();
        assert_eq!(engine.channel(2).period, 1899);
    }

    #[test]
    fn sound_data_from_bytes_sizes() {
        let data = SoundData::default();
        assert_eq!(
            data.to_audio_bytes().len(),
            crate::config::AUDIO_REGION_SIZE
        );
        assert_eq!(data.to_song_bytes().len(), crate::config::SONG_REGION_SIZE);
        assert_eq!(AUDIO_SFX_SIZE, 6);
        assert_eq!(MUSIC_ROWS, 32);
        let _ = Order::default();
        let _ = note_parts(1);
    }
}
