use std::collections::VecDeque;

use macroquad::prelude::*;

use crate::audio::{
    cell_at, cell_effect, cell_pitch, cell_timbre, cell_volume, pack_cell, set_cell,
    set_sfx_header, sfx_channel, sfx_loop_end, sfx_loop_start, sfx_speed, CH_NOISE, CH_PULSE1,
    CH_PULSE2, CH_WAVE, MAX_SPEED,
};
use crate::config::{
    COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE, SCREEN_HEIGHT, SCREEN_WIDTH, SFX_CELLS,
    SFX_COUNT, SFX_REGION_SIZE, SFX_SIZE, TILE_WIDTH,
};
use crate::input::{
    is_modifier_pressed, is_shift_pressed, is_shortcut_modifier_pressed, should_key_fire,
};
use crate::render::DrawHelpers;

const UNDO_LIMIT: usize = 32;
const CELL_PX: f32 = 5.0;
const BARS_TOP: f32 = 8.0;
const BARS_BOTTOM: f32 = 79.0;
const ROW_T_Y: f32 = 82.0;
const ROW_V_Y: f32 = 89.0;
const ROW_F_Y: f32 = 96.0;
const INFO_Y: f32 = 110.0;
const HINT_Y: f32 = SCREEN_HEIGHT as f32 - 7.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Pitch,
    Timbre,
    Volume,
    Effect,
}

impl Row {
    fn next(self) -> Self {
        match self {
            Self::Pitch => Self::Timbre,
            Self::Timbre => Self::Volume,
            Self::Volume => Self::Effect,
            Self::Effect => Self::Pitch,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::Pitch => Self::Effect,
            Self::Timbre => Self::Pitch,
            Self::Volume => Self::Timbre,
            Self::Effect => Self::Volume,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Pitch => "PIT",
            Self::Timbre => "TIM",
            Self::Volume => "VOL",
            Self::Effect => "FX",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HeaderField {
    Sfx,
    Channel,
    Speed,
    LoopStart,
    LoopEnd,
}

impl HeaderField {
    fn next(self) -> Self {
        match self {
            Self::Sfx => Self::Channel,
            Self::Channel => Self::Speed,
            Self::Speed => Self::LoopStart,
            Self::LoopStart => Self::LoopEnd,
            Self::LoopEnd => Self::Sfx,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::Sfx => Self::LoopEnd,
            Self::Channel => Self::Sfx,
            Self::Speed => Self::Channel,
            Self::LoopStart => Self::Speed,
            Self::LoopEnd => Self::LoopStart,
        }
    }
}

pub enum SfxEditorAction {
    None,
    ExitToMapEditor,
    ExitToTerminal,
    Save,
    Run,
    Audition(u8),
    Stop,
}

pub struct SfxEditorOutput {
    pub action: SfxEditorAction,
    pub modified_sfx: Option<u8>,
}

impl SfxEditorOutput {
    fn none() -> Self {
        Self {
            action: SfxEditorAction::None,
            modified_sfx: None,
        }
    }
    fn action(action: SfxEditorAction) -> Self {
        Self {
            action,
            modified_sfx: None,
        }
    }
    fn modified(idx: u8) -> Self {
        Self {
            action: SfxEditorAction::None,
            modified_sfx: Some(idx),
        }
    }
}

pub struct SfxEditor {
    pub data: [u8; SFX_REGION_SIZE],
    selected: u8,
    cursor_cell: u8,
    cursor_row: Row,
    header_focus: bool,
    header_field: HeaderField,
    undo: VecDeque<(u8, [u8; SFX_SIZE])>,
    redo: VecDeque<(u8, [u8; SFX_SIZE])>,
    cell_clipboard: Option<u16>,
    sfx_clipboard: Option<[u8; SFX_SIZE]>,
}

impl Default for SfxEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl SfxEditor {
    pub fn new() -> Self {
        Self {
            data: [0; SFX_REGION_SIZE],
            selected: 0,
            cursor_cell: 0,
            cursor_row: Row::Pitch,
            header_focus: false,
            header_field: HeaderField::Sfx,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            cell_clipboard: None,
            sfx_clipboard: None,
        }
    }

    pub fn update(&mut self) -> SfxEditorOutput {
        if is_key_pressed(KeyCode::Escape) {
            return SfxEditorOutput::action(if is_shift_pressed() {
                SfxEditorAction::ExitToMapEditor
            } else {
                SfxEditorAction::ExitToTerminal
            });
        }

        let modifier = is_modifier_pressed();
        let shift = is_shift_pressed();

        if modifier && !shift && is_key_pressed(KeyCode::S) {
            return SfxEditorOutput::action(SfxEditorAction::Save);
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
            self.sfx_clipboard = Some(self.get_sfx_bytes(self.selected));
            return SfxEditorOutput::none();
        }
        if modifier && shift && is_key_pressed(KeyCode::V) {
            if let Some(bytes) = self.sfx_clipboard {
                self.push_undo();
                self.set_sfx_bytes(self.selected, &bytes);
                return SfxEditorOutput::modified(self.selected);
            }
            return SfxEditorOutput::none();
        }
        if modifier && !shift && is_key_pressed(KeyCode::C) {
            self.cell_clipboard = Some(cell_at(&self.data, self.selected, self.cursor_cell));
            return SfxEditorOutput::none();
        }
        if modifier && !shift && is_key_pressed(KeyCode::V) {
            if let Some(c) = self.cell_clipboard {
                self.push_undo();
                set_cell(&mut self.data, self.selected, self.cursor_cell, c);
                return SfxEditorOutput::modified(self.selected);
            }
            return SfxEditorOutput::none();
        }
        if is_shortcut_modifier_pressed()
            && !shift
            && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter))
        {
            return SfxEditorOutput::action(SfxEditorAction::Run);
        }
        if modifier {
            return SfxEditorOutput::none();
        }

        if is_key_pressed(KeyCode::Tab) {
            self.header_focus = !self.header_focus;
            return SfxEditorOutput::none();
        }

        if is_key_pressed(KeyCode::Space) {
            return SfxEditorOutput::action(if shift {
                SfxEditorAction::Stop
            } else {
                SfxEditorAction::Audition(self.selected)
            });
        }

        if self.header_focus {
            return self.update_header(shift);
        }

        self.update_grid(shift)
    }

    fn update_header(&mut self, shift: bool) -> SfxEditorOutput {
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
            if matches!(self.header_field, HeaderField::Sfx) {
                self.selected = (self.selected as i32 + bump).rem_euclid(SFX_COUNT as i32) as u8;
                return SfxEditorOutput::none();
            }
            self.push_undo();
            self.bump_header(bump);
            return SfxEditorOutput::modified(self.selected);
        }
        SfxEditorOutput::none()
    }

    fn bump_header(&mut self, delta: i32) {
        let ch = sfx_channel(&self.data, self.selected);
        let sp = sfx_speed(&self.data, self.selected);
        let ls = sfx_loop_start(&self.data, self.selected);
        let le = sfx_loop_end(&self.data, self.selected);
        let (mut ch, mut sp, mut ls, mut le) = (ch, sp, ls, le);
        match self.header_field {
            HeaderField::Sfx => {}
            HeaderField::Channel => {
                ch = ((ch as i32 + delta).rem_euclid(4)) as u8;
            }
            HeaderField::Speed => {
                sp = (sp as i32 + delta).clamp(1, MAX_SPEED as i32) as u8;
            }
            HeaderField::LoopStart => {
                ls = (ls as i32 + delta).clamp(0, SFX_CELLS as i32) as u8;
            }
            HeaderField::LoopEnd => {
                le = (le as i32 + delta).clamp(0, SFX_CELLS as i32) as u8;
            }
        }
        set_sfx_header(&mut self.data, self.selected, ch, sp, ls, le);
    }

    fn update_grid(&mut self, shift: bool) -> SfxEditorOutput {
        let alt = is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt);
        if should_key_fire(KeyCode::Left, false) && !alt {
            self.cursor_cell = (self.cursor_cell + SFX_CELLS as u8 - 1) % SFX_CELLS as u8;
        }
        if should_key_fire(KeyCode::Right, false) && !alt {
            self.cursor_cell = (self.cursor_cell + 1) % SFX_CELLS as u8;
        }
        if is_key_pressed(KeyCode::Up) && !shift && !alt {
            self.cursor_row = self.cursor_row.prev();
        }
        if is_key_pressed(KeyCode::Down) && !shift && !alt {
            self.cursor_row = self.cursor_row.next();
        }

        let bump: i32 = if should_key_fire(KeyCode::Up, false) && (shift || alt) {
            if shift && matches!(self.cursor_row, Row::Pitch) {
                12
            } else {
                1
            }
        } else if should_key_fire(KeyCode::Down, false) && (shift || alt) {
            if shift && matches!(self.cursor_row, Row::Pitch) {
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
            return SfxEditorOutput::modified(self.selected);
        }

        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            let cur = cell_at(&self.data, self.selected, self.cursor_cell);
            if cur != 0 {
                self.push_undo();
                set_cell(&mut self.data, self.selected, self.cursor_cell, 0);
                return SfxEditorOutput::modified(self.selected);
            }
            return SfxEditorOutput::none();
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
                return SfxEditorOutput::modified(self.selected);
            }
        }

        SfxEditorOutput::none()
    }

    fn bump_field(&mut self, delta: i32) {
        let c = cell_at(&self.data, self.selected, self.cursor_cell);
        let p = cell_pitch(c) as i32;
        let t = cell_timbre(c) as i32;
        let v = cell_volume(c) as i32;
        let f = cell_effect(c) as i32;
        let new = match self.cursor_row {
            Row::Pitch => pack_cell((p + delta).clamp(0, 63) as u8, t as u8, v as u8, f as u8),
            Row::Timbre => pack_cell(p as u8, (t + delta).rem_euclid(8) as u8, v as u8, f as u8),
            Row::Volume => pack_cell(p as u8, t as u8, (v + delta).rem_euclid(8) as u8, f as u8),
            Row::Effect => pack_cell(p as u8, t as u8, v as u8, (f + delta).rem_euclid(8) as u8),
        };
        set_cell(&mut self.data, self.selected, self.cursor_cell, new);
    }

    fn set_field(&mut self, value: u8) {
        let c = cell_at(&self.data, self.selected, self.cursor_cell);
        let p = cell_pitch(c);
        let t = cell_timbre(c);
        let v = cell_volume(c);
        let f = cell_effect(c);
        let new = match self.cursor_row {
            Row::Pitch => pack_cell(value & 0x3F, t, v, f),
            Row::Timbre => pack_cell(p, value & 0x07, v, f),
            Row::Volume => pack_cell(p, t, value & 0x07, f),
            Row::Effect => pack_cell(p, t, v, value & 0x07),
        };
        set_cell(&mut self.data, self.selected, self.cursor_cell, new);
    }

    fn get_sfx_bytes(&self, idx: u8) -> [u8; SFX_SIZE] {
        let base = idx as usize * SFX_SIZE;
        let mut buf = [0u8; SFX_SIZE];
        buf.copy_from_slice(&self.data[base..base + SFX_SIZE]);
        buf
    }

    fn set_sfx_bytes(&mut self, idx: u8, bytes: &[u8; SFX_SIZE]) {
        let base = idx as usize * SFX_SIZE;
        self.data[base..base + SFX_SIZE].copy_from_slice(bytes);
    }

    fn push_undo(&mut self) {
        let bytes = self.get_sfx_bytes(self.selected);
        self.undo.push_back((self.selected, bytes));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    fn do_undo(&mut self) -> SfxEditorOutput {
        if let Some((idx, bytes)) = self.undo.pop_back() {
            let current = self.get_sfx_bytes(idx);
            self.redo.push_back((idx, current));
            self.set_sfx_bytes(idx, &bytes);
            self.selected = idx;
            return SfxEditorOutput::modified(idx);
        }
        SfxEditorOutput::none()
    }

    fn do_redo(&mut self) -> SfxEditorOutput {
        if let Some((idx, bytes)) = self.redo.pop_back() {
            let current = self.get_sfx_bytes(idx);
            self.undo.push_back((idx, current));
            self.set_sfx_bytes(idx, &bytes);
            self.selected = idx;
            return SfxEditorOutput::modified(idx);
        }
        SfxEditorOutput::none()
    }

    pub fn append_data(&self, content: &mut String) {
        if self.data.iter().any(|&b| b != 0) {
            content.push_str("\n__sfx__\n");
            for row in self.data.chunks(16) {
                let hex: String = row.iter().map(|b| format!("{b:02x}")).collect();
                content.push_str(&hex);
                content.push('\n');
            }
        }
    }

    pub fn draw(&self, helpers: &DrawHelpers) {
        self.draw_header(helpers);
        self.draw_bars(helpers);
        self.draw_field_rows(helpers);
        self.draw_cursor(helpers);
        self.draw_info(helpers);
        self.draw_hint(helpers);
    }

    fn draw_header(&self, helpers: &DrawHelpers) {
        let ch = sfx_channel(&self.data, self.selected);
        let sp = sfx_speed(&self.data, self.selected);
        let ls = sfx_loop_start(&self.data, self.selected);
        let le = sfx_loop_end(&self.data, self.selected);
        let ch_name = match ch {
            CH_PULSE1 => "P1",
            CH_PULSE2 => "P2",
            CH_WAVE => "WV",
            CH_NOISE => "NS",
            _ => "??",
        };
        let lp = if ls < SFX_CELLS as u8 && le > ls && le <= SFX_CELLS as u8 {
            format!("{ls:02}-{le:02}")
        } else {
            "--".to_string()
        };
        let label = format!(
            "SFX:{:02} CH:{} SPD:{:03} L:{}",
            self.selected, ch_name, sp, lp
        );
        for (i, c) in label.chars().enumerate() {
            helpers.draw_char(c, i as f32 * TILE_WIDTH as f32, 0.0, COLOR_WHITE);
        }
        if self.header_focus {
            let loop_set = lp.contains('-');
            let (start, len) = match self.header_field {
                HeaderField::Sfx => (4, 2),
                HeaderField::Channel => (10, 2),
                HeaderField::Speed => (17, 3),
                HeaderField::LoopStart => (23, 2),
                HeaderField::LoopEnd => {
                    if loop_set {
                        (26, 2)
                    } else {
                        (23, 2)
                    }
                }
            };
            let x = start as f32 * TILE_WIDTH as f32;
            let w = len as f32 * TILE_WIDTH as f32;
            helpers.draw_rect(x, 6.0, w, 1.0, COLOR_WHITE);
        }
    }

    fn draw_bars(&self, helpers: &DrawHelpers) {
        let height = BARS_BOTTOM - BARS_TOP;
        helpers.draw_rect(0.0, BARS_TOP, SCREEN_WIDTH as f32, 1.0, COLOR_DARK_GRAY);
        helpers.draw_rect(0.0, BARS_BOTTOM, SCREEN_WIDTH as f32, 1.0, COLOR_DARK_GRAY);
        for ci in 0..SFX_CELLS {
            let c = cell_at(&self.data, self.selected, ci as u8);
            let vol = cell_volume(c);
            if vol == 0 {
                continue;
            }
            let pitch = cell_pitch(c);
            let bar_h = (pitch as f32 / 63.0 * height).max(1.0);
            let x = ci as f32 * CELL_PX + 1.0;
            let y = BARS_BOTTOM - bar_h;
            helpers.draw_rect(x, y, CELL_PX - 2.0, bar_h, COLOR_WHITE);
        }
    }

    fn draw_field_rows(&self, helpers: &DrawHelpers) {
        for ci in 0..SFX_CELLS {
            let c = cell_at(&self.data, self.selected, ci as u8);
            let vol = cell_volume(c);
            if vol == 0 {
                continue;
            }
            let t = cell_timbre(c);
            let f = cell_effect(c);
            let x = ci as f32 * CELL_PX + 1.0;
            let tc = char::from_digit(t as u32, 10).unwrap_or('?');
            let vc = char::from_digit(vol as u32, 10).unwrap_or('?');
            let fc = char::from_digit(f as u32, 10).unwrap_or('?');
            helpers.draw_char(tc, x, ROW_T_Y, COLOR_WHITE);
            helpers.draw_char(vc, x, ROW_V_Y, COLOR_WHITE);
            helpers.draw_char(fc, x, ROW_F_Y, COLOR_WHITE);
        }
    }

    fn draw_cursor(&self, helpers: &DrawHelpers) {
        if self.header_focus {
            return;
        }
        let x = self.cursor_cell as f32 * CELL_PX;
        let (top, bot) = match self.cursor_row {
            Row::Pitch => (BARS_TOP - 1.0, BARS_BOTTOM + 1.0),
            Row::Timbre => (ROW_T_Y - 1.0, ROW_T_Y + 6.0),
            Row::Volume => (ROW_V_Y - 1.0, ROW_V_Y + 6.0),
            Row::Effect => (ROW_F_Y - 1.0, ROW_F_Y + 6.0),
        };
        let h = bot - top;
        helpers.draw_rect(x, top, 1.0, h, COLOR_LIGHT_GRAY);
        helpers.draw_rect(x + CELL_PX - 1.0, top, 1.0, h, COLOR_LIGHT_GRAY);
        helpers.draw_rect(x, top, CELL_PX, 1.0, COLOR_LIGHT_GRAY);
        helpers.draw_rect(x, bot, CELL_PX, 1.0, COLOR_LIGHT_GRAY);
    }

    fn draw_info(&self, helpers: &DrawHelpers) {
        let c = cell_at(&self.data, self.selected, self.cursor_cell);
        let label = format!(
            "C:{:02} {} P:{:02} T:{} V:{} F:{}",
            self.cursor_cell,
            self.cursor_row.label(),
            cell_pitch(c),
            cell_timbre(c),
            cell_volume(c),
            cell_effect(c),
        );
        for (i, ch) in label.chars().enumerate() {
            helpers.draw_char(ch, i as f32 * TILE_WIDTH as f32, INFO_Y, COLOR_WHITE);
        }
    }

    fn draw_hint(&self, helpers: &DrawHelpers) {
        let hint = if self.header_focus {
            "tab:cells  shift+up/dn:+-10"
        } else {
            "play:sp tab:hdr 0-7 alt+up/dn"
        };
        for (i, c) in hint.chars().enumerate() {
            helpers.draw_char(c, i as f32 * TILE_WIDTH as f32, HINT_Y, COLOR_DARK_GRAY);
        }
    }
}
