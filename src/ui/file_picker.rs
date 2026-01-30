use macroquad::prelude::*;

use crate::config::*;
use crate::input::{get_file_picker_action, EditorAction};
use crate::render::BitmapFont;

/// Result from file picker actions
#[derive(Clone, Debug)]
pub enum FilePickerResult {
    /// User selected a file to open
    Open(String),
    /// User wants to duplicate a file (returns original filename)
    Duplicate(String),
    /// User confirmed deletion of a file
    Delete(String),
    /// User cancelled
    Cancel,
}

/// File picker mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilePickerMode {
    /// Normal file selection
    Select,
    /// Confirming file deletion
    ConfirmDelete,
}

pub struct FilePicker {
    pub visible: bool,
    pub files: Vec<String>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub mode: FilePickerMode,
}

impl Default for FilePicker {
    fn default() -> Self {
        Self::new()
    }
}

impl FilePicker {
    const VISIBLE_ITEMS: usize = 20;

    pub fn new() -> Self {
        Self {
            visible: false,
            files: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            mode: FilePickerMode::Select,
        }
    }

    pub fn show(&mut self, files: Vec<String>) {
        self.files = files;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.mode = FilePickerMode::Select;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.mode = FilePickerMode::Select;
    }

    pub fn update(&mut self) -> Option<FilePickerResult> {
        if !self.visible {
            return None;
        }

        // Handle delete confirmation mode
        if self.mode == FilePickerMode::ConfirmDelete {
            return self.update_confirm_delete();
        }

        if self.files.is_empty() {
            // No files to select
            if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Enter) {
                self.hide();
                return Some(FilePickerResult::Cancel);
            }
            return None;
        }

        if let Some(action) = get_file_picker_action() {
            match action {
                EditorAction::MoveUp => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                        // Adjust scroll if needed
                        if self.selected_index < self.scroll_offset {
                            self.scroll_offset = self.selected_index;
                        }
                    }
                }
                EditorAction::MoveDown => {
                    if self.selected_index < self.files.len() - 1 {
                        self.selected_index += 1;
                        // Adjust scroll if needed
                        if self.selected_index >= self.scroll_offset + Self::VISIBLE_ITEMS {
                            self.scroll_offset = self.selected_index - Self::VISIBLE_ITEMS + 1;
                        }
                    }
                }
                EditorAction::DialogConfirm => {
                    let filename = self.files[self.selected_index].clone();
                    self.hide();
                    return Some(FilePickerResult::Open(filename));
                }
                EditorAction::DialogCancel => {
                    self.hide();
                    return Some(FilePickerResult::Cancel);
                }
                EditorAction::DuplicateFile => {
                    let filename = self.files[self.selected_index].clone();
                    return Some(FilePickerResult::Duplicate(filename));
                }
                EditorAction::DeleteFile => {
                    // Enter delete confirmation mode
                    self.mode = FilePickerMode::ConfirmDelete;
                }
                _ => {}
            }
        }

        None
    }

    fn update_confirm_delete(&mut self) -> Option<FilePickerResult> {
        // Check for y/n input via key press (more reliable than get_char_pressed)
        if is_key_pressed(KeyCode::Y) {
            let filename = self.files[self.selected_index].clone();
            self.mode = FilePickerMode::Select;
            return Some(FilePickerResult::Delete(filename));
        }

        if is_key_pressed(KeyCode::N) || is_key_pressed(KeyCode::Escape) {
            self.mode = FilePickerMode::Select;
        }

        None
    }

    #[allow(dead_code)]
    pub fn draw(&self, font: &BitmapFont) {
        if !self.visible {
            return;
        }

        // Draw file picker background
        let picker_width = 30 * TILE_WIDTH;
        let picker_height = 24 * TILE_HEIGHT;
        let picker_x = ((SCREEN_WIDTH - picker_width) / 2) as f32;
        let picker_y = ((SCREEN_HEIGHT - picker_height) / 2) as f32;

        // Background
        draw_rectangle(
            picker_x,
            picker_y,
            picker_width as f32,
            picker_height as f32,
            COLOR_BLACK,
        );

        // Border
        draw_rectangle_lines(
            picker_x,
            picker_y,
            picker_width as f32,
            picker_height as f32,
            2.0,
            COLOR_WHITE,
        );

        // Title
        let title = "Open File";
        let title_x = picker_x + TILE_WIDTH as f32;
        let title_y = picker_y + TILE_HEIGHT as f32;
        for (i, c) in title.chars().enumerate() {
            font.draw_char_with_shadow(
                c,
                title_x + (i as f32 * TILE_WIDTH as f32),
                title_y,
                COLOR_WHITE,
                COLOR_GRAY,
            );
        }

        if self.files.is_empty() {
            // Show "No files" message
            let msg = "No files found";
            let msg_x = picker_x + TILE_WIDTH as f32;
            let msg_y = picker_y + (3 * TILE_HEIGHT) as f32;
            for (i, c) in msg.chars().enumerate() {
                font.draw_char_with_shadow(
                    c,
                    msg_x + (i as f32 * TILE_WIDTH as f32),
                    msg_y,
                    COLOR_GRAY,
                    COLOR_BLACK,
                );
            }
            return;
        }

        // File list
        let list_x = picker_x + TILE_WIDTH as f32;
        let list_y = picker_y + (3 * TILE_HEIGHT) as f32;
        let max_filename_len = ((picker_width - 2 * TILE_WIDTH) / TILE_WIDTH) as usize;

        let visible_end = (self.scroll_offset + Self::VISIBLE_ITEMS).min(self.files.len());

        for (i, file_idx) in (self.scroll_offset..visible_end).enumerate() {
            let file = &self.files[file_idx];
            let y = list_y + (i as f32 * TILE_HEIGHT as f32);

            let is_selected = file_idx == self.selected_index;

            // Truncate filename if too long
            let display_name: String = if file.len() > max_filename_len {
                format!("{}...", &file[..max_filename_len - 3])
            } else {
                file.clone()
            };

            if is_selected {
                // Draw selection background
                draw_rectangle(
                    list_x,
                    y,
                    (display_name.len() * TILE_WIDTH as usize) as f32,
                    TILE_HEIGHT as f32,
                    COLOR_WHITE,
                );

                // Draw selected text
                for (j, c) in display_name.chars().enumerate() {
                    font.draw_char(c, list_x + (j as f32 * TILE_WIDTH as f32), y, COLOR_BLACK);
                }
            } else {
                // Draw normal text
                for (j, c) in display_name.chars().enumerate() {
                    font.draw_char_with_shadow(
                        c,
                        list_x + (j as f32 * TILE_WIDTH as f32),
                        y,
                        COLOR_WHITE,
                        COLOR_BLACK,
                    );
                }
            }
        }

        // Scroll indicators
        if self.scroll_offset > 0 {
            let ind_x = picker_x + (picker_width as f32 - 2.0 * TILE_WIDTH as f32);
            let ind_y = list_y;
            font.draw_char_with_shadow('^', ind_x, ind_y, COLOR_GRAY, COLOR_BLACK);
        }

        if visible_end < self.files.len() {
            let ind_x = picker_x + (picker_width as f32 - 2.0 * TILE_WIDTH as f32);
            let ind_y = list_y + ((Self::VISIBLE_ITEMS - 1) as f32 * TILE_HEIGHT as f32);
            font.draw_char_with_shadow('v', ind_x, ind_y, COLOR_GRAY, COLOR_BLACK);
        }

        // Instructions or delete confirmation
        let hint_x = picker_x + TILE_WIDTH as f32;
        let hint_y = picker_y + picker_height as f32 - (1.5 * TILE_HEIGHT as f32);

        if self.mode == FilePickerMode::ConfirmDelete {
            // Show delete confirmation prompt
            let prompt = "Delete file? (y/n)";
            for (i, c) in prompt.chars().enumerate() {
                font.draw_char_with_shadow(
                    c,
                    hint_x + (i as f32 * TILE_WIDTH as f32),
                    hint_y,
                    COLOR_WHITE,
                    COLOR_BLACK,
                );
            }
        } else {
            // Show normal instructions on two lines
            let hint1 = "Enter:Open ^D:Copy ^Del:Del";
            let hint2 = "Esc:Cancel";
            let hint1_y = hint_y - TILE_HEIGHT as f32;

            for (i, c) in hint1.chars().enumerate() {
                font.draw_char_with_shadow(
                    c,
                    hint_x + (i as f32 * TILE_WIDTH as f32),
                    hint1_y,
                    COLOR_GRAY,
                    COLOR_BLACK,
                );
            }
            for (i, c) in hint2.chars().enumerate() {
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
}
