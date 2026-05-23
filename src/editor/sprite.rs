use macroquad::prelude::*;

use crate::config::{
    COLOR_BLACK, COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE, SPRITE_SIZE, TILE_WIDTH,
};
use crate::input::{
    is_modifier_pressed, is_shift_pressed, is_shortcut_modifier_pressed, should_key_fire,
};
use crate::render::DrawHelpers;

const UNDO_LIMIT: usize = 32;
const SHEET_COLS: u8 = 5;

pub enum SpriteEditorAction {
    None,
    ExitToTextEditor,
    ExitToMapEditor,
    Save,
    Run,
}

pub struct SpriteEditorOutput {
    pub action: SpriteEditorAction,
    pub modified: bool,
}

impl SpriteEditorOutput {
    fn none() -> Self {
        Self {
            action: SpriteEditorAction::None,
            modified: false,
        }
    }

    fn action(action: SpriteEditorAction) -> Self {
        Self {
            action,
            modified: false,
        }
    }

    fn modified() -> Self {
        Self {
            action: SpriteEditorAction::None,
            modified: true,
        }
    }
}

pub struct SpriteEditor {
    pub data: [u8; 4096],
    selected: u8,
    cursor_x: u8,
    cursor_y: u8,
    color: u8,
    undo: Vec<(u8, [u8; SPRITE_SIZE])>,
    redo: Vec<(u8, [u8; SPRITE_SIZE])>,
    clipboard: Option<[u8; SPRITE_SIZE]>,
    painting: bool,
    sheet_col: u8,
}

impl Default for SpriteEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl SpriteEditor {
    pub fn new() -> Self {
        Self {
            data: [0; 4096],
            selected: 0,
            cursor_x: 0,
            cursor_y: 0,
            color: 1,
            undo: Vec::new(),
            redo: Vec::new(),
            clipboard: None,
            painting: false,
            sheet_col: 0,
        }
    }

    pub fn update(&mut self) -> SpriteEditorOutput {
        if is_key_pressed(KeyCode::Escape) {
            self.painting = false;
            return SpriteEditorOutput::action(if is_shift_pressed() {
                SpriteEditorAction::ExitToTextEditor
            } else {
                SpriteEditorAction::ExitToMapEditor
            });
        }

        let modifier = is_modifier_pressed();
        let shift = is_shift_pressed();

        if modifier && !shift && is_key_pressed(KeyCode::S) {
            return SpriteEditorOutput::action(SpriteEditorAction::Save);
        }
        if modifier && !shift && is_key_pressed(KeyCode::Z) {
            return self.do_undo();
        }
        if modifier && is_key_pressed(KeyCode::Y) || modifier && shift && is_key_pressed(KeyCode::Z)
        {
            return self.do_redo();
        }
        if modifier && !shift && is_key_pressed(KeyCode::C) {
            self.clipboard = Some(self.get_bytes(self.selected));
            return SpriteEditorOutput::none();
        }
        if modifier && !shift && is_key_pressed(KeyCode::V) {
            if let Some(data) = self.clipboard {
                self.push_undo();
                self.set_bytes(self.selected, &data);
                return SpriteEditorOutput::modified();
            }
            return SpriteEditorOutput::none();
        }
        if is_shortcut_modifier_pressed()
            && !shift
            && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter))
        {
            return SpriteEditorOutput::action(SpriteEditorAction::Run);
        }
        if modifier {
            return SpriteEditorOutput::none();
        }

        let alt = is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt);
        if alt {
            if is_key_pressed(KeyCode::Left) {
                self.select(self.selected.wrapping_sub(1));
            }
            if is_key_pressed(KeyCode::Right) {
                self.select(self.selected.wrapping_add(1));
            }
            if is_key_pressed(KeyCode::Up) {
                self.select(self.selected.wrapping_sub(16));
            }
            if is_key_pressed(KeyCode::Down) {
                self.select(self.selected.wrapping_add(16));
            }
            return SpriteEditorOutput::none();
        }

        if shift {
            let dir = if is_key_pressed(KeyCode::Left) {
                Some((-1i8, 0i8))
            } else if is_key_pressed(KeyCode::Right) {
                Some((1, 0))
            } else if is_key_pressed(KeyCode::Up) {
                Some((0, -1))
            } else if is_key_pressed(KeyCode::Down) {
                Some((0, 1))
            } else {
                None
            };
            let mut modified = false;
            if let Some((dx, dy)) = dir {
                self.push_undo();
                self.shift_sprite(self.selected, dx, dy);
                modified = true;
            }
            if is_key_pressed(KeyCode::F) {
                self.push_undo();
                self.flip_vertical(self.selected);
                modified = true;
            }
            return SpriteEditorOutput {
                action: SpriteEditorAction::None,
                modified,
            };
        }

        if is_key_pressed(KeyCode::F) {
            self.push_undo();
            self.flip_horizontal(self.selected);
            return SpriteEditorOutput::modified();
        }

        if is_key_pressed(KeyCode::Delete) {
            let bytes = self.get_bytes(self.selected);
            if bytes != [0; SPRITE_SIZE] {
                self.push_undo();
                self.set_bytes(self.selected, &[0; SPRITE_SIZE]);
                return SpriteEditorOutput::modified();
            }
            return SpriteEditorOutput::none();
        }

        // Color selection
        if is_key_pressed(KeyCode::Key1) {
            self.color = 0;
        }
        if is_key_pressed(KeyCode::Key2) {
            self.color = 1;
        }
        if is_key_pressed(KeyCode::Key3) {
            self.color = 2;
        }
        if is_key_pressed(KeyCode::Key4) {
            self.color = 3;
        }

        let mut moved = false;
        if should_key_fire(KeyCode::Left, false) {
            self.cursor_x = self.cursor_x.wrapping_sub(1) & 7;
            moved = true;
        }
        if should_key_fire(KeyCode::Right, false) {
            self.cursor_x = (self.cursor_x + 1) & 7;
            moved = true;
        }
        if should_key_fire(KeyCode::Up, false) {
            self.cursor_y = self.cursor_y.wrapping_sub(1) & 7;
            moved = true;
        }
        if should_key_fire(KeyCode::Down, false) {
            self.cursor_y = (self.cursor_y + 1) & 7;
            moved = true;
        }

        let mut modified = false;
        if is_key_pressed(KeyCode::Space) {
            self.push_undo();
            self.painting = true;
            self.set_pixel(self.selected, self.cursor_x, self.cursor_y, self.color);
            modified = true;
        } else if is_key_down(KeyCode::Space) {
            if moved {
                self.set_pixel(self.selected, self.cursor_x, self.cursor_y, self.color);
                modified = true;
            }
        } else {
            self.painting = false;
        }

        if is_key_pressed(KeyCode::C) {
            self.color = self.get_pixel(self.selected, self.cursor_x, self.cursor_y);
        }

        SpriteEditorOutput {
            action: SpriteEditorAction::None,
            modified,
        }
    }

    pub fn append_data(&self, content: &mut String) {
        if self.data.iter().any(|&b| b != 0) {
            content.push_str("\n__spr__\n");
            for row in self.data.chunks(16) {
                let hex: String = row.iter().map(|b| format!("{b:02x}")).collect();
                content.push_str(&hex);
                content.push('\n');
            }
        }
    }

    pub fn get_pixel(&self, sprite: u8, x: u8, y: u8) -> u8 {
        let base = sprite as usize * SPRITE_SIZE + y as usize * 2;
        let byte_idx = x as usize / 4;
        let bit_shift = 6 - (x as usize % 4) * 2;
        (self.data[base + byte_idx] >> bit_shift) & 3
    }

    pub fn draw(&self, helpers: &DrawHelpers) {
        let palette = [COLOR_BLACK, COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE];
        let zoom = 14.0f32;
        let editor_size = zoom * 8.0;

        // Current sprite zoomed
        for py in 0..8u8 {
            for px in 0..8u8 {
                let color = self.get_pixel(self.selected, px, py);
                helpers.draw_rect(
                    px as f32 * zoom,
                    py as f32 * zoom,
                    zoom,
                    zoom,
                    palette[color as usize],
                );
            }
        }

        // Grid
        let grid_color = Color::new(0.2, 0.2, 0.2, 1.0);
        for i in 1..8 {
            let pos = i as f32 * zoom;
            helpers.draw_rect(pos, 0.0, 1.0, editor_size, grid_color);
            helpers.draw_rect(0.0, pos, editor_size, 1.0, grid_color);
        }

        // Cursor highlight
        let cx = self.cursor_x as f32 * zoom;
        let cy = self.cursor_y as f32 * zoom;
        helpers.draw_rect(cx, cy, zoom, 1.0, COLOR_WHITE);
        helpers.draw_rect(cx, cy + zoom - 1.0, zoom, 1.0, COLOR_WHITE);
        helpers.draw_rect(cx, cy, 1.0, zoom, COLOR_WHITE);
        helpers.draw_rect(cx + zoom - 1.0, cy, 1.0, zoom, COLOR_WHITE);

        // Sprite sheet view
        let sheet_x = editor_size + 4.0;
        let start_col = self.sheet_col;
        for row in 0..16u8 {
            for col_off in 0..SHEET_COLS {
                let sheet_col = start_col.wrapping_add(col_off) % 16;
                let si = row * 16 + sheet_col;
                let bx = sheet_x + col_off as f32 * 8.0;
                let by = row as f32 * 8.0;
                for py in 0..8u8 {
                    for px in 0..8u8 {
                        let c = self.get_pixel(si, px, py);
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

        // Selected sprite highlight
        let sel_col = self.selected % 16;
        let col_off = sel_col.wrapping_sub(start_col) % 16;
        let sel_bx = sheet_x + col_off as f32 * 8.0;
        let sel_by = (self.selected / 16) as f32 * 8.0;
        helpers.draw_rect(sel_bx, sel_by, 8.0, 1.0, COLOR_WHITE);
        helpers.draw_rect(sel_bx, sel_by + 7.0, 8.0, 1.0, COLOR_WHITE);
        helpers.draw_rect(sel_bx, sel_by, 1.0, 8.0, COLOR_WHITE);
        helpers.draw_rect(sel_bx + 7.0, sel_by, 1.0, 8.0, COLOR_WHITE);

        // Color palette bar
        let bar_y = 134.0f32;
        for i in 0..4u8 {
            let px = 4.0 + i as f32 * 20.0;
            helpers.draw_rect(px, bar_y, 16.0, 8.0, palette[i as usize]);
            if i == 0 {
                helpers.draw_rect(px, bar_y, 16.0, 1.0, COLOR_DARK_GRAY);
                helpers.draw_rect(px, bar_y + 7.0, 16.0, 1.0, COLOR_DARK_GRAY);
                helpers.draw_rect(px, bar_y, 1.0, 8.0, COLOR_DARK_GRAY);
                helpers.draw_rect(px + 15.0, bar_y, 1.0, 8.0, COLOR_DARK_GRAY);
            }
            if i == self.color {
                helpers.draw_rect(px - 1.0, bar_y - 1.0, 18.0, 1.0, COLOR_WHITE);
                helpers.draw_rect(px - 1.0, bar_y + 8.0, 18.0, 1.0, COLOR_WHITE);
                helpers.draw_rect(px - 1.0, bar_y - 1.0, 1.0, 10.0, COLOR_WHITE);
                helpers.draw_rect(px + 16.0, bar_y - 1.0, 1.0, 10.0, COLOR_WHITE);
            }
        }

        // Sprite number
        let label = format!("#{}", self.selected);
        let label_x = 100.0;
        for (i, c) in label.chars().enumerate() {
            helpers.draw_char(
                c,
                label_x + i as f32 * TILE_WIDTH as f32,
                bar_y + 1.0,
                COLOR_WHITE,
            );
        }
    }

    fn get_bytes(&self, sprite: u8) -> [u8; SPRITE_SIZE] {
        let base = sprite as usize * SPRITE_SIZE;
        let mut data = [0u8; SPRITE_SIZE];
        data.copy_from_slice(&self.data[base..base + SPRITE_SIZE]);
        data
    }

    fn set_bytes(&mut self, sprite: u8, data: &[u8; SPRITE_SIZE]) {
        let base = sprite as usize * SPRITE_SIZE;
        self.data[base..base + SPRITE_SIZE].copy_from_slice(data);
    }

    fn set_pixel(&mut self, sprite: u8, x: u8, y: u8, color: u8) {
        let base = sprite as usize * SPRITE_SIZE + y as usize * 2;
        let byte_idx = x as usize / 4;
        let bit_shift = 6 - (x as usize % 4) * 2;
        let mask = !(3 << bit_shift);
        self.data[base + byte_idx] =
            (self.data[base + byte_idx] & mask) | ((color & 3) << bit_shift);
    }

    fn select(&mut self, idx: u8) {
        self.selected = idx;
        let sel_col = idx % 16;
        let offset = sel_col.wrapping_sub(self.sheet_col) % 16;
        if offset >= SHEET_COLS {
            if offset > 8 {
                self.sheet_col = sel_col;
            } else {
                self.sheet_col = sel_col.wrapping_sub(SHEET_COLS - 1) % 16;
            }
        }
    }

    fn push_undo(&mut self) {
        let data = self.get_bytes(self.selected);
        self.undo.push((self.selected, data));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn do_undo(&mut self) -> SpriteEditorOutput {
        if let Some((sprite, data)) = self.undo.pop() {
            let current = self.get_bytes(sprite);
            self.redo.push((sprite, current));
            self.set_bytes(sprite, &data);
            self.select(sprite);
            return SpriteEditorOutput::modified();
        }
        SpriteEditorOutput::none()
    }

    fn do_redo(&mut self) -> SpriteEditorOutput {
        if let Some((sprite, data)) = self.redo.pop() {
            let current = self.get_bytes(sprite);
            self.undo.push((sprite, current));
            self.set_bytes(sprite, &data);
            self.select(sprite);
            return SpriteEditorOutput::modified();
        }
        SpriteEditorOutput::none()
    }

    fn flip_horizontal(&mut self, sprite: u8) {
        for y in 0..8u8 {
            let mut row = [0u8; 8];
            for x in 0..8u8 {
                row[x as usize] = self.get_pixel(sprite, x, y);
            }
            for x in 0..8u8 {
                self.set_pixel(sprite, x, y, row[7 - x as usize]);
            }
        }
    }

    fn flip_vertical(&mut self, sprite: u8) {
        for y in 0..4u8 {
            for x in 0..8u8 {
                let top = self.get_pixel(sprite, x, y);
                let bot = self.get_pixel(sprite, x, 7 - y);
                self.set_pixel(sprite, x, y, bot);
                self.set_pixel(sprite, x, 7 - y, top);
            }
        }
    }

    fn shift_sprite(&mut self, sprite: u8, dx: i8, dy: i8) {
        let mut pixels = [[0u8; 8]; 8];
        for y in 0..8u8 {
            for x in 0..8u8 {
                pixels[y as usize][x as usize] = self.get_pixel(sprite, x, y);
            }
        }
        for y in 0..8i8 {
            for x in 0..8i8 {
                let sx = x - dx;
                let sy = y - dy;
                let color = if (0..8).contains(&sx) && (0..8).contains(&sy) {
                    pixels[sy as usize][sx as usize]
                } else {
                    0
                };
                self.set_pixel(sprite, x as u8, y as u8, color);
            }
        }
    }
}
