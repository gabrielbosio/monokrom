use std::collections::VecDeque;

use macroquad::prelude::*;

use crate::config::{
    COLOR_BLACK, COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE, SPRITE_SIZE, TILE_WIDTH,
};
use crate::input::{
    is_arrow_modifier_pressed, is_shift_pressed, is_shortcut_modifier_pressed, should_key_fire,
};
use crate::render::DrawHelpers;

const UNDO_LIMIT: usize = 32;
const SHEET_ROWS: u8 = 16;
const SHEET_COLS: u8 = 11;
const SHEET_TILE: f32 = 8.0;
const PANEL_SIZE: f32 = 64.0;
const COARSE_STEP_TILES: u8 = 4;
const SPRITE_SIZES: [u8; 4] = [8, 16, 32, 64];
const ZOOMS: [u8; 4] = [8, 4, 2, 1];

struct SpriteBlock {
    tiles: u8,
    data: Vec<u8>,
}

struct Snapshot {
    base: u8,
    block: SpriteBlock,
}

pub enum SpriteEditorAction {
    None,
    ExitToTerminal,
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
    undo: VecDeque<Snapshot>,
    redo: VecDeque<Snapshot>,
    clipboard: Option<SpriteBlock>,
    painting: bool,
    sheet_col: u8,
    zoom_index: usize,
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
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            clipboard: None,
            painting: false,
            sheet_col: 0,
            zoom_index: 0,
        }
    }

    pub fn update(&mut self) -> SpriteEditorOutput {
        if is_key_pressed(KeyCode::Escape) {
            self.painting = false;
            return SpriteEditorOutput::action(SpriteEditorAction::ExitToTerminal);
        }

        let modifier = is_shortcut_modifier_pressed();
        let shift = is_shift_pressed();

        if modifier && !shift && is_key_pressed(KeyCode::S) {
            return SpriteEditorOutput::action(SpriteEditorAction::Save);
        }
        if modifier && !shift && is_key_pressed(KeyCode::Z) {
            return self.do_undo();
        }
        if (modifier && is_key_pressed(KeyCode::Y))
            || (modifier && shift && is_key_pressed(KeyCode::Z))
        {
            return self.do_redo();
        }
        if modifier && !shift && is_key_pressed(KeyCode::C) {
            self.clipboard = Some(self.get_block(self.selected, self.tile_span()));
            return SpriteEditorOutput::none();
        }
        if modifier && !shift && is_key_pressed(KeyCode::V) {
            if let Some(block) = self.clipboard.take() {
                if block.tiles == self.tile_span() {
                    self.push_undo();
                    self.set_block(self.selected, &block);
                    self.clipboard = Some(block);
                    return SpriteEditorOutput::modified();
                }
                self.clipboard = Some(block);
            }
            return SpriteEditorOutput::none();
        }
        if modifier
            && !shift
            && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter))
        {
            return SpriteEditorOutput::action(SpriteEditorAction::Run);
        }

        if !modifier
            && (is_key_pressed(KeyCode::Equal)
                || is_key_pressed(KeyCode::KpAdd)
                || is_key_pressed(KeyCode::KpEqual))
        {
            self.zoom_in();
            return SpriteEditorOutput::none();
        }
        if !modifier && (is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract)) {
            self.zoom_out();
            return SpriteEditorOutput::none();
        }

        if is_arrow_modifier_pressed() {
            let step = self.selection_step(shift);
            if is_key_pressed(KeyCode::Left) {
                self.move_selection(-step, 0);
            }
            if is_key_pressed(KeyCode::Right) {
                self.move_selection(step, 0);
            }
            if is_key_pressed(KeyCode::Up) {
                self.move_selection(0, -step);
            }
            if is_key_pressed(KeyCode::Down) {
                self.move_selection(0, step);
            }
            return SpriteEditorOutput::none();
        }
        if modifier {
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
                self.shift_sprite(dx, dy);
                modified = true;
            }
            if is_key_pressed(KeyCode::F) {
                self.push_undo();
                self.flip_vertical();
                modified = true;
            }
            return SpriteEditorOutput {
                action: SpriteEditorAction::None,
                modified,
            };
        }

        if is_key_pressed(KeyCode::F) {
            self.push_undo();
            self.flip_horizontal();
            return SpriteEditorOutput::modified();
        }

        if is_key_pressed(KeyCode::Delete) {
            let block = self.get_block(self.selected, self.tile_span());
            if block.data.iter().any(|&b| b != 0) {
                self.push_undo();
                self.clear_sprite();
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

        let mask = self.sprite_size() - 1;
        let mut moved = false;
        if should_key_fire(KeyCode::Left, false) {
            self.cursor_x = self.cursor_x.wrapping_sub(1) & mask;
            moved = true;
        }
        if should_key_fire(KeyCode::Right, false) {
            self.cursor_x = (self.cursor_x + 1) & mask;
            moved = true;
        }
        if should_key_fire(KeyCode::Up, false) {
            self.cursor_y = self.cursor_y.wrapping_sub(1) & mask;
            moved = true;
        }
        if should_key_fire(KeyCode::Down, false) {
            self.cursor_y = (self.cursor_y + 1) & mask;
            moved = true;
        }

        let mut modified = false;
        if is_key_pressed(KeyCode::Space) {
            self.push_undo();
            self.painting = true;
            self.set_sprite_pixel(self.cursor_x, self.cursor_y, self.color);
            modified = true;
        } else if is_key_down(KeyCode::Space) {
            if moved {
                self.set_sprite_pixel(self.cursor_x, self.cursor_y, self.color);
                modified = true;
            }
        } else {
            self.painting = false;
        }

        if is_key_pressed(KeyCode::C) {
            self.color = self.get_sprite_pixel(self.cursor_x, self.cursor_y);
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
        let size = self.sprite_size();
        let zoom = self.zoom() as f32;
        let tile_span = self.tile_span();
        let sprite_x = 1.0;
        let sprite_y = 1.0;
        let sheet_x = PANEL_SIZE + 4.0;
        let sheet_y = 1.0;

        // Current sprite zoomed. The panel is always 64x64.
        for py in 0..size {
            for px in 0..size {
                let color = self.get_sprite_pixel(px, py);
                helpers.draw_rect(
                    sprite_x + px as f32 * zoom,
                    sprite_y + py as f32 * zoom,
                    zoom,
                    zoom,
                    palette[color as usize],
                );
            }
        }

        // Sprite sheet view
        for row in 0..SHEET_ROWS {
            for col in 0..SHEET_COLS {
                let sheet_col = self.sheet_col + col;
                let si = row * 16 + sheet_col;
                let bx = sheet_x + col as f32 * SHEET_TILE;
                let by = sheet_y + row as f32 * SHEET_TILE;
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

        let sel_col = self.selected % 16;
        let sel_row = self.selected / 16;

        // Panel borders
        Self::draw_panel_border(
            helpers,
            sprite_x - 1.0,
            sprite_y - 1.0,
            PANEL_SIZE + 2.0,
            PANEL_SIZE + 2.0,
        );
        Self::draw_panel_border(
            helpers,
            sheet_x - 1.0,
            sheet_y - 1.0,
            SHEET_COLS as f32 * SHEET_TILE + 2.0,
            SHEET_ROWS as f32 * SHEET_TILE + 2.0,
        );

        // Tile panel scrollbar
        let track_w = SHEET_COLS as f32 * SHEET_TILE;
        let thumb_w = (track_w * SHEET_COLS as f32 / 16.0).round().max(1.0);
        let max_start = (16 - SHEET_COLS) as f32;
        let thumb_x = (track_w - thumb_w) * self.sheet_col as f32 / max_start;
        helpers.draw_rect(
            sheet_x + thumb_x.round(),
            sheet_y + SHEET_ROWS as f32 * SHEET_TILE,
            thumb_w,
            1.0,
            COLOR_WHITE,
        );

        // Spritesheet cursor, drawn outside the sprite
        if sel_col >= self.sheet_col && sel_col + tile_span <= self.sheet_col + SHEET_COLS {
            let span = tile_span as f32 * SHEET_TILE;
            let sel_bx = sheet_x + (sel_col - self.sheet_col) as f32 * SHEET_TILE;
            let sel_by = sheet_y + sel_row as f32 * SHEET_TILE;
            Self::draw_outline(
                helpers,
                sel_bx - 1.0,
                sel_by - 1.0,
                span + 2.0,
                span + 2.0,
                COLOR_WHITE,
            );
        }

        // Sprite cursor, drawn outside the pixel
        let cx = sprite_x + self.cursor_x as f32 * zoom;
        let cy = sprite_y + self.cursor_y as f32 * zoom;
        Self::draw_outline(
            helpers,
            cx - 1.0,
            cy - 1.0,
            zoom + 2.0,
            zoom + 2.0,
            COLOR_WHITE,
        );

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

        // Sprite number and zoom
        let label = format!("#{:03} {}x", self.selected, self.zoom());
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

    fn draw_panel_border(helpers: &DrawHelpers, x: f32, y: f32, w: f32, h: f32) {
        Self::draw_outline(helpers, x, y, w, h, COLOR_LIGHT_GRAY);
    }

    fn draw_outline(helpers: &DrawHelpers, x: f32, y: f32, w: f32, h: f32, color: Color) {
        helpers.draw_rect(x, y, w, 1.0, color);
        helpers.draw_rect(x, y + h - 1.0, w, 1.0, color);
        helpers.draw_rect(x, y, 1.0, h, color);
        helpers.draw_rect(x + w - 1.0, y, 1.0, h, color);
    }

    fn sprite_size(&self) -> u8 {
        SPRITE_SIZES[self.zoom_index]
    }

    fn zoom(&self) -> u8 {
        ZOOMS[self.zoom_index]
    }

    fn tile_span(&self) -> u8 {
        self.sprite_size() / 8
    }

    fn zoom_in(&mut self) {
        if self.zoom_index > 0 {
            self.set_zoom_index(self.zoom_index - 1);
        }
    }

    fn zoom_out(&mut self) {
        if self.zoom_index + 1 < SPRITE_SIZES.len() {
            self.set_zoom_index(self.zoom_index + 1);
        }
    }

    fn set_zoom_index(&mut self, index: usize) {
        self.zoom_index = index;
        let max = self.sprite_size() - 1;
        self.cursor_x = self.cursor_x.min(max);
        self.cursor_y = self.cursor_y.min(max);
        self.painting = false;
        self.select(self.selected);
    }

    fn select(&mut self, idx: u8) {
        let max_pos = 16 - self.tile_span();
        let col = (idx % 16).min(max_pos);
        let row = (idx / 16).min(max_pos);
        self.selected = row * 16 + col;
        self.ensure_selected_visible();
    }

    fn selection_step(&self, coarse: bool) -> i8 {
        if coarse {
            COARSE_STEP_TILES as i8
        } else {
            1
        }
    }

    fn move_selection(&mut self, dx: i8, dy: i8) {
        let positions = (16 - self.tile_span() + 1) as i8;
        let col = (self.selected % 16) as i8;
        let row = (self.selected / 16) as i8;
        let new_col = (col + dx).rem_euclid(positions) as u8;
        let new_row = (row + dy).rem_euclid(positions) as u8;
        self.select(new_row * 16 + new_col);
    }

    fn ensure_selected_visible(&mut self) {
        let max_start = SHEET_ROWS - SHEET_COLS;
        self.sheet_col = self.sheet_col.min(max_start);
        let sel_col = self.selected % 16;
        let span = self.tile_span();
        if sel_col < self.sheet_col {
            self.sheet_col = sel_col;
        } else if sel_col + span > self.sheet_col + SHEET_COLS {
            self.sheet_col = (sel_col + span - SHEET_COLS).min(max_start);
        }
    }

    fn tile_at(&self, x: u8, y: u8) -> u8 {
        let col = self.selected % 16 + x / 8;
        let row = self.selected / 16 + y / 8;
        row * 16 + col
    }

    fn get_sprite_pixel(&self, x: u8, y: u8) -> u8 {
        let tile = self.tile_at(x, y);
        self.get_pixel(tile, x % 8, y % 8)
    }

    fn set_sprite_pixel(&mut self, x: u8, y: u8, color: u8) {
        let tile = self.tile_at(x, y);
        self.set_pixel(tile, x % 8, y % 8, color);
    }

    fn set_pixel(&mut self, sprite: u8, x: u8, y: u8, color: u8) {
        let base = sprite as usize * SPRITE_SIZE + y as usize * 2;
        let byte_idx = x as usize / 4;
        let bit_shift = 6 - (x as usize % 4) * 2;
        let mask = !(3 << bit_shift);
        self.data[base + byte_idx] =
            (self.data[base + byte_idx] & mask) | ((color & 3) << bit_shift);
    }

    fn get_block(&self, base: u8, tiles: u8) -> SpriteBlock {
        let mut data = Vec::with_capacity(tiles as usize * tiles as usize * SPRITE_SIZE);
        for ty in 0..tiles {
            for tx in 0..tiles {
                let idx = base as usize + ty as usize * 16 + tx as usize;
                let start = idx * SPRITE_SIZE;
                data.extend_from_slice(&self.data[start..start + SPRITE_SIZE]);
            }
        }
        SpriteBlock { tiles, data }
    }

    fn set_block(&mut self, base: u8, block: &SpriteBlock) {
        for ty in 0..block.tiles {
            for tx in 0..block.tiles {
                let src = (ty as usize * block.tiles as usize + tx as usize) * SPRITE_SIZE;
                let idx = base as usize + ty as usize * 16 + tx as usize;
                let dst = idx * SPRITE_SIZE;
                self.data[dst..dst + SPRITE_SIZE]
                    .copy_from_slice(&block.data[src..src + SPRITE_SIZE]);
            }
        }
    }

    fn push_undo(&mut self) {
        let base = self.selected;
        let block = self.get_block(base, self.tile_span());
        self.undo.push_back(Snapshot { base, block });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    fn do_undo(&mut self) -> SpriteEditorOutput {
        if let Some(snapshot) = self.undo.pop_back() {
            let current = self.get_block(snapshot.base, snapshot.block.tiles);
            self.redo.push_back(Snapshot {
                base: snapshot.base,
                block: current,
            });
            self.set_block(snapshot.base, &snapshot.block);
            self.select(snapshot.base);
            return SpriteEditorOutput::modified();
        }
        SpriteEditorOutput::none()
    }

    fn do_redo(&mut self) -> SpriteEditorOutput {
        if let Some(snapshot) = self.redo.pop_back() {
            let current = self.get_block(snapshot.base, snapshot.block.tiles);
            self.undo.push_back(Snapshot {
                base: snapshot.base,
                block: current,
            });
            self.set_block(snapshot.base, &snapshot.block);
            self.select(snapshot.base);
            return SpriteEditorOutput::modified();
        }
        SpriteEditorOutput::none()
    }

    fn clear_sprite(&mut self) {
        let base = self.selected;
        let span = self.tile_span();
        for ty in 0..span {
            for tx in 0..span {
                let idx = base as usize + ty as usize * 16 + tx as usize;
                let start = idx * SPRITE_SIZE;
                self.data[start..start + SPRITE_SIZE].fill(0);
            }
        }
    }

    fn flip_horizontal(&mut self) {
        let size = self.sprite_size();
        for y in 0..size {
            for x in 0..size / 2 {
                let left = self.get_sprite_pixel(x, y);
                let right = self.get_sprite_pixel(size - 1 - x, y);
                self.set_sprite_pixel(x, y, right);
                self.set_sprite_pixel(size - 1 - x, y, left);
            }
        }
    }

    fn flip_vertical(&mut self) {
        let size = self.sprite_size();
        for y in 0..size / 2 {
            for x in 0..size {
                let top = self.get_sprite_pixel(x, y);
                let bot = self.get_sprite_pixel(x, size - 1 - y);
                self.set_sprite_pixel(x, y, bot);
                self.set_sprite_pixel(x, size - 1 - y, top);
            }
        }
    }

    fn shift_sprite(&mut self, dx: i8, dy: i8) {
        let size = self.sprite_size();
        let mut pixels = vec![0u8; size as usize * size as usize];
        for y in 0..size {
            for x in 0..size {
                pixels[y as usize * size as usize + x as usize] = self.get_sprite_pixel(x, y);
            }
        }
        for y in 0..size as i16 {
            for x in 0..size as i16 {
                let sx = x - dx as i16;
                let sy = y - dy as i16;
                let color = if (0..size as i16).contains(&sx) && (0..size as i16).contains(&sy) {
                    pixels[sy as usize * size as usize + sx as usize]
                } else {
                    0
                };
                self.set_sprite_pixel(x as u8, y as u8, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_zoom_fills_the_fixed_panel() {
        let mut editor = SpriteEditor::new();
        for index in 0..SPRITE_SIZES.len() {
            editor.set_zoom_index(index);
            assert_eq!(editor.sprite_size() as u16 * editor.zoom() as u16, 64);
        }
    }

    #[test]
    fn zoom_steps_through_four_levels() {
        let mut editor = SpriteEditor::new();
        assert_eq!(editor.zoom(), 8);

        editor.zoom_in();
        assert_eq!(editor.zoom(), 8);
        editor.zoom_out();
        assert_eq!(editor.zoom(), 4);
        editor.zoom_out();
        assert_eq!(editor.zoom(), 2);
        editor.zoom_out();
        assert_eq!(editor.zoom(), 1);
        editor.zoom_out();
        assert_eq!(editor.zoom(), 1);

        editor.zoom_in();
        assert_eq!(editor.zoom(), 2);
        editor.zoom_in();
        assert_eq!(editor.zoom(), 4);
        editor.zoom_in();
        assert_eq!(editor.zoom(), 8);
    }

    #[test]
    fn selection_step_is_one_or_four_sprites_at_every_zoom() {
        let mut editor = SpriteEditor::new();
        for index in 0..SPRITE_SIZES.len() {
            editor.set_zoom_index(index);
            assert_eq!(editor.selection_step(false), 1);
            assert_eq!(editor.selection_step(true), 4);
        }
    }

    #[test]
    fn pixels_map_across_tiles_of_large_sprite() {
        let mut editor = SpriteEditor::new();
        editor.set_zoom_index(1);

        editor.set_sprite_pixel(0, 0, 1);
        editor.set_sprite_pixel(8, 0, 2);
        editor.set_sprite_pixel(0, 8, 3);
        editor.set_sprite_pixel(8, 8, 1);

        assert_eq!(editor.get_pixel(0, 0, 0), 1);
        assert_eq!(editor.get_pixel(1, 0, 0), 2);
        assert_eq!(editor.get_pixel(16, 0, 0), 3);
        assert_eq!(editor.get_pixel(17, 0, 0), 1);
        assert_eq!(editor.get_sprite_pixel(8, 8), 1);
    }

    #[test]
    fn selection_moves_one_sprite_at_every_zoom() {
        let mut editor = SpriteEditor::new();
        for index in 0..SPRITE_SIZES.len() {
            editor.set_zoom_index(index);
            editor.select(0);
            editor.move_selection(1, 0);
            assert_eq!(editor.selected, 1);
            editor.move_selection(0, 1);
            assert_eq!(editor.selected, 17);
        }
    }

    #[test]
    fn selection_wraps_by_sprite() {
        let mut editor = SpriteEditor::new();
        editor.set_zoom_index(3);

        editor.select(0);
        editor.move_selection(-1, 0);
        assert_eq!(editor.selected, 8);
        editor.move_selection(1, 0);
        assert_eq!(editor.selected, 0);
        editor.move_selection(0, -1);
        assert_eq!(editor.selected, 128);
    }

    #[test]
    fn selection_is_not_group_aligned() {
        let mut editor = SpriteEditor::new();
        editor.set_zoom_index(3);
        editor.select(3);

        assert_eq!(editor.selected, 3);
        editor.set_sprite_pixel(0, 0, 2);
        assert_eq!(editor.get_pixel(3, 0, 0), 2);
    }

    #[test]
    fn undo_restores_whole_sprite() {
        let mut editor = SpriteEditor::new();
        editor.set_zoom_index(1);
        editor.set_sprite_pixel(8, 8, 2);

        editor.push_undo();
        editor.set_sprite_pixel(8, 8, 3);
        editor.do_undo();

        assert_eq!(editor.get_sprite_pixel(8, 8), 2);
    }

    #[test]
    fn clear_removes_every_tile_of_sprite() {
        let mut editor = SpriteEditor::new();
        editor.set_zoom_index(2);
        editor.set_sprite_pixel(0, 0, 1);
        editor.set_sprite_pixel(31, 31, 2);

        editor.clear_sprite();

        assert!(editor
            .get_block(editor.selected, editor.tile_span())
            .data
            .iter()
            .all(|&b| b == 0));
    }
}
