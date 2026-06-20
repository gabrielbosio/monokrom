use std::collections::VecDeque;

use macroquad::prelude::*;

use crate::audio::{
    cell_detune, cell_pitch, cell_timbre, cell_volume, music_cell_at, music_pattern_end_flag,
    music_pattern_loop_end, music_pattern_loop_start, music_pattern_speed, pack_cell,
    set_music_cell, set_music_pattern_header, CH_NOISE, CH_PULSE1, CH_PULSE2, CH_WAVE, MAX_SPEED,
};
use crate::config::{
    COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE, MUSIC_CHANNELS, MUSIC_COUNT, MUSIC_END_LOOP,
    MUSIC_END_NEXT, MUSIC_END_STOP, MUSIC_PATTERN_SIZE, MUSIC_REGION_SIZE, MUSIC_ROWS,
    SCREEN_HEIGHT, TILE_WIDTH,
};
use crate::input::{
    is_modifier_pressed, is_shift_pressed, is_shortcut_modifier_pressed, should_key_fire,
};
use crate::render::DrawHelpers;

const UNDO_LIMIT: usize = 32;

const HEADER_Y: f32 = 0.0;
const CHANNEL_LABEL_Y: f32 = 6.0;
const GRID_Y: f32 = 12.0;
const ROW_HEIGHT: f32 = 6.0;
const VISIBLE_ROWS: usize = 20;
const INFO_Y: f32 = GRID_Y + ROW_HEIGHT * VISIBLE_ROWS as f32;
const HINT_Y: f32 = SCREEN_HEIGHT as f32 - 6.0;

const ROW_LABEL_X: f32 = 0.0;
const CHANNEL_X: [f32; MUSIC_CHANNELS] = [16.0, 52.0, 88.0, 124.0];
const CHANNEL_SLOT_WIDTH: f32 = 5.0 * TILE_WIDTH as f32;

const CH_NAMES: [&str; MUSIC_CHANNELS] = ["P1", "P2", "WV", "NS"];

const NOTE_NAMES: [(char, char); 12] = [
    ('C', '-'),
    ('C', '#'),
    ('D', '-'),
    ('D', '#'),
    ('E', '-'),
    ('F', '-'),
    ('F', '#'),
    ('G', '-'),
    ('G', '#'),
    ('A', '-'),
    ('A', '#'),
    ('B', '-'),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Pitch,
    Timbre,
    Volume,
    Detune,
}

impl Field {
    fn next(self) -> Self {
        match self {
            Self::Pitch => Self::Timbre,
            Self::Timbre => Self::Volume,
            Self::Volume => Self::Detune,
            Self::Detune => Self::Pitch,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::Pitch => Self::Detune,
            Self::Timbre => Self::Pitch,
            Self::Volume => Self::Timbre,
            Self::Detune => Self::Volume,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Pitch => "P",
            Self::Timbre => "T",
            Self::Volume => "V",
            Self::Detune => "D",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HeaderField {
    Pattern,
    Speed,
    End,
    LoopStart,
    LoopEnd,
}

impl HeaderField {
    fn next(self) -> Self {
        match self {
            Self::Pattern => Self::Speed,
            Self::Speed => Self::End,
            Self::End => Self::LoopStart,
            Self::LoopStart => Self::LoopEnd,
            Self::LoopEnd => Self::Pattern,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::Pattern => Self::LoopEnd,
            Self::Speed => Self::Pattern,
            Self::End => Self::Speed,
            Self::LoopStart => Self::End,
            Self::LoopEnd => Self::LoopStart,
        }
    }
}

pub enum MusicEditorAction {
    None,
    ExitToSfxEditor,
    ExitToTerminal,
    Save,
    Run,
    Audition(u8),
    Stop,
}

pub struct MusicEditorOutput {
    pub action: MusicEditorAction,
    pub modified_pattern: Option<u8>,
}

impl MusicEditorOutput {
    fn none() -> Self {
        Self {
            action: MusicEditorAction::None,
            modified_pattern: None,
        }
    }
    fn action(action: MusicEditorAction) -> Self {
        Self {
            action,
            modified_pattern: None,
        }
    }
    fn modified(idx: u8) -> Self {
        Self {
            action: MusicEditorAction::None,
            modified_pattern: Some(idx),
        }
    }
}

pub struct MusicEditor {
    pub data: [u8; MUSIC_REGION_SIZE],
    selected: u8,
    cursor_row: u8,
    cursor_channel: u8,
    cursor_field: Field,
    scroll_row: u8,
    header_focus: bool,
    header_field: HeaderField,
    undo: VecDeque<(u8, [u8; MUSIC_PATTERN_SIZE])>,
    redo: VecDeque<(u8, [u8; MUSIC_PATTERN_SIZE])>,
    cell_clipboard: Option<u16>,
    pattern_clipboard: Option<[u8; MUSIC_PATTERN_SIZE]>,
}

impl Default for MusicEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl MusicEditor {
    pub fn new() -> Self {
        Self {
            data: [0; MUSIC_REGION_SIZE],
            selected: 0,
            cursor_row: 0,
            cursor_channel: 0,
            cursor_field: Field::Pitch,
            scroll_row: 0,
            header_focus: false,
            header_field: HeaderField::Pattern,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            cell_clipboard: None,
            pattern_clipboard: None,
        }
    }

    pub fn update(&mut self) -> MusicEditorOutput {
        if is_key_pressed(KeyCode::Escape) {
            return MusicEditorOutput::action(if is_shift_pressed() {
                MusicEditorAction::ExitToSfxEditor
            } else {
                MusicEditorAction::ExitToTerminal
            });
        }

        let modifier = is_modifier_pressed();
        let shift = is_shift_pressed();

        if modifier && !shift && is_key_pressed(KeyCode::S) {
            return MusicEditorOutput::action(MusicEditorAction::Save);
        }
        if modifier && !shift && is_key_pressed(KeyCode::Z) {
            return self.do_undo();
        }
        if (modifier && is_key_pressed(KeyCode::Y))
            || (modifier && shift && is_key_pressed(KeyCode::Z))
        {
            return self.do_redo();
        }
        if modifier && shift && is_key_pressed(KeyCode::C) {
            self.pattern_clipboard = Some(self.get_pattern_bytes(self.selected));
            return MusicEditorOutput::none();
        }
        if modifier && shift && is_key_pressed(KeyCode::V) {
            if let Some(bytes) = self.pattern_clipboard {
                self.push_undo();
                self.set_pattern_bytes(self.selected, &bytes);
                return MusicEditorOutput::modified(self.selected);
            }
            return MusicEditorOutput::none();
        }
        if modifier && !shift && !self.header_focus && is_key_pressed(KeyCode::C) {
            self.cell_clipboard = Some(music_cell_at(
                &self.data,
                self.selected,
                self.cursor_row,
                self.cursor_channel,
            ));
            return MusicEditorOutput::none();
        }
        if modifier && !shift && !self.header_focus && is_key_pressed(KeyCode::V) {
            if let Some(c) = self.cell_clipboard {
                self.push_undo();
                set_music_cell(
                    &mut self.data,
                    self.selected,
                    self.cursor_row,
                    self.cursor_channel,
                    c,
                );
                return MusicEditorOutput::modified(self.selected);
            }
            return MusicEditorOutput::none();
        }
        if is_shortcut_modifier_pressed()
            && !shift
            && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter))
        {
            return MusicEditorOutput::action(MusicEditorAction::Run);
        }
        if modifier {
            return MusicEditorOutput::none();
        }

        if is_key_pressed(KeyCode::Tab) {
            self.header_focus = !self.header_focus;
            return MusicEditorOutput::none();
        }

        if is_key_pressed(KeyCode::Space) {
            return MusicEditorOutput::action(if shift {
                MusicEditorAction::Stop
            } else {
                MusicEditorAction::Audition(self.selected)
            });
        }

        if self.header_focus {
            return self.update_header(shift);
        }

        self.update_grid(shift)
    }

    fn update_header(&mut self, shift: bool) -> MusicEditorOutput {
        if should_key_fire(KeyCode::Left, false) {
            self.header_field = self.header_field.prev();
        }
        if should_key_fire(KeyCode::Right, false) {
            self.header_field = self.header_field.next();
        }
        let bump: i32 = if should_key_fire(KeyCode::Up, false) {
            if shift {
                10
            } else {
                1
            }
        } else if should_key_fire(KeyCode::Down, false) {
            if shift {
                -10
            } else {
                -1
            }
        } else {
            0
        };
        if bump != 0 {
            if matches!(self.header_field, HeaderField::Pattern) {
                self.selected = (self.selected as i32 + bump).rem_euclid(MUSIC_COUNT as i32) as u8;
                self.clamp_scroll();
                return MusicEditorOutput::none();
            }
            self.push_undo();
            self.bump_header(bump);
            return MusicEditorOutput::modified(self.selected);
        }
        MusicEditorOutput::none()
    }

    fn bump_header(&mut self, delta: i32) {
        let sp = music_pattern_speed(&self.data, self.selected);
        let end = music_pattern_end_flag(&self.data, self.selected);
        let ls = music_pattern_loop_start(&self.data, self.selected);
        let le = music_pattern_loop_end(&self.data, self.selected);
        let (mut sp, mut end, mut ls, mut le) = (sp, end, ls, le);
        match self.header_field {
            HeaderField::Pattern => {}
            HeaderField::Speed => {
                sp = (sp as i32 + delta).clamp(1, MAX_SPEED as i32) as u8;
            }
            HeaderField::End => {
                end = ((end as i32 + delta).rem_euclid(3)) as u8;
            }
            HeaderField::LoopStart => {
                ls = (ls as i32 + delta).clamp(0, MUSIC_ROWS as i32) as u8;
            }
            HeaderField::LoopEnd => {
                le = (le as i32 + delta).clamp(0, MUSIC_ROWS as i32) as u8;
            }
        }
        set_music_pattern_header(&mut self.data, self.selected, sp, end, ls, le);
    }

    fn update_grid(&mut self, shift: bool) -> MusicEditorOutput {
        let alt = is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt);

        if should_key_fire(KeyCode::Left, false) && !alt {
            if shift {
                self.cursor_channel =
                    (self.cursor_channel + MUSIC_CHANNELS as u8 - 1) % MUSIC_CHANNELS as u8;
            } else {
                self.cursor_field = self.cursor_field.prev();
                if matches!(self.cursor_field, Field::Detune) {
                    self.cursor_channel =
                        (self.cursor_channel + MUSIC_CHANNELS as u8 - 1) % MUSIC_CHANNELS as u8;
                }
            }
        }
        if should_key_fire(KeyCode::Right, false) && !alt {
            if shift {
                self.cursor_channel = (self.cursor_channel + 1) % MUSIC_CHANNELS as u8;
            } else {
                self.cursor_field = self.cursor_field.next();
                if matches!(self.cursor_field, Field::Pitch) {
                    self.cursor_channel = (self.cursor_channel + 1) % MUSIC_CHANNELS as u8;
                }
            }
        }

        if should_key_fire(KeyCode::Up, false) && !alt {
            let step = if shift { 8 } else { 1 };
            self.cursor_row = (self.cursor_row + MUSIC_ROWS as u8 - step) % MUSIC_ROWS as u8;
            self.ensure_cursor_visible();
        }
        if should_key_fire(KeyCode::Down, false) && !alt {
            let step = if shift { 8 } else { 1 };
            self.cursor_row = (self.cursor_row + step) % MUSIC_ROWS as u8;
            self.ensure_cursor_visible();
        }

        let bump: i32 = if should_key_fire(KeyCode::Up, false) && alt {
            if shift && matches!(self.cursor_field, Field::Pitch) {
                12
            } else {
                1
            }
        } else if should_key_fire(KeyCode::Down, false) && alt {
            if shift && matches!(self.cursor_field, Field::Pitch) {
                -12
            } else {
                -1
            }
        } else {
            0
        };

        if bump != 0 {
            self.push_undo();
            self.bump_field(bump);
            return MusicEditorOutput::modified(self.selected);
        }

        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            let cur = music_cell_at(
                &self.data,
                self.selected,
                self.cursor_row,
                self.cursor_channel,
            );
            if cur != 0 {
                self.push_undo();
                set_music_cell(
                    &mut self.data,
                    self.selected,
                    self.cursor_row,
                    self.cursor_channel,
                    0,
                );
                return MusicEditorOutput::modified(self.selected);
            }
            return MusicEditorOutput::none();
        }

        for (code, val) in [
            (KeyCode::Key0, 0),
            (KeyCode::Key1, 1),
            (KeyCode::Key2, 2),
            (KeyCode::Key3, 3),
            (KeyCode::Key4, 4),
            (KeyCode::Key5, 5),
            (KeyCode::Key6, 6),
            (KeyCode::Key7, 7),
        ] {
            if is_key_pressed(code) {
                self.push_undo();
                self.set_field(val);
                return MusicEditorOutput::modified(self.selected);
            }
        }

        MusicEditorOutput::none()
    }

    fn bump_field(&mut self, delta: i32) {
        let c = music_cell_at(
            &self.data,
            self.selected,
            self.cursor_row,
            self.cursor_channel,
        );
        let p = cell_pitch(c) as i32;
        let t = cell_timbre(c) as i32;
        let v = cell_volume(c) as i32;
        let d = cell_detune(c) as i32;
        let new = match self.cursor_field {
            Field::Pitch => pack_cell((p + delta).clamp(0, 63) as u8, t as u8, v as u8, d as u8),
            Field::Timbre => pack_cell(p as u8, (t + delta).rem_euclid(8) as u8, v as u8, d as u8),
            Field::Volume => pack_cell(p as u8, t as u8, (v + delta).rem_euclid(8) as u8, d as u8),
            Field::Detune => pack_cell(p as u8, t as u8, v as u8, (d + delta).rem_euclid(8) as u8),
        };
        set_music_cell(
            &mut self.data,
            self.selected,
            self.cursor_row,
            self.cursor_channel,
            new,
        );
    }

    fn set_field(&mut self, value: u8) {
        let c = music_cell_at(
            &self.data,
            self.selected,
            self.cursor_row,
            self.cursor_channel,
        );
        let p = cell_pitch(c);
        let t = cell_timbre(c);
        let v = cell_volume(c);
        let d = cell_detune(c);
        let new = match self.cursor_field {
            Field::Pitch => pack_cell(value & 0x3F, t, v, d),
            Field::Timbre => pack_cell(p, value & 0x07, v, d),
            Field::Volume => pack_cell(p, t, value & 0x07, d),
            Field::Detune => pack_cell(p, t, v, value & 0x07),
        };
        set_music_cell(
            &mut self.data,
            self.selected,
            self.cursor_row,
            self.cursor_channel,
            new,
        );
    }

    fn ensure_cursor_visible(&mut self) {
        let visible = VISIBLE_ROWS as u8;
        if self.cursor_row < self.scroll_row {
            self.scroll_row = self.cursor_row;
        } else if self.cursor_row >= self.scroll_row + visible {
            self.scroll_row = self.cursor_row + 1 - visible;
        }
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let max = (MUSIC_ROWS as u8).saturating_sub(VISIBLE_ROWS as u8);
        if self.scroll_row > max {
            self.scroll_row = max;
        }
    }

    fn get_pattern_bytes(&self, idx: u8) -> [u8; MUSIC_PATTERN_SIZE] {
        let base = idx as usize * MUSIC_PATTERN_SIZE;
        let mut buf = [0u8; MUSIC_PATTERN_SIZE];
        buf.copy_from_slice(&self.data[base..base + MUSIC_PATTERN_SIZE]);
        buf
    }

    fn set_pattern_bytes(&mut self, idx: u8, bytes: &[u8; MUSIC_PATTERN_SIZE]) {
        let base = idx as usize * MUSIC_PATTERN_SIZE;
        self.data[base..base + MUSIC_PATTERN_SIZE].copy_from_slice(bytes);
    }

    fn push_undo(&mut self) {
        let bytes = self.get_pattern_bytes(self.selected);
        self.undo.push_back((self.selected, bytes));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    fn do_undo(&mut self) -> MusicEditorOutput {
        if let Some((idx, bytes)) = self.undo.pop_back() {
            let current = self.get_pattern_bytes(idx);
            self.redo.push_back((idx, current));
            self.set_pattern_bytes(idx, &bytes);
            self.selected = idx;
            return MusicEditorOutput::modified(idx);
        }
        MusicEditorOutput::none()
    }

    fn do_redo(&mut self) -> MusicEditorOutput {
        if let Some((idx, bytes)) = self.redo.pop_back() {
            let current = self.get_pattern_bytes(idx);
            self.undo.push_back((idx, current));
            self.set_pattern_bytes(idx, &bytes);
            self.selected = idx;
            return MusicEditorOutput::modified(idx);
        }
        MusicEditorOutput::none()
    }

    pub fn append_data(&self, content: &mut String) {
        if self.data.iter().any(|&b| b != 0) {
            content.push_str("\n__mus__\n");
            for row in self.data.chunks(16) {
                let hex: String = row.iter().map(|b| format!("{b:02x}")).collect();
                content.push_str(&hex);
                content.push('\n');
            }
        }
    }

    pub fn draw(&self, helpers: &DrawHelpers) {
        self.draw_header(helpers);
        self.draw_grid(helpers);
        self.draw_cursor(helpers);
        self.draw_info(helpers);
        self.draw_hint(helpers);
    }

    fn draw_header(&self, helpers: &DrawHelpers) {
        let sp = music_pattern_speed(&self.data, self.selected);
        let end = music_pattern_end_flag(&self.data, self.selected);
        let ls = music_pattern_loop_start(&self.data, self.selected);
        let le = music_pattern_loop_end(&self.data, self.selected);
        let end_name = match end {
            MUSIC_END_STOP => "STP",
            MUSIC_END_LOOP => "LOP",
            MUSIC_END_NEXT => "NXT",
            _ => "???",
        };
        let lp = if end == MUSIC_END_LOOP
            && ls < MUSIC_ROWS as u8
            && le > ls
            && le <= MUSIC_ROWS as u8
        {
            format!("{ls:02}-{le:02}")
        } else {
            "--".to_string()
        };
        let label = format!(
            "MUS:{:02} SPD:{:03} END:{} L:{}",
            self.selected, sp, end_name, lp
        );
        for (i, c) in label.chars().enumerate() {
            helpers.draw_char(c, i as f32 * TILE_WIDTH as f32, HEADER_Y, COLOR_WHITE);
        }
        if self.header_focus {
            let loop_set = lp.contains('-');
            let (start, len) = match self.header_field {
                HeaderField::Pattern => (4, 2),
                HeaderField::Speed => (11, 3),
                HeaderField::End => (19, 3),
                HeaderField::LoopStart => (25, 2),
                HeaderField::LoopEnd => {
                    if loop_set {
                        (28, 2)
                    } else {
                        (25, 2)
                    }
                }
            };
            let x = start as f32 * TILE_WIDTH as f32;
            let w = len as f32 * TILE_WIDTH as f32;
            helpers.draw_rect(x, 5.0, w, 1.0, COLOR_WHITE);
        }
    }

    fn draw_grid(&self, helpers: &DrawHelpers) {
        for (ch_idx, name) in CH_NAMES.iter().enumerate() {
            let x = CHANNEL_X[ch_idx];
            for (i, c) in name.chars().enumerate() {
                helpers.draw_char(
                    c,
                    x + i as f32 * TILE_WIDTH as f32,
                    CHANNEL_LABEL_Y,
                    COLOR_DARK_GRAY,
                );
            }
        }

        for visible_row in 0..VISIBLE_ROWS {
            let row = self.scroll_row as usize + visible_row;
            if row >= MUSIC_ROWS {
                break;
            }
            let y = GRID_Y + visible_row as f32 * ROW_HEIGHT;
            let in_loop = self.row_in_loop(row as u8);
            let label_color = if row.is_multiple_of(4) {
                COLOR_WHITE
            } else {
                COLOR_DARK_GRAY
            };
            self.draw_row_label(helpers, row as u8, y, label_color);

            for (ch, &x) in CHANNEL_X.iter().enumerate() {
                let cell = music_cell_at(&self.data, self.selected, row as u8, ch as u8);
                let color = if in_loop {
                    COLOR_WHITE
                } else {
                    COLOR_LIGHT_GRAY
                };
                self.draw_cell(helpers, cell, x, y, color);
            }
        }
    }

    fn row_in_loop(&self, row: u8) -> bool {
        if music_pattern_end_flag(&self.data, self.selected) != MUSIC_END_LOOP {
            return false;
        }
        let ls = music_pattern_loop_start(&self.data, self.selected);
        let le = music_pattern_loop_end(&self.data, self.selected);
        if ls >= MUSIC_ROWS as u8 || le <= ls || le > MUSIC_ROWS as u8 {
            return false;
        }
        row >= ls && row < le
    }

    fn draw_row_label(&self, helpers: &DrawHelpers, row: u8, y: f32, color: Color) {
        let tens = char::from_digit((row / 10) as u32, 10).unwrap_or('?');
        let ones = char::from_digit((row % 10) as u32, 10).unwrap_or('?');
        helpers.draw_char(tens, ROW_LABEL_X, y, color);
        helpers.draw_char(ones, ROW_LABEL_X + TILE_WIDTH as f32, y, color);
    }

    fn draw_cell(&self, helpers: &DrawHelpers, cell: u16, x: f32, y: f32, color: Color) {
        let pitch = cell_pitch(cell);
        let volume = cell_volume(cell);
        if volume == 0 {
            for i in 0..3 {
                helpers.draw_char('-', x + i as f32 * TILE_WIDTH as f32, y, COLOR_DARK_GRAY);
            }
            helpers.draw_char('-', x + 4.0 * TILE_WIDTH as f32, y, COLOR_DARK_GRAY);
            return;
        }
        let (note, sharp, oct) = pitch_to_note_chars(pitch);
        helpers.draw_char(note, x, y, color);
        helpers.draw_char(sharp, x + TILE_WIDTH as f32, y, color);
        helpers.draw_char(oct, x + 2.0 * TILE_WIDTH as f32, y, color);
        let vc = char::from_digit(volume as u32, 10).unwrap_or('?');
        helpers.draw_char(vc, x + 4.0 * TILE_WIDTH as f32, y, color);
    }

    fn draw_cursor(&self, helpers: &DrawHelpers) {
        if self.header_focus {
            return;
        }
        if self.cursor_row < self.scroll_row
            || self.cursor_row >= self.scroll_row + VISIBLE_ROWS as u8
        {
            return;
        }
        let visible_row = (self.cursor_row - self.scroll_row) as f32;
        let y = GRID_Y + visible_row * ROW_HEIGHT;
        let x = CHANNEL_X[self.cursor_channel as usize];
        let top = y - 1.0;
        let bot = y + ROW_HEIGHT - 1.0;
        let w = CHANNEL_SLOT_WIDTH;
        helpers.draw_rect(x, top, 1.0, ROW_HEIGHT + 1.0, COLOR_LIGHT_GRAY);
        helpers.draw_rect(x + w - 1.0, top, 1.0, ROW_HEIGHT + 1.0, COLOR_LIGHT_GRAY);
        helpers.draw_rect(x, top, w, 1.0, COLOR_LIGHT_GRAY);
        helpers.draw_rect(x, bot, w, 1.0, COLOR_LIGHT_GRAY);
    }

    fn draw_info(&self, helpers: &DrawHelpers) {
        let cell = music_cell_at(
            &self.data,
            self.selected,
            self.cursor_row,
            self.cursor_channel,
        );
        let ch_name = match self.cursor_channel {
            CH_PULSE1 => "P1",
            CH_PULSE2 => "P2",
            CH_WAVE => "WV",
            CH_NOISE => "NS",
            _ => "??",
        };
        let label = format!(
            "R:{:02} {} F:{} P:{:02} T:{} V:{} D:{}",
            self.cursor_row,
            ch_name,
            self.cursor_field.label(),
            cell_pitch(cell),
            cell_timbre(cell),
            cell_volume(cell),
            cell_detune(cell),
        );
        for (i, ch) in label.chars().enumerate() {
            helpers.draw_char(ch, i as f32 * TILE_WIDTH as f32, INFO_Y, COLOR_WHITE);
        }
    }

    fn draw_hint(&self, helpers: &DrawHelpers) {
        let hint = if self.header_focus {
            "tab:grid shift+up/dn:+-10"
        } else {
            "play:sp tab:hdr alt+up/dn"
        };
        for (i, c) in hint.chars().enumerate() {
            helpers.draw_char(c, i as f32 * TILE_WIDTH as f32, HINT_Y, COLOR_DARK_GRAY);
        }
    }
}

fn pitch_to_note_chars(pitch: u8) -> (char, char, char) {
    let semi = (pitch % 12) as usize;
    let octave = 2 + pitch / 12;
    let (note, sharp) = NOTE_NAMES[semi];
    let oct_char = char::from_digit(octave as u32, 10).unwrap_or('?');
    (note, sharp, oct_char)
}
