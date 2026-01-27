use macroquad::prelude::*;

use crate::config::*;
use crate::input::{get_dialog_action, EditorAction};
use crate::render::BitmapFont;

/// Dialog for prompting user input (filename)
pub struct InputDialog {
    pub title: String,
    pub input: String,
    pub visible: bool,
}

impl Default for InputDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl InputDialog {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            input: String::new(),
            visible: false,
        }
    }

    pub fn show(&mut self, title: &str) {
        self.title = title.to_string();
        self.input.clear();
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Process input and return result: Some(input) on confirm, None on cancel
    pub fn update(&mut self) -> Option<DialogResult> {
        if !self.visible {
            return None;
        }

        if let Some(action) = get_dialog_action() {
            match action {
                EditorAction::DialogConfirm => {
                    let result = self.input.clone();
                    self.hide();
                    return Some(DialogResult::Confirm(result));
                }
                EditorAction::DialogCancel => {
                    self.hide();
                    return Some(DialogResult::Cancel);
                }
                EditorAction::InsertChar(c) => {
                    self.input.push(c);
                }
                EditorAction::Backspace => {
                    self.input.pop();
                }
                _ => {}
            }
        }

        None
    }

    pub fn draw(&self, font: &BitmapFont) {
        if !self.visible {
            return;
        }

        // Draw dialog background
        let dialog_width = 28 * TILE_WIDTH;
        let dialog_height = 5 * TILE_HEIGHT;
        let dialog_x = ((SCREEN_WIDTH - dialog_width) / 2) as f32;
        let dialog_y = ((SCREEN_HEIGHT - dialog_height) / 2) as f32;

        // Background
        draw_rectangle(
            dialog_x,
            dialog_y,
            dialog_width as f32,
            dialog_height as f32,
            COLOR_BLACK,
        );

        // Border
        draw_rectangle_lines(
            dialog_x,
            dialog_y,
            dialog_width as f32,
            dialog_height as f32,
            2.0,
            COLOR_WHITE,
        );

        // Title
        let title_x = dialog_x + TILE_WIDTH as f32;
        let title_y = dialog_y + TILE_HEIGHT as f32;
        for (i, c) in self.title.chars().enumerate() {
            font.draw_char_with_shadow(
                c,
                title_x + (i as f32 * TILE_WIDTH as f32),
                title_y,
                COLOR_WHITE,
                COLOR_GRAY,
            );
        }

        // Input field background
        let input_x = dialog_x + TILE_WIDTH as f32;
        let input_y = dialog_y + (3 * TILE_HEIGHT) as f32;
        let input_width = (dialog_width - 2 * TILE_WIDTH) as f32;

        draw_rectangle(
            input_x,
            input_y,
            input_width,
            TILE_HEIGHT as f32,
            COLOR_GRAY,
        );

        // Input text (truncate if too long)
        let max_chars = ((input_width / TILE_WIDTH as f32) as usize).saturating_sub(1);
        let display_input: String = if self.input.len() > max_chars {
            self.input
                .chars()
                .skip(self.input.len() - max_chars)
                .collect()
        } else {
            self.input.clone()
        };

        for (i, c) in display_input.chars().enumerate() {
            font.draw_char(
                c,
                input_x + (i as f32 * TILE_WIDTH as f32),
                input_y,
                COLOR_WHITE,
            );
        }

        // Cursor
        let cursor_x = input_x + (display_input.len() as f32 * TILE_WIDTH as f32);
        draw_rectangle(
            cursor_x,
            input_y,
            TILE_WIDTH as f32,
            TILE_HEIGHT as f32,
            COLOR_WHITE,
        );
    }
}

/// Confirmation dialog (yes/no)
pub struct ConfirmDialog {
    pub message: String,
    pub visible: bool,
}

impl Default for ConfirmDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfirmDialog {
    pub fn new() -> Self {
        Self {
            message: String::new(),
            visible: false,
        }
    }

    pub fn show(&mut self, message: &str) {
        self.message = message.to_string();
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Process input and return result
    pub fn update(&mut self) -> Option<DialogResult> {
        if !self.visible {
            return None;
        }

        // Check for 'y' or 'n' key presses
        if let Some(c) = get_char_pressed() {
            match c.to_ascii_lowercase() {
                'y' => {
                    self.hide();
                    return Some(DialogResult::Confirm(String::new()));
                }
                'n' => {
                    self.hide();
                    return Some(DialogResult::Cancel);
                }
                _ => {}
            }
        }

        if is_key_pressed(KeyCode::Escape) {
            self.hide();
            return Some(DialogResult::Cancel);
        }

        None
    }

    pub fn draw(&self, font: &BitmapFont) {
        if !self.visible {
            return;
        }

        // Draw dialog background
        let dialog_width = 28 * TILE_WIDTH;
        let dialog_height = 4 * TILE_HEIGHT;
        let dialog_x = ((SCREEN_WIDTH - dialog_width) / 2) as f32;
        let dialog_y = ((SCREEN_HEIGHT - dialog_height) / 2) as f32;

        // Background
        draw_rectangle(
            dialog_x,
            dialog_y,
            dialog_width as f32,
            dialog_height as f32,
            COLOR_BLACK,
        );

        // Border
        draw_rectangle_lines(
            dialog_x,
            dialog_y,
            dialog_width as f32,
            dialog_height as f32,
            2.0,
            COLOR_WHITE,
        );

        // Message
        let msg_x = dialog_x + TILE_WIDTH as f32;
        let msg_y = dialog_y + TILE_HEIGHT as f32;
        for (i, c) in self.message.chars().enumerate() {
            font.draw_char_with_shadow(
                c,
                msg_x + (i as f32 * TILE_WIDTH as f32),
                msg_y,
                COLOR_WHITE,
                COLOR_GRAY,
            );
        }

        // Instructions
        let hint = "(y/n)";
        let hint_x = dialog_x + TILE_WIDTH as f32;
        let hint_y = dialog_y + (2.5 * TILE_HEIGHT as f32);
        for (i, c) in hint.chars().enumerate() {
            font.draw_char_with_shadow(
                c,
                hint_x + (i as f32 * TILE_WIDTH as f32),
                hint_y,
                COLOR_GRAY,
                COLOR_BLACK,
            );
        }
    }
}

#[derive(Debug, Clone)]
pub enum DialogResult {
    Confirm(String),
    Cancel,
}
