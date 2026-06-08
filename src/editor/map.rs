use std::collections::VecDeque;

use macroquad::prelude::*;

use crate::config::{COLOR_BLACK, COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE, TILE_WIDTH};
use crate::editor::sprite::SpriteEditor;
use crate::input::{
    is_modifier_pressed, is_shift_pressed, is_shortcut_modifier_pressed, should_key_fire,
};
use crate::render::DrawHelpers;

const UNDO_LIMIT: usize = 64;
const VIEWPORT_TILES: u16 = 16;
const PEEK_TILES_X: u16 = 20;
const PEEK_TILES_Y: u16 = 18;
const PICKER_COLS: u8 = 4;

pub enum MapEditorAction {
    None,
    ExitToSpriteEditor,
    ExitToSfxEditor,
    Save,
    Run,
}

pub struct MapEditorOutput {
    pub action: MapEditorAction,
    pub modified: bool,
}

impl MapEditorOutput {
    fn none() -> Self {
        Self {
            action: MapEditorAction::None,
            modified: false,
        }
    }

    fn action(action: MapEditorAction) -> Self {
        Self {
            action,
            modified: false,
        }
    }

    fn modified() -> Self {
        Self {
            action: MapEditorAction::None,
            modified: true,
        }
    }
}

pub struct MapEditor {
    pub data: [u8; 4096],
    cursor_x: u16,
    cursor_y: u16,
    selected_tile: u8,
    undo: VecDeque<(u16, u16, u8)>,
    redo: VecDeque<(u16, u16, u8)>,
    viewport_x: u16,
    viewport_y: u16,
    painting: bool,
    peeking: bool,
    picker_col: u8,
}

impl Default for MapEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl MapEditor {
    pub fn new() -> Self {
        Self {
            data: [0; 4096],
            cursor_x: 0,
            cursor_y: 0,
            selected_tile: 0,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            viewport_x: 0,
            viewport_y: 0,
            painting: false,
            peeking: false,
            picker_col: 0,
        }
    }

    pub fn update(&mut self) -> MapEditorOutput {
        if is_key_pressed(KeyCode::Escape) {
            self.painting = false;
            self.peeking = false;
            return MapEditorOutput::action(if is_shift_pressed() {
                MapEditorAction::ExitToSpriteEditor
            } else {
                MapEditorAction::ExitToSfxEditor
            });
        }

        self.peeking = is_key_down(KeyCode::Tab);
        if self.peeking {
            self.painting = false;
            return MapEditorOutput::none();
        }

        let modifier = is_modifier_pressed();
        let shift = is_shift_pressed();

        if modifier && !shift && is_key_pressed(KeyCode::S) {
            return MapEditorOutput::action(MapEditorAction::Save);
        }
        if modifier && !shift && is_key_pressed(KeyCode::Z) {
            return self.do_undo();
        }
        if (modifier && is_key_pressed(KeyCode::Y))
            || (modifier && shift && is_key_pressed(KeyCode::Z))
        {
            return self.do_redo();
        }
        if is_shortcut_modifier_pressed()
            && !shift
            && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter))
        {
            return MapEditorOutput::action(MapEditorAction::Run);
        }
        if modifier {
            return MapEditorOutput::none();
        }

        let alt = is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt);
        if alt {
            if is_key_pressed(KeyCode::Left) {
                self.select_tile(self.selected_tile.wrapping_sub(1));
            }
            if is_key_pressed(KeyCode::Right) {
                self.select_tile(self.selected_tile.wrapping_add(1));
            }
            if is_key_pressed(KeyCode::Up) {
                self.select_tile(self.selected_tile.wrapping_sub(16));
            }
            if is_key_pressed(KeyCode::Down) {
                self.select_tile(self.selected_tile.wrapping_add(16));
            }
            return MapEditorOutput::none();
        }

        let fast = is_shift_pressed();
        let step: u16 = if fast { 8 } else { 1 };

        let mut moved = false;
        if should_key_fire(KeyCode::Left, fast) && self.cursor_x > 0 {
            self.cursor_x = self.cursor_x.saturating_sub(step);
            moved = true;
        }
        if should_key_fire(KeyCode::Right, fast) && self.cursor_x < 127 {
            self.cursor_x = (self.cursor_x + step).min(127);
            moved = true;
        }
        if should_key_fire(KeyCode::Up, fast) && self.cursor_y > 0 {
            self.cursor_y = self.cursor_y.saturating_sub(step);
            moved = true;
        }
        if should_key_fire(KeyCode::Down, fast) && self.cursor_y < 31 {
            self.cursor_y = (self.cursor_y + step).min(31);
            moved = true;
        }

        if self.cursor_x < self.viewport_x {
            self.viewport_x = self.cursor_x;
        }
        if self.cursor_x >= self.viewport_x + VIEWPORT_TILES {
            self.viewport_x = self.cursor_x - (VIEWPORT_TILES - 1);
        }
        if self.cursor_y < self.viewport_y {
            self.viewport_y = self.cursor_y;
        }
        if self.cursor_y >= self.viewport_y + VIEWPORT_TILES {
            self.viewport_y = self.cursor_y - (VIEWPORT_TILES - 1);
        }

        let mut modified = false;
        if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
            self.painting = true;
            if self.place_tile(self.selected_tile) {
                modified = true;
            }
        } else if is_key_down(KeyCode::Space) {
            if moved && self.place_tile(self.selected_tile) {
                modified = true;
            }
        } else {
            self.painting = false;
        }

        if (is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace))
            && self.place_tile(0)
        {
            modified = true;
        }

        if is_key_pressed(KeyCode::C) {
            let idx = self.cursor_y as usize * 128 + self.cursor_x as usize;
            self.select_tile(self.data[idx]);
        }

        MapEditorOutput {
            action: MapEditorAction::None,
            modified,
        }
    }

    pub fn append_data(&self, content: &mut String) {
        if self.data.iter().any(|&b| b != 0) {
            content.push_str("\n__map__\n");
            for row in self.data.chunks(16) {
                let hex: String = row.iter().map(|b| format!("{b:02x}")).collect();
                content.push_str(&hex);
                content.push('\n');
            }
        }
    }

    pub fn draw(&self, sprites: &SpriteEditor, helpers: &DrawHelpers) {
        let palette = [COLOR_BLACK, COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE];

        // Map viewport
        let (cols, rows) = if self.peeking {
            (PEEK_TILES_X, PEEK_TILES_Y)
        } else {
            (VIEWPORT_TILES, VIEWPORT_TILES)
        };
        for ty in 0..rows {
            for tx in 0..cols {
                let mx = self.viewport_x + tx;
                let my = self.viewport_y + ty;
                if mx < 128 && my < 32 {
                    let tile = self.data[my as usize * 128 + mx as usize];
                    let sx = tx as f32 * 8.0;
                    let sy = ty as f32 * 8.0;
                    for py in 0..8u8 {
                        for px in 0..8u8 {
                            let c = sprites.get_pixel(tile, px, py);
                            if c != 0 {
                                helpers.draw_rect(
                                    sx + px as f32,
                                    sy + py as f32,
                                    1.0,
                                    1.0,
                                    palette[c as usize],
                                );
                            }
                        }
                    }
                }
            }
        }

        if self.peeking {
            return;
        }

        // Grid
        let grid_color = Color::new(0.15, 0.15, 0.15, 1.0);
        for i in 1..VIEWPORT_TILES {
            let pos = i as f32 * 8.0;
            helpers.draw_rect(pos, 0.0, 1.0, 128.0, grid_color);
            helpers.draw_rect(0.0, pos, 128.0, 1.0, grid_color);
        }

        // Cursor
        let cx = (self.cursor_x - self.viewport_x) as f32 * 8.0;
        let cy = (self.cursor_y - self.viewport_y) as f32 * 8.0;
        helpers.draw_rect(cx, cy, 8.0, 1.0, COLOR_WHITE);
        helpers.draw_rect(cx, cy + 7.0, 8.0, 1.0, COLOR_WHITE);
        helpers.draw_rect(cx, cy, 1.0, 8.0, COLOR_WHITE);
        helpers.draw_rect(cx + 7.0, cy, 1.0, 8.0, COLOR_WHITE);

        // Sprite picker
        let sheet_x = 128.0f32;
        let picker_col_offset = self.picker_col;
        for row in 0..16u8 {
            for col in 0..PICKER_COLS {
                let si = row * 16 + picker_col_offset + col;
                let bx = sheet_x + col as f32 * 8.0;
                let by = row as f32 * 8.0;
                for py in 0..8u8 {
                    for px in 0..8u8 {
                        let c = sprites.get_pixel(si, px, py);
                        if c != 0 {
                            helpers.draw_rect(
                                bx + px as f32,
                                by + py as f32,
                                1.0,
                                1.0,
                                palette[c as usize],
                            );
                        }
                    }
                }
            }
        }

        // Picker selection highlight
        let sel_row = self.selected_tile / 16;
        let sel_col = self.selected_tile % 16 - picker_col_offset;
        if sel_col < PICKER_COLS {
            let sx = sheet_x + sel_col as f32 * 8.0;
            let sy = sel_row as f32 * 8.0;
            helpers.draw_rect(sx, sy, 8.0, 1.0, COLOR_WHITE);
            helpers.draw_rect(sx, sy + 7.0, 8.0, 1.0, COLOR_WHITE);
            helpers.draw_rect(sx, sy, 1.0, 8.0, COLOR_WHITE);
            helpers.draw_rect(sx + 7.0, sy, 1.0, 8.0, COLOR_WHITE);
        }

        // Status bar
        let tw = TILE_WIDTH as f32;
        let label = format!(
            "t:{} {},{}",
            self.selected_tile, self.cursor_x, self.cursor_y
        );
        for (i, ch) in label.chars().enumerate() {
            helpers.draw_char(ch, i as f32 * tw, 130.0, COLOR_WHITE);
        }

        // Selected tile preview
        for py in 0..8u8 {
            for px in 0..8u8 {
                let c = sprites.get_pixel(self.selected_tile, px, py);
                if c != 0 {
                    helpers.draw_rect(
                        144.0 + px as f32,
                        130.0 + py as f32,
                        1.0,
                        1.0,
                        palette[c as usize],
                    );
                }
            }
        }
    }

    fn select_tile(&mut self, idx: u8) {
        self.selected_tile = idx;
        let sel_col = idx % 16;
        if sel_col < self.picker_col {
            self.picker_col = sel_col;
        } else if sel_col >= self.picker_col + PICKER_COLS {
            self.picker_col = sel_col + 1 - PICKER_COLS;
        }
    }

    /// Place `tile` at the cursor position. Returns true if the tile changed.
    fn place_tile(&mut self, tile: u8) -> bool {
        let idx = self.cursor_y as usize * 128 + self.cursor_x as usize;
        let old = self.data[idx];
        if old == tile {
            return false;
        }
        self.undo.push_back((self.cursor_x, self.cursor_y, old));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.redo.clear();
        self.data[idx] = tile;
        true
    }

    fn do_undo(&mut self) -> MapEditorOutput {
        if let Some((x, y, old_tile)) = self.undo.pop_back() {
            let idx = y as usize * 128 + x as usize;
            let current = self.data[idx];
            self.redo.push_back((x, y, current));
            self.data[idx] = old_tile;
            return MapEditorOutput::modified();
        }
        MapEditorOutput::none()
    }

    fn do_redo(&mut self) -> MapEditorOutput {
        if let Some((x, y, tile)) = self.redo.pop_back() {
            let idx = y as usize * 128 + x as usize;
            let current = self.data[idx];
            self.undo.push_back((x, y, current));
            self.data[idx] = tile;
            return MapEditorOutput::modified();
        }
        MapEditorOutput::none()
    }
}
