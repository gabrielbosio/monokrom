use std::collections::VecDeque;

use macroquad::prelude::*;

use crate::audio::format::SoundData;
use crate::config::{COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE, TILE_WIDTH};
use crate::input::{
    is_arrow_modifier_pressed, is_shift_pressed, is_shortcut_modifier_pressed, should_key_fire,
};
use crate::render::DrawHelpers;

const UNDO_LIMIT: usize = 16;
const HEADER_Y: f32 = 0.0;
const TOP: f32 = 24.0;
const BOTTOM: f32 = 84.0;
const CELL_PX: f32 = 5.0;
const LABEL_Y: f32 = 90.0;

pub enum WaveEditorAction {
    None,
    ExitToTerminal,
    Save,
    Run,
    Preview(u8),
    Stop,
}

pub struct WaveEditorOutput {
    pub action: WaveEditorAction,
    pub modified: bool,
}

impl WaveEditorOutput {
    fn none() -> Self {
        Self {
            action: WaveEditorAction::None,
            modified: false,
        }
    }

    fn action(action: WaveEditorAction) -> Self {
        Self {
            action,
            modified: false,
        }
    }

    fn modified() -> Self {
        Self {
            action: WaveEditorAction::None,
            modified: true,
        }
    }
}

pub struct WaveEditor {
    wave: u8,
    sample: u8,
    undo: VecDeque<SoundData>,
    redo: VecDeque<SoundData>,
    clipboard: Option<[u8; 16]>,
}

impl Default for WaveEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl WaveEditor {
    pub fn new() -> Self {
        Self {
            wave: 0,
            sample: 0,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            clipboard: None,
        }
    }

    pub fn wave(&self) -> u8 {
        self.wave
    }

    pub fn update(&mut self, data: &mut SoundData) -> WaveEditorOutput {
        if is_key_pressed(KeyCode::Escape) {
            return WaveEditorOutput::action(WaveEditorAction::ExitToTerminal);
        }

        let modifier = is_shortcut_modifier_pressed();
        let shift = is_shift_pressed();

        if modifier && !shift && is_key_pressed(KeyCode::S) {
            return WaveEditorOutput::action(WaveEditorAction::Save);
        }
        if modifier
            && !shift
            && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter))
        {
            return WaveEditorOutput::action(WaveEditorAction::Run);
        }
        if modifier && !shift && is_key_pressed(KeyCode::Z) {
            return self.do_undo(data);
        }
        if (modifier && is_key_pressed(KeyCode::Y))
            || (modifier && shift && is_key_pressed(KeyCode::Z))
        {
            return self.do_redo(data);
        }

        if modifier && !shift && is_key_pressed(KeyCode::C) {
            self.clipboard = Some(data.waves[self.wave as usize]);
            return WaveEditorOutput::none();
        }
        if modifier && !shift && is_key_pressed(KeyCode::V) {
            if let Some(wave) = self.clipboard {
                self.push_undo(data);
                data.waves[self.wave as usize] = wave;
                return WaveEditorOutput::modified();
            }
            return WaveEditorOutput::none();
        }

        if is_key_pressed(KeyCode::Space) {
            return WaveEditorOutput::action(if shift {
                WaveEditorAction::Stop
            } else {
                WaveEditorAction::Preview(self.wave)
            });
        }

        if is_arrow_modifier_pressed()
            && (is_key_down(KeyCode::Left) || is_key_down(KeyCode::Right))
        {
            if should_key_fire(KeyCode::Left, false) {
                self.wave = (self.wave + 15) % 16;
            } else if should_key_fire(KeyCode::Right, false) {
                self.wave = (self.wave + 1) % 16;
            }
            return WaveEditorOutput::none();
        }

        let left = should_key_fire(KeyCode::Left, false);
        let right = should_key_fire(KeyCode::Right, false);
        let up = should_key_fire(KeyCode::Up, false);
        let down = should_key_fire(KeyCode::Down, false);

        if left || right {
            let sign = if right { 1 } else { -1 };
            let step = if shift { 8 } else { 1 };
            self.sample = (self.sample as i32 + sign * step).rem_euclid(32) as u8;
        }

        if up || down {
            self.push_undo(data);
            let sign = if up { 1 } else { -1 };
            let step = if shift { 4 } else { 1 };
            let wave = &mut data.waves[self.wave as usize];
            let byte = &mut wave[(self.sample / 2) as usize];
            let shift = if self.sample.is_multiple_of(2) { 4 } else { 0 };
            let current = ((*byte >> shift) & 0x0F) as i32;
            let next = (current + sign * step).clamp(0, 15) as u8;
            *byte = (*byte & !(0x0F << shift)) | (next << shift);
            return WaveEditorOutput::modified();
        }

        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            self.push_undo(data);
            data.waves[self.wave as usize] = [0; 16];
            return WaveEditorOutput::modified();
        }

        WaveEditorOutput::none()
    }

    fn push_undo(&mut self, data: &SoundData) {
        self.undo.push_back(data.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    fn do_undo(&mut self, data: &mut SoundData) -> WaveEditorOutput {
        if let Some(snapshot) = self.undo.pop_back() {
            self.redo.push_back(data.clone());
            *data = snapshot;
            return WaveEditorOutput::modified();
        }
        WaveEditorOutput::none()
    }

    fn do_redo(&mut self, data: &mut SoundData) -> WaveEditorOutput {
        if let Some(snapshot) = self.redo.pop_back() {
            self.undo.push_back(data.clone());
            *data = snapshot;
            return WaveEditorOutput::modified();
        }
        WaveEditorOutput::none()
    }

    pub fn draw(&self, data: &SoundData, helpers: &DrawHelpers) {
        draw_text(
            helpers,
            0.0,
            HEADER_Y,
            &format!("WAVE {:02}", self.wave),
            COLOR_WHITE,
        );

        helpers.draw_rect(0.0, TOP, 160.0, 1.0, COLOR_DARK_GRAY);
        helpers.draw_rect(0.0, BOTTOM, 160.0, 1.0, COLOR_DARK_GRAY);

        let wave = &data.waves[self.wave as usize];
        for index in 0..32 {
            let byte = wave[index / 2];
            let value = if index.is_multiple_of(2) {
                byte >> 4
            } else {
                byte & 0x0F
            };
            let selected = index as u8 == self.sample;
            let color = if selected {
                COLOR_WHITE
            } else {
                COLOR_LIGHT_GRAY
            };
            let x = index as f32 * CELL_PX + 1.0;
            let height = (value as f32 / 15.0 * (BOTTOM - TOP)).max(1.0);
            helpers.draw_rect(x, BOTTOM - height, CELL_PX - 2.0, height, color);
            let c = char::from_digit(value as u32, 16).unwrap_or('?');
            helpers.draw_char(c, x, LABEL_Y, color);
        }
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
    use crate::config::WAVE_TABLE_COUNT;

    #[test]
    fn wave_editor_clears_selected_wave() {
        let mut editor = WaveEditor::new();
        let mut data = SoundData::default();
        data.waves[0][0] = 0xFF;
        editor.wave = 0;
        editor.push_undo(&data);
        data.waves[0] = [0; 16];
        assert!(data
            .waves
            .iter()
            .take(WAVE_TABLE_COUNT)
            .all(|w| w == &[0; 16]));
    }
}
