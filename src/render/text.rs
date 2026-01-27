#![allow(dead_code)]

use macroquad::prelude::*;

use crate::config::*;
use crate::render::font::BitmapFont;

pub struct TextRenderer<'a> {
    font: &'a BitmapFont,
}

impl<'a> TextRenderer<'a> {
    pub fn new(font: &'a BitmapFont) -> Self {
        Self { font }
    }

    /// Draw text at tile coordinates with shadow
    pub fn draw_text(&self, text: &str, tile_x: u32, tile_y: u32) {
        self.draw_text_colored(text, tile_x, tile_y, COLOR_WHITE, COLOR_BLACK);
    }

    /// Draw text at tile coordinates with custom colors
    pub fn draw_text_colored(
        &self,
        text: &str,
        tile_x: u32,
        tile_y: u32,
        fg_color: Color,
        shadow_color: Color,
    ) {
        let start_x = (tile_x * TILE_WIDTH) as f32;
        let y = (tile_y * TILE_HEIGHT) as f32;

        for (i, c) in text.chars().enumerate() {
            let x = start_x + (i as f32 * TILE_WIDTH as f32);
            self.font
                .draw_char_with_shadow(c, x, y, fg_color, shadow_color);
        }
    }

    /// Draw text at pixel coordinates with shadow
    pub fn draw_text_px(&self, text: &str, x: f32, y: f32) {
        self.draw_text_px_colored(text, x, y, COLOR_WHITE, COLOR_BLACK);
    }

    /// Draw text at pixel coordinates with custom colors
    pub fn draw_text_px_colored(
        &self,
        text: &str,
        x: f32,
        y: f32,
        fg_color: Color,
        shadow_color: Color,
    ) {
        for (i, c) in text.chars().enumerate() {
            let char_x = x + (i as f32 * TILE_WIDTH as f32);
            self.font
                .draw_char_with_shadow(c, char_x, y, fg_color, shadow_color);
        }
    }

    /// Draw a single character at tile coordinates
    pub fn draw_char(
        &self,
        c: char,
        tile_x: u32,
        tile_y: u32,
        fg_color: Color,
        shadow_color: Color,
    ) {
        let x = (tile_x * TILE_WIDTH) as f32;
        let y = (tile_y * TILE_HEIGHT) as f32;
        self.font
            .draw_char_with_shadow(c, x, y, fg_color, shadow_color);
    }

    /// Draw a single character at pixel coordinates
    pub fn draw_char_px(&self, c: char, x: f32, y: f32, fg_color: Color, shadow_color: Color) {
        self.font
            .draw_char_with_shadow(c, x, y, fg_color, shadow_color);
    }

    /// Draw text with selection highlighting
    pub fn draw_text_with_selection(
        &self,
        text: &str,
        tile_x: u32,
        tile_y: u32,
        selection_start: Option<usize>,
        selection_end: Option<usize>,
    ) {
        let y = (tile_y * TILE_HEIGHT) as f32;

        for (i, c) in text.chars().enumerate() {
            let x = (tile_x * TILE_WIDTH) as f32 + (i as f32 * TILE_WIDTH as f32);

            let is_selected = match (selection_start, selection_end) {
                (Some(start), Some(end)) => i >= start && i < end,
                _ => false,
            };

            if is_selected {
                // Draw selection background
                draw_rectangle(
                    x,
                    y,
                    TILE_WIDTH as f32,
                    TILE_HEIGHT as f32,
                    COLOR_SELECTION_BG,
                );
                // Draw text in inverted colors (no shadow on selection)
                self.font.draw_char(c, x, y, COLOR_SELECTION_FG);
            } else {
                self.font
                    .draw_char_with_shadow(c, x, y, COLOR_WHITE, COLOR_BLACK);
            }
        }
    }
}
