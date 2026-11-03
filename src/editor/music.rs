use std::collections::VecDeque;

use macroquad::prelude::*;

use crate::audio::engine::{
    EFFECT_ARPEGGIO, EFFECT_NONE, EFFECT_NOTE_CUT, EFFECT_NOTE_DELAY, EFFECT_PORTA_DOWN,
    EFFECT_PORTA_UP, EFFECT_RETRIGGER, EFFECT_SET_MASTER_VOLUME, EFFECT_SET_PANNING,
    EFFECT_SET_SHAPE, EFFECT_SET_SPEED, EFFECT_SET_VOLUME, EFFECT_TABLE, EFFECT_TONE_PORTA,
    EFFECT_VIBRATO,
};
use crate::audio::format::{ChannelType, MusicCell, SoundData};
use crate::audio::notes::{note_parts, NOTE_MAX, NOTE_MIN, NOTE_OFF};
use crate::config::{
    COLOR_BLACK, COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE, MUSIC_CHANNELS, MUSIC_ROWS,
    SONG_COUNT, SONG_ORDER_COUNT, SONG_PATTERN_COUNT, TILE_WIDTH,
};
use crate::input::{
    is_arrow_modifier_pressed, is_shift_pressed, is_shortcut_modifier_pressed, should_key_fire,
};
use crate::render::DrawHelpers;

const UNDO_LIMIT: usize = 16;
const HEADER_Y: f32 = 0.0;
const HEADER2_Y: f32 = 6.0;
const GRID_Y: f32 = 18.0;
const ROW_HEIGHT: f32 = 6.0;
const VISIBLE_ROWS: usize = 19;
const INFO_Y: f32 = 132.0;
const ROW_LABEL_X: f32 = 0.0;
const CHANNEL_X: [f32; MUSIC_CHANNELS] = [2.0, 11.0, 20.0, 29.0];
const CH_NAMES: [&str; MUSIC_CHANNELS] = ["P1", "P2", "WV", "NS"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Note,
    Instrument,
    Effect,
    Value,
}

impl Field {
    fn next(self) -> Self {
        match self {
            Self::Note => Self::Instrument,
            Self::Instrument => Self::Effect,
            Self::Effect => Self::Value,
            Self::Value => Self::Note,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::Note => Self::Value,
            Self::Instrument => Self::Note,
            Self::Effect => Self::Instrument,
            Self::Value => Self::Effect,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Instrument => "Instrument",
            Self::Effect => "Effect",
            Self::Value => "Value",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeaderField {
    Song,
    Order,
    Pulse1,
    Pulse2,
    Wave,
    Noise,
    Ticks,
    Loop,
    LoopStart,
    LoopEnd,
    OrderCount,
}

impl HeaderField {
    fn next(self, loop_enabled: bool) -> Self {
        match self {
            Self::Song => Self::Order,
            Self::Order => Self::Pulse1,
            Self::Pulse1 => Self::Pulse2,
            Self::Pulse2 => Self::Wave,
            Self::Wave => Self::Noise,
            Self::Noise => Self::Ticks,
            Self::Ticks => Self::Loop,
            Self::Loop => {
                if loop_enabled {
                    Self::LoopStart
                } else {
                    Self::OrderCount
                }
            }
            Self::LoopStart => Self::LoopEnd,
            Self::LoopEnd => Self::OrderCount,
            Self::OrderCount => Self::Song,
        }
    }
    fn prev(self, loop_enabled: bool) -> Self {
        match self {
            Self::Song => Self::OrderCount,
            Self::Order => Self::Song,
            Self::Pulse1 => Self::Order,
            Self::Pulse2 => Self::Pulse1,
            Self::Wave => Self::Pulse2,
            Self::Noise => Self::Wave,
            Self::Ticks => Self::Noise,
            Self::Loop => Self::Ticks,
            Self::LoopStart => Self::Loop,
            Self::LoopEnd => Self::LoopStart,
            Self::OrderCount => {
                if loop_enabled {
                    Self::LoopEnd
                } else {
                    Self::Loop
                }
            }
        }
    }
}

pub enum MusicEditorAction {
    None,
    ExitToSoundEditor,
    ExitToTerminal,
    Save,
    Run,
    Preview(u8),
    Stop,
}

pub struct MusicEditorOutput {
    pub action: MusicEditorAction,
    pub modified: bool,
}

impl MusicEditorOutput {
    fn none() -> Self {
        Self {
            action: MusicEditorAction::None,
            modified: false,
        }
    }
    fn action(action: MusicEditorAction) -> Self {
        Self {
            action,
            modified: false,
        }
    }
    fn modified() -> Self {
        Self {
            action: MusicEditorAction::None,
            modified: true,
        }
    }
}

pub struct MusicEditor {
    song: u8,
    order: u8,
    row: u8,
    channel: u8,
    field: Field,
    scroll_row: u8,
    header_focus: bool,
    header_field: HeaderField,
    undo: VecDeque<SoundData>,
    redo: VecDeque<SoundData>,
    cell_clipboard: Option<MusicCell>,
    value_high: Option<u8>,
    note_octave: u8,
    note_octave_pending: bool,
}

impl Default for MusicEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl MusicEditor {
    pub fn new() -> Self {
        Self {
            song: 0,
            order: 0,
            row: 0,
            channel: 0,
            field: Field::Note,
            scroll_row: 0,
            header_focus: false,
            header_field: HeaderField::Song,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            cell_clipboard: None,
            value_high: None,
            note_octave: 4,
            note_octave_pending: false,
        }
    }

    pub fn current_order(&self) -> u8 {
        self.order
    }

    pub fn update(&mut self, data: &mut SoundData) -> MusicEditorOutput {
        if is_key_pressed(KeyCode::Escape) {
            return MusicEditorOutput::action(if is_shift_pressed() {
                MusicEditorAction::ExitToSoundEditor
            } else {
                MusicEditorAction::ExitToTerminal
            });
        }

        let modifier = is_shortcut_modifier_pressed();
        let shift = is_shift_pressed();

        if modifier && !shift && is_key_pressed(KeyCode::S) {
            return MusicEditorOutput::action(MusicEditorAction::Save);
        }
        if modifier
            && !shift
            && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter))
        {
            return MusicEditorOutput::action(MusicEditorAction::Run);
        }
        if modifier && !shift && is_key_pressed(KeyCode::Z) {
            return self.do_undo(data);
        }
        if (modifier && is_key_pressed(KeyCode::Y))
            || (modifier && shift && is_key_pressed(KeyCode::Z))
        {
            return self.do_redo(data);
        }
        if modifier && !shift && is_key_pressed(KeyCode::X) {
            let cell = self.current_cell(data);
            self.cell_clipboard = Some(cell);
            self.value_high = None;
            self.note_octave_pending = false;
            if cell != MusicCell::default() {
                self.push_undo(data);
                self.set_current_cell(data, MusicCell::default());
                return MusicEditorOutput::modified();
            }
            return MusicEditorOutput::none();
        }
        if modifier && !shift && is_key_pressed(KeyCode::C) {
            self.cell_clipboard = Some(self.current_cell(data));
            return MusicEditorOutput::none();
        }
        if modifier && !shift && is_key_pressed(KeyCode::V) {
            if let Some(cell) = self.cell_clipboard {
                self.value_high = None;
                self.note_octave_pending = false;
                self.push_undo(data);
                self.set_current_cell(data, cell);
                return MusicEditorOutput::modified();
            }
            return MusicEditorOutput::none();
        }

        if is_key_pressed(KeyCode::Tab) {
            self.value_high = None;
            self.note_octave_pending = false;
            self.header_focus = !self.header_focus;
            return MusicEditorOutput::none();
        }
        if is_key_pressed(KeyCode::Space) {
            return MusicEditorOutput::action(if shift {
                MusicEditorAction::Stop
            } else {
                MusicEditorAction::Preview(self.song)
            });
        }

        if self.header_focus {
            return self.update_header(data);
        }
        self.update_grid(data)
    }

    fn update_header(&mut self, data: &mut SoundData) -> MusicEditorOutput {
        self.value_high = None;
        self.note_octave_pending = false;
        let loop_enabled = data.songs[self.song as usize].loop_enabled;
        if !loop_enabled
            && matches!(
                self.header_field,
                HeaderField::LoopStart | HeaderField::LoopEnd
            )
        {
            self.header_field = HeaderField::Loop;
        }
        if should_key_fire(KeyCode::Left, false) {
            self.header_field = self.header_field.prev(loop_enabled);
        }
        if should_key_fire(KeyCode::Right, false) {
            self.header_field = self.header_field.next(loop_enabled);
        }
        let bump = if should_key_fire(KeyCode::Up, false) {
            if is_shift_pressed() {
                10
            } else {
                1
            }
        } else if should_key_fire(KeyCode::Down, false) {
            if is_shift_pressed() {
                -10
            } else {
                -1
            }
        } else {
            0
        };
        if bump == 0 {
            return MusicEditorOutput::none();
        }
        self.push_undo(data);
        match self.header_field {
            HeaderField::Song => {
                self.song = (self.song as i32 + bump).rem_euclid(SONG_COUNT as i32) as u8;
                self.order = data.songs[self.song as usize]
                    .order_start
                    .min(SONG_ORDER_COUNT as u8 - 1);
            }
            HeaderField::Order => {
                self.order = (self.order as i32 + bump).rem_euclid(SONG_ORDER_COUNT as i32) as u8;
            }
            HeaderField::Pulse1 => {
                let patterns = &mut data.orders[self.order as usize].patterns;
                patterns[0] =
                    (patterns[0] as i32 + bump).rem_euclid(SONG_PATTERN_COUNT as i32) as u8;
            }
            HeaderField::Pulse2 => {
                let patterns = &mut data.orders[self.order as usize].patterns;
                patterns[1] =
                    (patterns[1] as i32 + bump).rem_euclid(SONG_PATTERN_COUNT as i32) as u8;
            }
            HeaderField::Wave => {
                let patterns = &mut data.orders[self.order as usize].patterns;
                patterns[2] =
                    (patterns[2] as i32 + bump).rem_euclid(SONG_PATTERN_COUNT as i32) as u8;
            }
            HeaderField::Noise => {
                let patterns = &mut data.orders[self.order as usize].patterns;
                patterns[3] =
                    (patterns[3] as i32 + bump).rem_euclid(SONG_PATTERN_COUNT as i32) as u8;
            }
            HeaderField::Ticks => {
                data.header.ticks_per_row =
                    (data.header.ticks_per_row as i32 + bump).clamp(1, 255) as u8;
            }
            HeaderField::Loop => {
                let song = &mut data.songs[self.song as usize];
                song.loop_enabled = !song.loop_enabled;
                if song.loop_enabled {
                    if song.loop_end <= song.loop_start {
                        song.loop_end = (song.loop_start + 1).min(SONG_ORDER_COUNT as u8);
                    }
                } else {
                    self.header_field = HeaderField::Loop;
                }
            }
            HeaderField::LoopStart => {
                let song = &mut data.songs[self.song as usize];
                song.loop_start =
                    (song.loop_start as i32 + bump).clamp(0, SONG_ORDER_COUNT as i32 - 1) as u8;
                if song.loop_enabled && song.loop_end <= song.loop_start {
                    song.loop_end = (song.loop_start + 1).min(SONG_ORDER_COUNT as u8);
                }
            }
            HeaderField::LoopEnd => {
                let song = &mut data.songs[self.song as usize];
                song.loop_end =
                    (song.loop_end as i32 + bump).clamp(0, SONG_ORDER_COUNT as i32) as u8;
                if song.loop_end == 0 {
                    song.loop_enabled = false;
                    song.loop_start = 0;
                    self.header_field = HeaderField::Loop;
                } else if song.loop_enabled && song.loop_end <= song.loop_start {
                    song.loop_start = song.loop_end.saturating_sub(1);
                }
            }
            HeaderField::OrderCount => {
                data.header.order_count =
                    (data.header.order_count as i32 + bump).clamp(0, 64) as u8;
            }
        }
        MusicEditorOutput::modified()
    }

    fn update_grid(&mut self, data: &mut SoundData) -> MusicEditorOutput {
        let shift = is_shift_pressed();
        let arrow_mod = is_arrow_modifier_pressed();
        if should_key_fire(KeyCode::Left, false) && !arrow_mod {
            self.value_high = None;
            self.note_octave_pending = false;
            if shift {
                self.channel = (self.channel + MUSIC_CHANNELS as u8 - 1) % MUSIC_CHANNELS as u8;
            } else {
                self.field = self.field.prev();
                if self.field == Field::Value {
                    self.channel = (self.channel + MUSIC_CHANNELS as u8 - 1) % MUSIC_CHANNELS as u8;
                }
            }
            self.sync_note_octave(data);
        }
        if should_key_fire(KeyCode::Right, false) && !arrow_mod {
            self.value_high = None;
            self.note_octave_pending = false;
            if shift {
                self.channel = (self.channel + 1) % MUSIC_CHANNELS as u8;
            } else {
                self.field = self.field.next();
                if self.field == Field::Note {
                    self.channel = (self.channel + 1) % MUSIC_CHANNELS as u8;
                }
            }
            self.sync_note_octave(data);
        }
        let up = should_key_fire(KeyCode::Up, false);
        let down = should_key_fire(KeyCode::Down, false);
        if !arrow_mod {
            let step = if shift { 8 } else { 1 };
            if up {
                self.value_high = None;
                self.note_octave_pending = false;
                self.row = (self.row + MUSIC_ROWS as u8 - step) % MUSIC_ROWS as u8;
                self.ensure_cursor_visible();
                self.sync_note_octave(data);
            }
            if down {
                self.value_high = None;
                self.note_octave_pending = false;
                self.row = (self.row + step) % MUSIC_ROWS as u8;
                self.ensure_cursor_visible();
                self.sync_note_octave(data);
            }
        } else {
            let bump = if up {
                if shift {
                    match self.field {
                        Field::Note => 12,
                        Field::Value => 16,
                        _ => 1,
                    }
                } else {
                    1
                }
            } else if down {
                if shift {
                    match self.field {
                        Field::Note => -12,
                        Field::Value => -16,
                        _ => -1,
                    }
                } else {
                    -1
                }
            } else {
                0
            };
            if bump != 0 {
                self.value_high = None;
                self.note_octave_pending = false;
                if self.field == Field::Value && self.current_cell(data).effect == EFFECT_NONE {
                    return MusicEditorOutput::none();
                }
                self.push_undo(data);
                self.bump_field(data, bump);
                return MusicEditorOutput::modified();
            }
        }

        if self.field == Field::Note && !is_shortcut_modifier_pressed() && !arrow_mod {
            if let Some(semitone) = pressed_note_letter(shift) {
                let mut cell = self.current_cell(data);
                cell.note = note_number(self.note_octave, semitone);
                self.push_undo(data);
                self.set_current_cell(data, cell);
                self.note_octave_pending = true;
                return MusicEditorOutput::modified();
            }
            if let Some(octave) = pressed_octave_digit() {
                self.note_octave = octave;
                let mut cell = self.current_cell(data);
                if (NOTE_MIN..=NOTE_MAX).contains(&cell.note) {
                    let semitone = (cell.note - NOTE_MIN) % 12;
                    let note = note_number(octave, semitone);
                    if note != cell.note {
                        if !self.note_octave_pending {
                            self.push_undo(data);
                        }
                        cell.note = note;
                        self.set_current_cell(data, cell);
                        self.note_octave_pending = true;
                        return MusicEditorOutput::modified();
                    }
                    return MusicEditorOutput::none();
                }
                self.note_octave_pending = false;
                return MusicEditorOutput::none();
            }
        }

        if self.field == Field::Value && !is_shortcut_modifier_pressed() && !arrow_mod {
            if let Some(digit) = pressed_hex_digit() {
                let mut cell = self.current_cell(data);
                if cell.effect == EFFECT_NONE {
                    self.value_high = None;
                    return MusicEditorOutput::none();
                }
                if let Some(high) = self.value_high.take() {
                    cell.value = (high << 4) | digit;
                    self.set_current_cell(data, cell);
                } else {
                    self.push_undo(data);
                    cell.value = digit << 4;
                    self.set_current_cell(data, cell);
                    self.value_high = Some(digit);
                }
                return MusicEditorOutput::modified();
            }
        }

        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            self.value_high = None;
            self.note_octave_pending = false;
            self.push_undo(data);
            self.set_current_cell(data, MusicCell::default());
            return MusicEditorOutput::modified();
        }
        MusicEditorOutput::none()
    }

    fn bump_field(&mut self, data: &mut SoundData, delta: i32) {
        let mut cell = self.current_cell(data);
        match self.field {
            Field::Note => {
                if cell.note == 0 {
                    cell.note = if delta > 0 { NOTE_MIN } else { 0 };
                } else if cell.note == NOTE_OFF {
                    cell.note = 0;
                } else {
                    let next = cell.note as i32 + delta;
                    cell.note = if next < NOTE_MIN as i32 {
                        0
                    } else {
                        next.min(NOTE_MAX as i32) as u8
                    };
                }
            }
            Field::Instrument => {
                cell.instrument = self.next_instrument(data, cell.instrument, delta.signum());
            }
            Field::Effect => {
                cell.effect = (cell.effect as i32 + delta)
                    .rem_euclid(EFFECT_SET_MASTER_VOLUME as i32 + 1)
                    as u8;
            }
            Field::Value => {
                cell.value = (cell.value as i32 + delta).clamp(0, 255) as u8;
            }
        }
        self.set_current_cell(data, cell);
    }

    fn next_instrument(&self, data: &SoundData, current: u8, dir: i32) -> u8 {
        let wanted = match self.channel {
            0 | 1 => ChannelType::Pulse,
            2 => ChannelType::Wave,
            _ => ChannelType::Noise,
        };
        let candidates: Vec<u8> = (0..data.instruments.len() as u8)
            .filter(|&index| index == 0 || data.instruments[index as usize].channel == wanted)
            .collect();
        if candidates.is_empty() {
            return 0;
        }
        let position = candidates.iter().position(|&index| index == current);
        let next = match position {
            Some(position) => (position as i32 + dir).rem_euclid(candidates.len() as i32) as usize,
            None => {
                if dir >= 0 {
                    0
                } else {
                    candidates.len() - 1
                }
            }
        };
        candidates[next]
    }

    fn current_cell(&self, data: &SoundData) -> MusicCell {
        let pattern = data.orders[self.order as usize].patterns[self.channel as usize] as usize;
        data.patterns[self.channel as usize][pattern][self.row as usize]
    }

    fn set_current_cell(&mut self, data: &mut SoundData, cell: MusicCell) {
        let pattern = data.orders[self.order as usize].patterns[self.channel as usize] as usize;
        data.patterns[self.channel as usize][pattern][self.row as usize] = cell;
    }

    fn ensure_cursor_visible(&mut self) {
        if self.row < self.scroll_row {
            self.scroll_row = self.row;
        } else if self.row >= self.scroll_row + VISIBLE_ROWS as u8 {
            self.scroll_row = self.row + 1 - VISIBLE_ROWS as u8;
        }
    }

    fn sync_note_octave(&mut self, data: &SoundData) {
        if let Some((_, octave)) = note_parts(self.current_cell(data).note) {
            self.note_octave = octave;
        }
    }

    fn push_undo(&mut self, data: &SoundData) {
        self.undo.push_back(data.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    fn do_undo(&mut self, data: &mut SoundData) -> MusicEditorOutput {
        self.value_high = None;
        self.note_octave_pending = false;
        if let Some(snapshot) = self.undo.pop_back() {
            self.redo.push_back(data.clone());
            *data = snapshot;
            return MusicEditorOutput::modified();
        }
        MusicEditorOutput::none()
    }

    fn do_redo(&mut self, data: &mut SoundData) -> MusicEditorOutput {
        self.value_high = None;
        self.note_octave_pending = false;
        if let Some(snapshot) = self.redo.pop_back() {
            self.undo.push_back(data.clone());
            *data = snapshot;
            return MusicEditorOutput::modified();
        }
        MusicEditorOutput::none()
    }

    pub fn draw(&self, data: &SoundData, helpers: &DrawHelpers) {
        self.draw_header(data, helpers);
        self.draw_grid(data, helpers);
        self.draw_info(data, helpers);
    }

    fn draw_header(&self, data: &SoundData, helpers: &DrawHelpers) {
        let song = data.songs[self.song as usize];
        let patterns = data.orders[self.order as usize].patterns;
        let line1 = format!(
            "SONG:{:02} ORD:{:02} P1:{:02} P2:{:02} WV:{:02} NS:{:02}",
            self.song, self.order, patterns[0], patterns[1], patterns[2], patterns[3]
        );
        draw_text(helpers, 0.0, HEADER_Y, &line1, COLOR_WHITE);
        let line2 = if song.loop_enabled {
            format!(
                "SPD:{:03} L:ON {:02}-{:02} ORDS:{:02}",
                data.header.ticks_per_row, song.loop_start, song.loop_end, data.header.order_count
            )
        } else {
            format!(
                "SPD:{:03} L:OFF ORDS:{:02}",
                data.header.ticks_per_row, data.header.order_count
            )
        };
        draw_text(helpers, 0.0, HEADER2_Y, &line2, COLOR_WHITE);

        if self.header_focus {
            let (x, width, y) = match self.header_field {
                HeaderField::Song => (5, 2, HEADER_Y),
                HeaderField::Order => (12, 2, HEADER_Y),
                HeaderField::Pulse1 => (18, 2, HEADER_Y),
                HeaderField::Pulse2 => (24, 2, HEADER_Y),
                HeaderField::Wave => (30, 2, HEADER_Y),
                HeaderField::Noise => (36, 2, HEADER_Y),
                HeaderField::Ticks => (4, 3, HEADER2_Y),
                HeaderField::Loop => (10, if song.loop_enabled { 2 } else { 3 }, HEADER2_Y),
                HeaderField::LoopStart => (13, 2, HEADER2_Y),
                HeaderField::LoopEnd => (16, 2, HEADER2_Y),
                HeaderField::OrderCount => (if song.loop_enabled { 24 } else { 19 }, 2, HEADER2_Y),
            };
            helpers.draw_rect(
                x as f32 * TILE_WIDTH as f32,
                y + 5.0,
                width as f32 * TILE_WIDTH as f32,
                1.0,
                COLOR_WHITE,
            );
        }
    }

    fn draw_grid(&self, data: &SoundData, helpers: &DrawHelpers) {
        for (index, name) in CH_NAMES.iter().enumerate() {
            let color = if index as u8 == self.channel {
                COLOR_WHITE
            } else {
                COLOR_LIGHT_GRAY
            };
            draw_text(
                helpers,
                CHANNEL_X[index] * TILE_WIDTH as f32,
                GRID_Y - 6.0,
                name,
                color,
            );
        }
        let patterns = data.orders[self.order as usize].patterns;
        for visible in 0..VISIBLE_ROWS {
            let row = self.scroll_row as usize + visible;
            if row >= MUSIC_ROWS {
                break;
            }
            let y = GRID_Y + visible as f32 * ROW_HEIGHT;
            let label_color = if row == self.row as usize {
                COLOR_WHITE
            } else if row.is_multiple_of(4) {
                COLOR_LIGHT_GRAY
            } else {
                COLOR_DARK_GRAY
            };
            draw_text(helpers, ROW_LABEL_X, y, &format!("{:02}", row), label_color);
            for channel in 0..MUSIC_CHANNELS {
                let pattern = patterns[channel] as usize;
                let cell = data.patterns[channel][pattern][row];
                let empty = cell.note == 0 && cell.instrument == 0 && cell.effect == 0;
                let column_selected = channel as u8 == self.channel;
                let cell_selected = column_selected && row == self.row as usize;
                let color = if column_selected {
                    COLOR_WHITE
                } else if empty {
                    COLOR_DARK_GRAY
                } else {
                    COLOR_LIGHT_GRAY
                };
                let text = cell_text(cell);
                let x = CHANNEL_X[channel] * TILE_WIDTH as f32;
                draw_text(helpers, x, y, &text, color);
                if cell_selected {
                    let (offset, width) = match self.field {
                        Field::Note => (0, 3),
                        Field::Instrument => (3, 2),
                        Field::Effect => (5, 2),
                        Field::Value => (7, 2),
                    };
                    let sub_x = x + offset as f32 * TILE_WIDTH as f32;
                    helpers.draw_rect(
                        sub_x,
                        y,
                        width as f32 * TILE_WIDTH as f32,
                        ROW_HEIGHT,
                        COLOR_WHITE,
                    );
                    draw_text(
                        helpers,
                        sub_x,
                        y,
                        &text[offset..offset + width],
                        COLOR_BLACK,
                    );
                }
            }
        }
    }

    fn draw_info(&self, data: &SoundData, helpers: &DrawHelpers) {
        let cell = self.current_cell(data);
        let label = match self.field {
            Field::Effect | Field::Value => "Effect",
            _ => self.field.label(),
        };
        let value = match self.field {
            Field::Note => note_text(cell.note),
            Field::Instrument => instrument_text(cell.instrument),
            Field::Effect | Field::Value => {
                let value = if cell.effect == EFFECT_NONE {
                    "..".to_string()
                } else {
                    format!("{:02X}", cell.value)
                };
                format!("{:<14}{value:>2}", effect_label(cell.effect))
            }
        };
        draw_text(
            helpers,
            0.0,
            INFO_Y,
            &format!("{label:<12}{value}"),
            COLOR_WHITE,
        );
    }
}

fn cell_text(cell: MusicCell) -> String {
    let note = note_text(cell.note);
    let ins = instrument_text(cell.instrument);
    let effect = effect_name(cell.effect);
    let value = if cell.effect == EFFECT_NONE {
        "..".to_string()
    } else {
        format!("{:02X}", cell.value)
    };
    format!("{note}{ins}{effect}{value}")
}

fn note_text(note: u8) -> String {
    if note == NOTE_OFF {
        "OFF".to_string()
    } else {
        note_parts(note)
            .map(|(name, octave)| format!("{name}{octave}"))
            .unwrap_or_else(|| "---".to_string())
    }
}

fn instrument_text(instrument: u8) -> String {
    if instrument == 0 {
        "..".to_string()
    } else {
        format!("{instrument:02}")
    }
}

fn pressed_note_letter(sharp: bool) -> Option<u8> {
    const KEYS: [(KeyCode, u8); 7] = [
        (KeyCode::C, 0),
        (KeyCode::D, 2),
        (KeyCode::E, 4),
        (KeyCode::F, 5),
        (KeyCode::G, 7),
        (KeyCode::A, 9),
        (KeyCode::B, 11),
    ];
    for (key, semitone) in KEYS {
        if is_key_pressed(key) {
            return Some(semitone + sharp as u8);
        }
    }
    None
}

fn pressed_octave_digit() -> Option<u8> {
    const KEYS: [(KeyCode, u8); 12] = [
        (KeyCode::Key2, 2),
        (KeyCode::Key3, 3),
        (KeyCode::Key4, 4),
        (KeyCode::Key5, 5),
        (KeyCode::Key6, 6),
        (KeyCode::Key7, 7),
        (KeyCode::Kp2, 2),
        (KeyCode::Kp3, 3),
        (KeyCode::Kp4, 4),
        (KeyCode::Kp5, 5),
        (KeyCode::Kp6, 6),
        (KeyCode::Kp7, 7),
    ];
    for (key, octave) in KEYS {
        if is_key_pressed(key) {
            return Some(octave);
        }
    }
    None
}

fn note_number(octave: u8, semitone: u8) -> u8 {
    let note = (octave as i32 - 2) * 12 + semitone as i32 + NOTE_MIN as i32;
    note.clamp(NOTE_MIN as i32, NOTE_MAX as i32) as u8
}

fn pressed_hex_digit() -> Option<u8> {
    const KEYS: [(KeyCode, u8); 26] = [
        (KeyCode::Key0, 0),
        (KeyCode::Key1, 1),
        (KeyCode::Key2, 2),
        (KeyCode::Key3, 3),
        (KeyCode::Key4, 4),
        (KeyCode::Key5, 5),
        (KeyCode::Key6, 6),
        (KeyCode::Key7, 7),
        (KeyCode::Key8, 8),
        (KeyCode::Key9, 9),
        (KeyCode::A, 10),
        (KeyCode::B, 11),
        (KeyCode::C, 12),
        (KeyCode::D, 13),
        (KeyCode::E, 14),
        (KeyCode::F, 15),
        (KeyCode::Kp0, 0),
        (KeyCode::Kp1, 1),
        (KeyCode::Kp2, 2),
        (KeyCode::Kp3, 3),
        (KeyCode::Kp4, 4),
        (KeyCode::Kp5, 5),
        (KeyCode::Kp6, 6),
        (KeyCode::Kp7, 7),
        (KeyCode::Kp8, 8),
        (KeyCode::Kp9, 9),
    ];
    for (key, digit) in KEYS {
        if is_key_pressed(key) {
            return Some(digit);
        }
    }
    None
}

fn effect_name(effect: u8) -> &'static str {
    match effect {
        EFFECT_NONE => "..",
        EFFECT_ARPEGGIO => "AR",
        EFFECT_PORTA_UP => "P+",
        EFFECT_PORTA_DOWN => "P-",
        EFFECT_TONE_PORTA => "TP",
        EFFECT_VIBRATO => "VB",
        EFFECT_NOTE_DELAY => "DL",
        EFFECT_NOTE_CUT => "CT",
        EFFECT_RETRIGGER => "RT",
        EFFECT_SET_VOLUME => "VL",
        EFFECT_SET_SHAPE => "SH",
        EFFECT_TABLE => "TB",
        EFFECT_SET_SPEED => "SP",
        EFFECT_SET_PANNING => "PN",
        EFFECT_SET_MASTER_VOLUME => "MS",
        _ => "??",
    }
}

fn effect_label(effect: u8) -> &'static str {
    match effect {
        EFFECT_NONE => "None",
        EFFECT_ARPEGGIO => "Arpeggio",
        EFFECT_PORTA_UP => "Porta up",
        EFFECT_PORTA_DOWN => "Porta down",
        EFFECT_TONE_PORTA => "Tone porta",
        EFFECT_VIBRATO => "Vibrato",
        EFFECT_NOTE_DELAY => "Note delay",
        EFFECT_NOTE_CUT => "Note cut",
        EFFECT_RETRIGGER => "Retrigger",
        EFFECT_SET_VOLUME => "Volume",
        EFFECT_SET_SHAPE => "Shape",
        EFFECT_TABLE => "Table",
        EFFECT_SET_SPEED => "Speed",
        EFFECT_SET_PANNING => "Panning",
        EFFECT_SET_MASTER_VOLUME => "Master volume",
        _ => "Unknown",
    }
}

fn draw_text(helpers: &DrawHelpers, x: f32, y: f32, text: &str, color: Color) {
    for (index, c) in text.chars().enumerate() {
        helpers.draw_char(c, x + index as f32 * TILE_WIDTH as f32, y, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANNEL_WIDTH: f32 = 9.0;

    #[test]
    fn cell_text_fits_nine_chars() {
        let cell = MusicCell {
            note: 34,
            instrument: 3,
            effect: EFFECT_ARPEGGIO,
            value: 0x37,
        };
        assert_eq!(cell_text(cell).chars().count(), 9);
        assert_eq!(cell_text(MusicCell::default()), "---......");
    }

    #[test]
    fn loop_fields_skip_when_loop_is_off() {
        assert_eq!(HeaderField::Loop.next(false), HeaderField::OrderCount);
        assert_eq!(HeaderField::OrderCount.prev(false), HeaderField::Loop);
        assert_eq!(HeaderField::Loop.next(true), HeaderField::LoopStart);
        assert_eq!(HeaderField::OrderCount.prev(true), HeaderField::LoopEnd);
    }

    #[test]
    fn note_numbers_match_octaves() {
        assert_eq!(note_number(2, 0), 1);
        assert_eq!(note_number(4, 5), 30);
        assert_eq!(note_number(7, 11), 72);
        assert_eq!(note_number(7, 12), 72);
    }

    #[test]
    fn effect_names_are_two_chars() {
        for effect in 0..15 {
            assert_eq!(effect_name(effect).chars().count(), 2);
        }
    }

    #[test]
    fn value_requires_effect() {
        let without_effect = MusicCell {
            note: 34,
            instrument: 3,
            effect: EFFECT_NONE,
            value: 0x37,
        };
        assert_eq!(cell_text(without_effect), "A-403....");

        let with_effect = MusicCell {
            note: 34,
            instrument: 3,
            effect: EFFECT_ARPEGGIO,
            value: 0,
        };
        assert_eq!(cell_text(with_effect), "A-403AR00");
    }

    #[test]
    fn channels_fit_screen() {
        assert_eq!(CHANNEL_X[3] + CHANNEL_WIDTH, 38.0);
    }

    #[test]
    fn channel_pixels_do_not_overlap() {
        for index in 1..MUSIC_CHANNELS {
            let previous_end = (CHANNEL_X[index - 1] + CHANNEL_WIDTH) * TILE_WIDTH as f32;
            let start = CHANNEL_X[index] * TILE_WIDTH as f32;
            assert!(start >= previous_end, "channel {index} overlaps");
        }
    }
}
