use macroquad::prelude::*;

use super::font::BitmapFont;
use crate::config::*;

/// Helper struct for drawing UI elements with scaling
pub struct DrawHelpers<'a> {
    pub font: &'a BitmapFont,
    pub scale: f32,
}

impl<'a> DrawHelpers<'a> {
    pub fn new(font: &'a BitmapFont, scale: f32) -> Self {
        Self { font, scale }
    }

    /// Draw a filled rectangle with scaling applied
    pub fn draw_rect(&self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        draw_rectangle(
            x * self.scale,
            y * self.scale,
            w * self.scale,
            h * self.scale,
            color,
        );
    }

    /// Draw a rectangle border with scaling applied
    pub fn draw_rect_lines(&self, x: f32, y: f32, w: f32, h: f32, thickness: f32, color: Color) {
        draw_rectangle_lines(
            x * self.scale,
            y * self.scale,
            w * self.scale,
            h * self.scale,
            thickness,
            color,
        );
    }

    /// Draw a character with scaling applied
    pub fn draw_char(&self, c: char, x: f32, y: f32, color: Color) {
        self.font
            .draw_char_scaled(c, x * self.scale, y * self.scale, self.scale, color);
    }

    /// Draw a character with shadow with scaling applied
    pub fn draw_char_with_shadow(&self, c: char, x: f32, y: f32, fg: Color, shadow: Color) {
        self.font.draw_char_with_shadow_scaled(
            c,
            x * self.scale,
            y * self.scale,
            self.scale,
            fg,
            shadow,
        );
    }

    /// Draw a string of text with shadow
    pub fn draw_text_with_shadow(&self, text: &str, x: f32, y: f32, fg: Color, shadow: Color) {
        for (i, c) in text.chars().enumerate() {
            self.draw_char_with_shadow(c, x + (i as f32 * TILE_WIDTH as f32), y, fg, shadow);
        }
    }

    /// Draw a string of text without shadow
    pub fn draw_text(&self, text: &str, x: f32, y: f32, color: Color) {
        for (i, c) in text.chars().enumerate() {
            self.draw_char(c, x + (i as f32 * TILE_WIDTH as f32), y, color);
        }
    }
}
