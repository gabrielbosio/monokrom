use macroquad::prelude::*;

use crate::config::*;
use crate::input::{get_file_picker_action, EditorAction};
use crate::render::BitmapFont;
use crate::ui::dialog::DialogResult;

pub struct FilePicker {
    pub visible: bool,
    pub files: Vec<String>,
    pub selected_index: usize,
    pub scroll_offset: usize,
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
        }
    }

    pub fn show(&mut self, files: Vec<String>) {
        self.files = files;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn update(&mut self) -> Option<DialogResult> {
        if !self.visible {
            return None;
        }

        if self.files.is_empty() {
            // No files to select
            if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Enter) {
                self.hide();
                return Some(DialogResult::Cancel);
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
                    return Some(DialogResult::Confirm(filename));
                }
                EditorAction::DialogCancel => {
                    self.hide();
                    return Some(DialogResult::Cancel);
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
                    font.draw_char(
                        c,
                        list_x + (j as f32 * TILE_WIDTH as f32),
                        y,
                        COLOR_BLACK,
                    );
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
            let indicator = "^";
            let ind_x = picker_x + (picker_width as f32 - 2.0 * TILE_WIDTH as f32);
            let ind_y = list_y;
            font.draw_char_with_shadow(indicator.chars().next().unwrap(), ind_x, ind_y, COLOR_GRAY, COLOR_BLACK);
        }

        if visible_end < self.files.len() {
            let indicator = "v";
            let ind_x = picker_x + (picker_width as f32 - 2.0 * TILE_WIDTH as f32);
            let ind_y = list_y + ((Self::VISIBLE_ITEMS - 1) as f32 * TILE_HEIGHT as f32);
            font.draw_char_with_shadow(indicator.chars().next().unwrap(), ind_x, ind_y, COLOR_GRAY, COLOR_BLACK);
        }

        // Instructions
        let hint = "Enter:Select Esc:Cancel";
        let hint_x = picker_x + TILE_WIDTH as f32;
        let hint_y = picker_y + picker_height as f32 - (1.5 * TILE_HEIGHT as f32);
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
