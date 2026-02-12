use macroquad::prelude::*;

use crate::config::{
    COLOR_BLACK, COLOR_GRAY, COLOR_WHITE, SCREEN_HEIGHT, SCREEN_WIDTH, TILE_HEIGHT, TILE_WIDTH,
};
use crate::input::{get_dialog_action, EditorAction};
use crate::render::DrawHelpers;

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

    pub fn draw_scaled(&self, helpers: &DrawHelpers) {
        if !self.visible {
            return;
        }

        let dialog_width = (28 * TILE_WIDTH) as f32;
        let dialog_height = (5 * TILE_HEIGHT) as f32;
        let dialog_x = (SCREEN_WIDTH as f32 - dialog_width) / 2.0;
        let dialog_y = (SCREEN_HEIGHT as f32 - dialog_height) / 2.0;

        helpers.draw_dialog_frame(dialog_x, dialog_y, dialog_width, dialog_height);

        // Title
        let title_x = dialog_x + TILE_WIDTH as f32;
        let title_y = dialog_y + TILE_HEIGHT as f32;
        helpers.draw_text_with_shadow(&self.title, title_x, title_y, COLOR_WHITE, COLOR_GRAY);

        // Input field background
        let input_x = dialog_x + TILE_WIDTH as f32;
        let input_y = dialog_y + (3 * TILE_HEIGHT) as f32;
        let input_width = dialog_width - (2 * TILE_WIDTH) as f32;
        helpers.draw_rect(
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

        helpers.draw_text(&display_input, input_x, input_y, COLOR_WHITE);

        // Cursor
        let cursor_x = input_x + (display_input.len() as f32 * TILE_WIDTH as f32);
        helpers.draw_rect(
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
                    return Some(DialogResult::Reject);
                }
                _ => {}
            }
        }

        // Escape cancels the prompt (goes back to editing)
        if is_key_pressed(KeyCode::Escape) {
            self.hide();
            return Some(DialogResult::Cancel);
        }

        None
    }

    pub fn draw_scaled(&self, helpers: &DrawHelpers) {
        if !self.visible {
            return;
        }

        let dialog_width = (28 * TILE_WIDTH) as f32;
        let dialog_height = (4 * TILE_HEIGHT) as f32;
        let dialog_x = (SCREEN_WIDTH as f32 - dialog_width) / 2.0;
        let dialog_y = (SCREEN_HEIGHT as f32 - dialog_height) / 2.0;

        helpers.draw_dialog_frame(dialog_x, dialog_y, dialog_width, dialog_height);

        // Message
        let msg_x = dialog_x + TILE_WIDTH as f32;
        let msg_y = dialog_y + TILE_HEIGHT as f32;
        helpers.draw_text_with_shadow(&self.message, msg_x, msg_y, COLOR_WHITE, COLOR_GRAY);

        // Instructions
        let hint_x = dialog_x + TILE_WIDTH as f32;
        let hint_y = dialog_y + (2.5 * TILE_HEIGHT as f32);
        helpers.draw_text_with_shadow("(y/n)", hint_x, hint_y, COLOR_GRAY, COLOR_BLACK);
    }
}

#[derive(Debug, Clone)]
pub enum DialogResult {
    Confirm(String),
    Reject,
    Cancel,
}

/// Message dialog for displaying info/error messages (dismiss with any key)
pub struct MessageDialog {
    pub message: String,
    pub visible: bool,
}

impl Default for MessageDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageDialog {
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

    /// Process input - dismiss on any key press
    pub fn update(&mut self) -> bool {
        if !self.visible {
            return false;
        }

        // Dismiss on any key press
        if is_key_pressed(KeyCode::Escape)
            || is_key_pressed(KeyCode::Enter)
            || is_key_pressed(KeyCode::Space)
            || get_char_pressed().is_some()
        {
            self.hide();
            return true;
        }

        false
    }

    pub fn draw_scaled(&self, helpers: &DrawHelpers) {
        if !self.visible {
            return;
        }

        let dialog_width = (28 * TILE_WIDTH) as f32;
        let dialog_height = (4 * TILE_HEIGHT) as f32;
        let dialog_x = (SCREEN_WIDTH as f32 - dialog_width) / 2.0;
        let dialog_y = (SCREEN_HEIGHT as f32 - dialog_height) / 2.0;

        helpers.draw_dialog_frame(dialog_x, dialog_y, dialog_width, dialog_height);

        // Message
        let msg_x = dialog_x + TILE_WIDTH as f32;
        let msg_y = dialog_y + TILE_HEIGHT as f32;
        helpers.draw_text_with_shadow(&self.message, msg_x, msg_y, COLOR_WHITE, COLOR_GRAY);

        // Instructions
        let hint_x = dialog_x + TILE_WIDTH as f32;
        let hint_y = dialog_y + (2.5 * TILE_HEIGHT as f32);
        helpers.draw_text_with_shadow("(press any key)", hint_x, hint_y, COLOR_GRAY, COLOR_BLACK);
    }
}
