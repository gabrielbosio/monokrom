use macroquad::prelude::*;

use crate::config::{
    COLOR_BLACK, COLOR_DARK_GRAY, COLOR_WHITE, SCREEN_HEIGHT, SCREEN_WIDTH, TILE_HEIGHT, TILE_WIDTH,
};
use crate::input::{get_file_picker_action, EditorAction};
use crate::render::DrawHelpers;

/// Result from file picker actions
#[derive(Clone, Debug)]
pub enum FilePickerResult {
    /// User selected a file to open
    Open(String),
    /// User wants to duplicate a file (returns original filename)
    Duplicate(String),
    /// User confirmed deletion of a file
    Delete(String),
    /// User wants to export (download) a file
    #[cfg(target_arch = "wasm32")]
    Export(String),
    /// User wants to import (upload) a file
    #[cfg(target_arch = "wasm32")]
    Import,
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
                EditorAction::MoveUp if self.selected_index > 0 => {
                    self.selected_index -= 1;
                    // Adjust scroll if needed
                    if self.selected_index < self.scroll_offset {
                        self.scroll_offset = self.selected_index;
                    }
                }
                EditorAction::MoveDown if self.selected_index < self.files.len() - 1 => {
                    self.selected_index += 1;
                    // Adjust scroll if needed
                    if self.selected_index >= self.scroll_offset + Self::VISIBLE_ITEMS {
                        self.scroll_offset = self.selected_index - Self::VISIBLE_ITEMS + 1;
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
                #[cfg(target_arch = "wasm32")]
                EditorAction::Export => {
                    let filename = self.files[self.selected_index].clone();
                    return Some(FilePickerResult::Export(filename));
                }
                #[cfg(target_arch = "wasm32")]
                EditorAction::Import => {
                    return Some(FilePickerResult::Import);
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

    pub fn draw_scaled(&self, helpers: &DrawHelpers) {
        if !self.visible {
            return;
        }

        let picker_width = (30 * TILE_WIDTH) as f32;
        let picker_height = (24 * TILE_HEIGHT) as f32;
        let picker_x = (SCREEN_WIDTH as f32 - picker_width) / 2.0;
        let picker_y = (SCREEN_HEIGHT as f32 - picker_height) / 2.0;

        // Background
        helpers.draw_rect(picker_x, picker_y, picker_width, picker_height, COLOR_BLACK);

        // Border
        helpers.draw_rect_lines(
            picker_x,
            picker_y,
            picker_width,
            picker_height,
            2.0,
            COLOR_WHITE,
        );

        // Title
        let title_x = picker_x + TILE_WIDTH as f32;
        let title_y = picker_y + TILE_HEIGHT as f32;
        helpers.draw_text_with_shadow("Open File", title_x, title_y, COLOR_WHITE, COLOR_DARK_GRAY);

        if self.files.is_empty() {
            let msg_x = picker_x + TILE_WIDTH as f32;
            let msg_y = picker_y + (3 * TILE_HEIGHT) as f32;
            helpers.draw_text_with_shadow(
                "No files found",
                msg_x,
                msg_y,
                COLOR_DARK_GRAY,
                COLOR_BLACK,
            );
            return;
        }

        // File list
        let list_x = picker_x + TILE_WIDTH as f32;
        let list_y = picker_y + (3 * TILE_HEIGHT) as f32;
        let max_filename_len =
            ((picker_width - (2 * TILE_WIDTH) as f32) / TILE_WIDTH as f32) as usize;
        let visible_end = (self.scroll_offset + Self::VISIBLE_ITEMS).min(self.files.len());

        for (i, file_idx) in (self.scroll_offset..visible_end).enumerate() {
            let file = &self.files[file_idx];
            let y = list_y + (i as f32 * TILE_HEIGHT as f32);
            let is_selected = file_idx == self.selected_index;

            let display_name: String = if file.len() > max_filename_len {
                format!("{}...", &file[..max_filename_len - 3])
            } else {
                file.clone()
            };

            if is_selected {
                helpers.draw_rect(
                    list_x,
                    y,
                    (display_name.len() * TILE_WIDTH as usize) as f32,
                    TILE_HEIGHT as f32,
                    COLOR_WHITE,
                );
                helpers.draw_text(&display_name, list_x, y, COLOR_BLACK);
            } else {
                helpers.draw_text_with_shadow(&display_name, list_x, y, COLOR_WHITE, COLOR_BLACK);
            }
        }

        // Scroll indicators
        if self.scroll_offset > 0 {
            let ind_x = picker_x + picker_width - 2.0 * TILE_WIDTH as f32;
            let ind_y = list_y;
            helpers.draw_char_with_shadow('^', ind_x, ind_y, COLOR_DARK_GRAY, COLOR_BLACK);
        }

        if visible_end < self.files.len() {
            let ind_x = picker_x + picker_width - 2.0 * TILE_WIDTH as f32;
            let ind_y = list_y + ((Self::VISIBLE_ITEMS - 1) as f32 * TILE_HEIGHT as f32);
            helpers.draw_char_with_shadow('v', ind_x, ind_y, COLOR_DARK_GRAY, COLOR_BLACK);
        }

        // Instructions or delete confirmation
        let hint_x = picker_x + TILE_WIDTH as f32;
        let hint_y = picker_y + picker_height - (1.5 * TILE_HEIGHT as f32);

        if self.mode == FilePickerMode::ConfirmDelete {
            helpers.draw_text_with_shadow(
                "Delete file? (y/n)",
                hint_x,
                hint_y,
                COLOR_WHITE,
                COLOR_BLACK,
            );
        } else {
            let hint1_y = hint_y - TILE_HEIGHT as f32;
            #[cfg(not(target_arch = "wasm32"))]
            helpers.draw_text_with_shadow(
                "Enter:Open ^D:Copy ^Del:Del",
                hint_x,
                hint1_y,
                COLOR_DARK_GRAY,
                COLOR_BLACK,
            );
            #[cfg(target_arch = "wasm32")]
            helpers.draw_text_with_shadow(
                "Enter:Open ^D:Copy ^Del:Del",
                hint_x,
                hint1_y,
                COLOR_DARK_GRAY,
                COLOR_BLACK,
            );
            #[cfg(not(target_arch = "wasm32"))]
            helpers.draw_text_with_shadow(
                "Esc:Cancel",
                hint_x,
                hint_y,
                COLOR_DARK_GRAY,
                COLOR_BLACK,
            );
            #[cfg(target_arch = "wasm32")]
            helpers.draw_text_with_shadow(
                "^E:Exp ^I:Imp Esc:Cancel",
                hint_x,
                hint_y,
                COLOR_DARK_GRAY,
                COLOR_BLACK,
            );
        }
    }
}
