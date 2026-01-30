use macroquad::prelude::*;

use crate::config::*;
use crate::editor::{operations, Cursor, CursorPosition, History, Selection, TextBuffer};
use crate::filesystem;
use crate::input::{get_editor_action, EditorAction};
use crate::render::{BitmapFont, ScrollbarState};
use crate::ui::{
    ConfirmDialog, DialogResult, FilePicker, FilePickerMode, FilePickerResult, InputDialog,
    MessageDialog,
};

/// Application state modes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMode {
    /// Normal editing mode
    Editing,
    /// Save dialog open
    SaveDialog,
    /// Open file picker
    OpenPicker,
    /// Close confirmation dialog
    CloseConfirm,
    /// Find dialog open
    FindDialog,
    /// Replace dialog open
    ReplaceDialog,
    /// Message dialog (error/info)
    Message,
}

pub struct App {
    // Editor state
    pub buffer: TextBuffer,
    pub cursor: Cursor,
    pub selection: Selection,
    pub history: History,

    // Viewport
    pub scroll_x: usize,
    pub scroll_y: usize,
    pub scrollbar_state: ScrollbarState,

    // File state
    pub current_filename: Option<String>,
    pub is_modified: bool,

    // UI state
    pub mode: AppMode,
    pub input_dialog: InputDialog,
    pub confirm_dialog: ConfirmDialog,
    pub message_dialog: MessageDialog,
    pub file_picker: FilePicker,
    pub pending_new_file: bool, // Create new file after save dialog completes
    pub pending_open_file: bool, // Open file picker after save dialog completes

    // Search state
    pub search_query: String,
    pub replace_text: String,
    pub current_match_pos: Option<usize>, // Character index of current match
    pub is_replacing: bool,               // True if in replace mode (vs find mode)

    // Clipboard
    pub clipboard: Option<arboard::Clipboard>,

    // Cursor blink
    pub cursor_blink_timer: f64,
    pub cursor_visible: bool,

    // Rendering
    pub font: BitmapFont,
}

impl App {
    pub async fn new() -> Self {
        let font = BitmapFont::new().await;
        let clipboard = arboard::Clipboard::new().ok();

        Self {
            buffer: TextBuffer::new(),
            cursor: Cursor::new(),
            selection: Selection::new(),
            history: History::new(),

            scroll_x: 0,
            scroll_y: 0,
            scrollbar_state: ScrollbarState::default(),

            current_filename: None,
            is_modified: false,

            mode: AppMode::Editing,
            input_dialog: InputDialog::new(),
            confirm_dialog: ConfirmDialog::new(),
            message_dialog: MessageDialog::new(),
            file_picker: FilePicker::new(),
            pending_new_file: false,
            pending_open_file: false,

            search_query: String::new(),
            replace_text: String::new(),
            current_match_pos: None,
            is_replacing: false,

            clipboard,

            cursor_blink_timer: 0.0,
            cursor_visible: true,

            font,
        }
    }

    fn is_in_search_mode(&self) -> bool {
        !self.search_query.is_empty()
    }

    pub fn update(&mut self) {
        // Update cursor blink
        self.cursor_blink_timer += get_frame_time() as f64;
        if self.cursor_blink_timer >= CURSOR_BLINK_RATE {
            self.cursor_blink_timer = 0.0;
            self.cursor_visible = !self.cursor_visible;
        }

        match self.mode {
            AppMode::Editing => self.update_editing(),
            AppMode::SaveDialog => self.update_save_dialog(),
            AppMode::OpenPicker => self.update_open_picker(),
            AppMode::CloseConfirm => self.update_close_confirm(),
            AppMode::FindDialog => self.update_find_dialog(),
            AppMode::ReplaceDialog => self.update_replace_dialog(),
            AppMode::Message => self.update_message_dialog(),
        }

        // Update scrollbar state
        let visible_cols = self.visible_cols();
        let visible_lines = self.visible_lines();
        self.scrollbar_state.update(
            self.buffer.line_count().max(1),
            visible_lines,
            self.scroll_y,
            self.buffer.max_line_width(),
            visible_cols,
            self.scroll_x,
        );
    }

    fn visible_cols(&self) -> usize {
        (if self.scrollbar_state.vertical_visible {
            EDITOR_TILES_X
        } else {
            SCREEN_TILES_X
        }) as usize
    }

    fn visible_lines(&self) -> usize {
        (if self.scrollbar_state.horizontal_visible {
            EDITOR_TILES_Y
        } else {
            SCREEN_TILES_Y
        }) as usize
    }

    fn update_editing(&mut self) {
        if let Some(action) = get_editor_action() {
            // Reset cursor blink on any action
            self.cursor_visible = true;
            self.cursor_blink_timer = 0.0;

            match action {
                // Text input
                EditorAction::InsertChar(c) => {
                    if self.is_in_search_mode() {
                        return;
                    }
                    operations::insert_char(
                        &mut self.buffer,
                        &mut self.cursor,
                        &mut self.selection,
                        &mut self.history,
                        c,
                    );
                    self.is_modified = true;
                    self.ensure_cursor_visible();
                }
                EditorAction::InsertNewline => {
                    // If in replace mode with an active match, replace and find next
                    if self.is_replacing && self.current_match_pos.is_some() {
                        self.replace_and_find_next();
                        return;
                    }
                    // If in find mode (not replace) with active search, find next
                    if self.is_in_search_mode() {
                        self.find_next();
                        return;
                    }
                    operations::insert_char(
                        &mut self.buffer,
                        &mut self.cursor,
                        &mut self.selection,
                        &mut self.history,
                        '\n',
                    );
                    self.is_modified = true;
                    self.ensure_cursor_visible();
                }

                // Cursor movement
                EditorAction::MoveLeft => {
                    self.cursor.move_left(&self.buffer);
                    self.selection.clear_and_sync(self.cursor.position);
                    self.ensure_cursor_visible();
                }
                EditorAction::MoveRight => {
                    self.cursor.move_right(&self.buffer);
                    self.selection.clear_and_sync(self.cursor.position);
                    self.ensure_cursor_visible();
                }
                EditorAction::MoveUp => {
                    self.cursor.move_up(&self.buffer);
                    self.selection.clear_and_sync(self.cursor.position);
                    self.ensure_cursor_visible();
                }
                EditorAction::MoveDown => {
                    self.cursor.move_down(&self.buffer);
                    self.selection.clear_and_sync(self.cursor.position);
                    self.ensure_cursor_visible();
                }
                EditorAction::MoveWordLeft => {
                    self.cursor.move_word_left(&self.buffer);
                    self.selection.clear_and_sync(self.cursor.position);
                    self.ensure_cursor_visible();
                }
                EditorAction::MoveWordRight => {
                    self.cursor.move_word_right(&self.buffer);
                    self.selection.clear_and_sync(self.cursor.position);
                    self.ensure_cursor_visible();
                }
                EditorAction::MoveToLineStart => {
                    self.cursor.move_to_line_start();
                    self.selection.clear_and_sync(self.cursor.position);
                    self.ensure_cursor_visible();
                }
                EditorAction::MoveToLineEnd => {
                    self.cursor.move_to_line_end(&self.buffer);
                    self.selection.clear_and_sync(self.cursor.position);
                    self.ensure_cursor_visible();
                }

                // Selection
                EditorAction::SelectLeft => {
                    // Set anchor at current position if not already selecting
                    if self.selection.anchor.is_none() {
                        self.selection.anchor = Some(self.cursor.position);
                    }
                    self.cursor.move_left(&self.buffer);
                    self.selection.cursor = self.cursor.position;
                    self.ensure_cursor_visible();
                }
                EditorAction::SelectRight => {
                    if self.selection.anchor.is_none() {
                        self.selection.anchor = Some(self.cursor.position);
                    }
                    self.cursor.move_right(&self.buffer);
                    self.selection.cursor = self.cursor.position;
                    self.ensure_cursor_visible();
                }
                EditorAction::SelectUp => {
                    if self.selection.anchor.is_none() {
                        self.selection.anchor = Some(self.cursor.position);
                    }
                    self.cursor.move_up(&self.buffer);
                    self.selection.cursor = self.cursor.position;
                    self.ensure_cursor_visible();
                }
                EditorAction::SelectDown => {
                    if self.selection.anchor.is_none() {
                        self.selection.anchor = Some(self.cursor.position);
                    }
                    self.cursor.move_down(&self.buffer);
                    self.selection.cursor = self.cursor.position;
                    self.ensure_cursor_visible();
                }
                EditorAction::SelectAll => {
                    self.selection.anchor = Some(CursorPosition::new(0, 0));
                    let last_line = self.buffer.line_count().saturating_sub(1);
                    let last_col = self.buffer.line_len(last_line);
                    self.cursor.set_position(last_line, last_col);
                    self.selection.cursor = self.cursor.position;
                }

                // Editing
                EditorAction::Backspace => {
                    if self.is_in_search_mode() {
                        return;
                    }
                    operations::delete_before(
                        &mut self.buffer,
                        &mut self.cursor,
                        &mut self.selection,
                        &mut self.history,
                    );
                    self.is_modified = true;
                    self.ensure_cursor_visible();
                }
                EditorAction::Delete => {
                    if self.is_in_search_mode() {
                        return;
                    }
                    operations::delete_at(
                        &mut self.buffer,
                        &mut self.cursor,
                        &mut self.selection,
                        &mut self.history,
                    );
                    self.is_modified = true;
                }
                EditorAction::SwapLineUp => {
                    if self.is_in_search_mode() {
                        return;
                    }
                    operations::swap_line_up(
                        &mut self.buffer,
                        &mut self.cursor,
                        &mut self.selection,
                        &mut self.history,
                    );
                    self.is_modified = true;
                    self.ensure_cursor_visible();
                }
                EditorAction::SwapLineDown => {
                    if self.is_in_search_mode() {
                        return;
                    }
                    operations::swap_line_down(
                        &mut self.buffer,
                        &mut self.cursor,
                        &mut self.selection,
                        &mut self.history,
                    );
                    self.is_modified = true;
                    self.ensure_cursor_visible();
                }

                // Clipboard
                EditorAction::Cut => {
                    if self.is_in_search_mode() {
                        return;
                    }
                    let (text, _clipboard_ok) = operations::cut_selection(
                        &mut self.buffer,
                        &mut self.cursor,
                        &mut self.selection,
                        &mut self.history,
                        &mut self.clipboard,
                    );
                    if text.is_some() {
                        self.is_modified = true;
                    }
                }
                EditorAction::Copy => {
                    let (_text, _clipboard_ok) = operations::copy_selection(
                        &self.buffer,
                        &self.selection,
                        &mut self.clipboard,
                    );
                }
                EditorAction::Paste => {
                    if self.is_in_search_mode() {
                        return;
                    }
                    operations::paste(
                        &mut self.buffer,
                        &mut self.cursor,
                        &mut self.selection,
                        &mut self.history,
                        &mut self.clipboard,
                    );
                    self.is_modified = true;
                    self.ensure_cursor_visible();
                }

                // History
                EditorAction::Undo => {
                    if self.is_in_search_mode() {
                        return;
                    }
                    operations::undo(
                        &mut self.buffer,
                        &mut self.cursor,
                        &mut self.selection,
                        &mut self.history,
                    );
                    self.ensure_cursor_visible();
                }
                EditorAction::Redo => {
                    if self.is_in_search_mode() {
                        return;
                    }
                    operations::redo(
                        &mut self.buffer,
                        &mut self.cursor,
                        &mut self.selection,
                        &mut self.history,
                    );
                    self.ensure_cursor_visible();
                }

                // Scrolling
                EditorAction::ScrollUp => {
                    if self.scroll_y > 0 {
                        self.scroll_y -= 1;
                    }
                }
                EditorAction::ScrollDown => {
                    let max_scroll = self
                        .buffer
                        .line_count()
                        .saturating_sub(self.visible_lines());
                    if self.scroll_y < max_scroll {
                        self.scroll_y += 1;
                    }
                }
                EditorAction::ScrollLeft => {
                    if self.scroll_x > 0 {
                        self.scroll_x -= 1;
                    }
                }
                EditorAction::ScrollRight => {
                    let max_scroll = self
                        .buffer
                        .max_line_width()
                        .saturating_sub(self.visible_cols());
                    if self.scroll_x < max_scroll {
                        self.scroll_x += 1;
                    }
                }

                // File operations
                EditorAction::Save => {
                    if self.current_filename.is_some() {
                        self.save_current_file();
                    } else {
                        self.mode = AppMode::SaveDialog;
                        self.input_dialog.show("Save as:");
                    }
                }
                EditorAction::Open => {
                    if self.is_modified {
                        self.pending_open_file = true;
                        self.mode = AppMode::CloseConfirm;
                        self.confirm_dialog.show("Save changes?");
                    } else {
                        self.mode = AppMode::OpenPicker;
                        let files = filesystem::list_files().unwrap_or_default();
                        self.file_picker.show(files);
                    }
                }
                EditorAction::New => {
                    if self.is_modified {
                        self.mode = AppMode::CloseConfirm;
                        self.confirm_dialog.show("Save changes?");
                    } else {
                        self.create_new_file();
                    }
                }

                EditorAction::Find => {
                    self.is_replacing = false;
                    self.mode = AppMode::FindDialog;
                    self.input_dialog.show("Find:");
                }
                EditorAction::Replace => {
                    self.is_replacing = true;
                    self.mode = AppMode::FindDialog;
                    self.input_dialog.show("Find:");
                }
                EditorAction::FindNext => {
                    self.find_next();
                }

                EditorAction::DialogCancel => {
                    // Escape pressed - clear search mode if active
                    if !self.search_query.is_empty() {
                        self.search_query.clear();
                        self.replace_text.clear();
                        self.current_match_pos = None;
                        self.is_replacing = false;
                        self.selection.clear();
                    }
                }

                _ => {}
            }
        }
    }

    fn update_save_dialog(&mut self) {
        if let Some(result) = self.input_dialog.update() {
            match result {
                DialogResult::Confirm(filename) => {
                    if filename.is_empty() {
                        self.show_message("Error: Filename cannot be empty");
                        return;
                    }
                    if !filesystem::is_valid_filename(&filename) {
                        self.show_message("Error: Invalid filename");
                        return;
                    }
                    self.current_filename = Some(filename);
                    if self.save_current_file() {
                        // Check if we have a pending action after saving
                        if self.pending_new_file || self.pending_open_file {
                            self.after_close_confirm_action();
                            return;
                        }
                    } else {
                        // Save failed, error message already shown
                        return;
                    }
                }
                DialogResult::Reject | DialogResult::Cancel => {
                    self.pending_new_file = false;
                    self.pending_open_file = false;
                }
            }
            self.mode = AppMode::Editing;
        }
    }

    fn update_open_picker(&mut self) {
        if let Some(result) = self.file_picker.update() {
            match result {
                FilePickerResult::Open(filename) => {
                    self.open_file(&filename);
                    self.mode = AppMode::Editing;
                }
                FilePickerResult::Duplicate(filename) => {
                    // Duplicate the file and refresh the picker
                    if let Ok(new_name) = filesystem::duplicate_file(&filename) {
                        // Refresh file list
                        if let Ok(files) = filesystem::list_files() {
                            // Find the index of the new file
                            let new_index = files.iter().position(|f| f == &new_name).unwrap_or(0);
                            self.file_picker.show(files);
                            self.file_picker.selected_index = new_index;
                        }
                    }
                }
                FilePickerResult::Delete(filename) => {
                    // Delete the file and refresh the picker
                    if filesystem::delete_file(&filename).is_ok() {
                        // Refresh file list
                        if let Ok(files) = filesystem::list_files() {
                            let old_index = self.file_picker.selected_index;
                            self.file_picker.show(files);
                            // Adjust selected index if needed
                            if !self.file_picker.files.is_empty() {
                                self.file_picker.selected_index =
                                    old_index.min(self.file_picker.files.len() - 1);
                            }
                        }
                    }
                }
                FilePickerResult::Cancel => {
                    self.mode = AppMode::Editing;
                }
            }
        }
    }

    fn update_close_confirm(&mut self) {
        if let Some(result) = self.confirm_dialog.update() {
            match result {
                DialogResult::Confirm(_) => {
                    // User wants to save ('y')
                    if self.current_filename.is_some() {
                        self.save_current_file();
                        self.after_close_confirm_action();
                        return;
                    } else {
                        // Need to ask for filename first
                        self.mode = AppMode::SaveDialog;
                        self.input_dialog.show("Save as:");
                        return; // Don't change mode yet, save dialog will handle it
                    }
                }
                DialogResult::Reject => {
                    // User doesn't want to save ('n')
                    self.after_close_confirm_action();
                    return;
                }
                DialogResult::Cancel => {
                    // User pressed Escape - cancel the prompt, go back to editing
                    self.pending_new_file = false;
                    self.pending_open_file = false;
                }
            }
            self.mode = AppMode::Editing;
        }
    }

    fn after_close_confirm_action(&mut self) {
        if self.pending_open_file {
            self.pending_open_file = false;
            self.pending_new_file = false;
            self.mode = AppMode::OpenPicker;
            let files = filesystem::list_files().unwrap_or_default();
            self.file_picker.show(files);
        } else {
            self.pending_new_file = false;
            self.create_new_file();
        }
    }

    fn update_find_dialog(&mut self) {
        if let Some(result) = self.input_dialog.update() {
            match result {
                DialogResult::Confirm(query) => {
                    self.search_query = query;
                    if self.is_replacing {
                        // Move to replace dialog to get replacement text
                        self.mode = AppMode::ReplaceDialog;
                        self.input_dialog.show("Replace with:");
                    } else {
                        // Just find - search for first match
                        self.mode = AppMode::Editing;
                        self.find_next();
                    }
                }
                DialogResult::Reject | DialogResult::Cancel => {
                    self.mode = AppMode::Editing;
                }
            }
        }
    }

    fn update_replace_dialog(&mut self) {
        if let Some(result) = self.input_dialog.update() {
            match result {
                DialogResult::Confirm(replacement) => {
                    self.replace_text = replacement;
                    self.mode = AppMode::Editing;
                    // Find first match
                    self.find_next();
                }
                DialogResult::Reject | DialogResult::Cancel => {
                    self.mode = AppMode::Editing;
                }
            }
        }
    }

    fn update_message_dialog(&mut self) {
        if self.message_dialog.update() {
            self.mode = AppMode::Editing;
        }
    }

    fn show_message(&mut self, message: &str) {
        self.message_dialog.show(message);
        self.mode = AppMode::Message;
    }

    fn find_next(&mut self) {
        if self.search_query.is_empty() {
            return;
        }

        let text = self.buffer.to_string();
        let query = &self.search_query;

        // Start searching from current cursor position
        let cursor_idx = self.cursor.char_index(&self.buffer);
        let search_start = if self.current_match_pos == Some(cursor_idx) {
            // If we're at a match, search from after it
            cursor_idx + query.len()
        } else {
            cursor_idx
        };

        // Search from cursor to end
        if let Some(rel_pos) = text[search_start..].find(query) {
            let match_pos = search_start + rel_pos;
            self.select_match(match_pos, query.len());
            return;
        }

        // Wrap around: search from beginning to cursor
        if let Some(match_pos) = text[..cursor_idx].find(query) {
            self.select_match(match_pos, query.len());
            return;
        }

        // No match found
        self.current_match_pos = None;
    }

    fn select_match(&mut self, char_idx: usize, len: usize) {
        self.current_match_pos = Some(char_idx);

        // Move cursor to start of match
        let (line, col) = self.buffer.char_to_line_col(char_idx);
        self.cursor.set_position(line, col);

        // Select the match
        self.selection.start(self.cursor.position);
        let end_idx = char_idx + len;
        let (end_line, end_col) = self.buffer.char_to_line_col(end_idx);
        self.selection.cursor = CursorPosition {
            line: end_line,
            col: end_col,
        };

        self.ensure_cursor_visible();

        // If in replace mode and Enter is pressed, replace and find next
        if self.is_replacing {
            self.try_replace_current();
        }
    }

    fn try_replace_current(&mut self) {
        // This is called after finding a match in replace mode
        // The actual replacement happens when Enter is pressed again
        // For now, just highlight the match - replacement happens on next Enter
    }

    fn replace_and_find_next(&mut self) {
        let Some(match_pos) = self.current_match_pos else {
            return;
        };
        if self.search_query.is_empty() {
            return;
        }

        let query_len = self.search_query.len();

        // Save for undo
        self.history.push(&self.buffer, self.cursor.position);

        // Delete the matched text
        self.buffer.delete_range(match_pos, match_pos + query_len);

        // Insert replacement
        self.buffer.insert(match_pos, &self.replace_text);

        // Update cursor position
        let new_pos = match_pos + self.replace_text.len();
        let (line, col) = self.buffer.char_to_line_col(new_pos);
        self.cursor.set_position(line, col);
        self.selection.clear();

        self.is_modified = true;
        self.current_match_pos = None;

        // Find next match
        self.find_next();
    }

    fn ensure_cursor_visible(&mut self) {
        let visible_cols = self.visible_cols();
        let visible_lines = self.visible_lines();

        // Vertical scrolling
        if self.cursor.line() < self.scroll_y {
            self.scroll_y = self.cursor.line();
        } else if self.cursor.line() >= self.scroll_y + visible_lines {
            self.scroll_y = self.cursor.line() - visible_lines + 1;
        }

        // Horizontal scrolling
        if self.cursor.col() < self.scroll_x {
            self.scroll_x = self.cursor.col();
        } else if self.cursor.col() >= self.scroll_x + visible_cols {
            self.scroll_x = self.cursor.col() - visible_cols + 1;
        }
    }

    fn save_current_file(&mut self) -> bool {
        if let Some(ref filename) = self.current_filename {
            let content = self.buffer.to_string();
            match filesystem::write_file(filename, &content) {
                Ok(()) => {
                    self.is_modified = false;
                    true
                }
                Err(_) => {
                    self.show_message("Error: Could not save file");
                    false
                }
            }
        } else {
            false
        }
    }

    fn open_file(&mut self, filename: &str) {
        match filesystem::read_file(filename) {
            Ok(content) => {
                self.buffer = TextBuffer::from_str(&content);
                self.cursor = Cursor::new();
                self.selection = Selection::new();
                self.history.clear();
                self.scroll_x = 0;
                self.scroll_y = 0;
                self.current_filename = Some(filename.to_string());
                self.is_modified = false;
            }
            Err(_) => {
                self.show_message("Error: Could not open file");
            }
        }
    }

    fn create_new_file(&mut self) {
        self.buffer = TextBuffer::new();
        self.cursor = Cursor::new();
        self.selection = Selection::new();
        self.history.clear();
        self.scroll_x = 0;
        self.scroll_y = 0;
        self.current_filename = None;
        self.is_modified = false;
    }

    pub fn draw(&self) {
        // Clear with gray background
        clear_background(COLOR_GRAY);

        // Draw editor content (scaled)
        self.draw_editor();

        // Draw scrollbars (scaled)
        self.draw_scrollbars_scaled();

        // Draw dialogs (scaled)
        match self.mode {
            AppMode::SaveDialog | AppMode::FindDialog | AppMode::ReplaceDialog => {
                self.draw_input_dialog()
            }
            AppMode::OpenPicker => self.draw_file_picker(),
            AppMode::CloseConfirm => self.draw_confirm_dialog(),
            AppMode::Message => self.draw_message_dialog(),
            AppMode::Editing => {
                // Draw search hint if we have an active search
                if !self.search_query.is_empty() {
                    self.draw_search_hint();
                }
            }
        }
    }

    fn draw_search_hint(&self) {
        // Draw hint at bottom of screen for active search
        let hint = if self.is_replacing {
            "Enter:Replace+Next  Esc:Done"
        } else {
            "Enter:Find Next  Esc:Done"
        };
        let hint_x = TILE_WIDTH as f32;
        let hint_y = (SCREEN_HEIGHT - TILE_HEIGHT * 2) as f32;

        // Draw background
        self.draw_scaled_rect(
            0.0,
            hint_y - 2.0,
            SCREEN_WIDTH as f32,
            (TILE_HEIGHT + 4) as f32,
            COLOR_BLACK,
        );

        for (i, c) in hint.chars().enumerate() {
            self.draw_scaled_char_with_shadow(
                c,
                hint_x + (i as f32 * TILE_WIDTH as f32),
                hint_y,
                COLOR_WHITE,
                COLOR_GRAY,
            );
        }
    }

    fn draw_scaled_rect(&self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let scale = SCALE as f32;
        draw_rectangle(x * scale, y * scale, w * scale, h * scale, color);
    }

    fn draw_scaled_char(&self, c: char, x: f32, y: f32, color: Color) {
        let scale = SCALE as f32;
        self.font
            .draw_char_scaled(c, x * scale, y * scale, scale, color);
    }

    fn draw_scaled_char_with_shadow(&self, c: char, x: f32, y: f32, fg: Color, shadow: Color) {
        let scale = SCALE as f32;
        self.font
            .draw_char_with_shadow_scaled(c, x * scale, y * scale, scale, fg, shadow);
    }

    fn draw_editor(&self) {
        let visible_cols = self.visible_cols();
        let visible_lines = self.visible_lines();

        // Draw each visible line
        for screen_line in 0..visible_lines {
            let buffer_line = self.scroll_y + screen_line;
            if buffer_line >= self.buffer.line_count() {
                break;
            }

            let line_text = self.buffer.get_line(buffer_line);
            let y = (screen_line * TILE_HEIGHT as usize) as f32;

            // Get selection range for this line if any
            let selection_range = self.selection.get_line_selection(buffer_line, &self.buffer);

            // Draw each visible character
            for screen_col in 0..visible_cols {
                let buffer_col = self.scroll_x + screen_col;
                let x = (screen_col * TILE_WIDTH as usize) as f32;

                let c = line_text.chars().nth(buffer_col).unwrap_or(' ');

                let is_selected = match selection_range {
                    Some((start, end)) => buffer_col >= start && buffer_col < end,
                    None => false,
                };

                if is_selected {
                    // Draw selection background
                    self.draw_scaled_rect(x, y, TILE_WIDTH as f32, TILE_HEIGHT as f32, COLOR_WHITE);
                    // Draw character in inverted colors
                    self.draw_scaled_char(c, x, y, COLOR_BLACK);
                } else if c != ' ' {
                    // Draw character with shadow
                    self.draw_scaled_char_with_shadow(c, x, y, COLOR_WHITE, COLOR_BLACK);
                }
            }
        }

        // Draw cursor (only in editing mode and when visible)
        if self.mode == AppMode::Editing && self.cursor_visible {
            let cursor_screen_line = self.cursor.line().saturating_sub(self.scroll_y);
            let cursor_screen_col = self.cursor.col().saturating_sub(self.scroll_x);

            // Only draw if cursor is in visible area
            if self.cursor.line() >= self.scroll_y
                && self.cursor.line() < self.scroll_y + visible_lines
                && self.cursor.col() >= self.scroll_x
                && self.cursor.col() < self.scroll_x + visible_cols
            {
                let cursor_x = (cursor_screen_col * TILE_WIDTH as usize) as f32;
                let cursor_y = (cursor_screen_line * TILE_HEIGHT as usize) as f32;

                // Draw cursor as a block (slightly larger to cover text shadow)
                self.draw_scaled_rect(
                    cursor_x,
                    cursor_y,
                    TILE_WIDTH as f32 + 1.0,
                    TILE_HEIGHT as f32 + 1.0,
                    COLOR_WHITE,
                );

                // Draw character under cursor in inverted colors
                let line_text = self.buffer.get_line(self.cursor.line());
                let c = line_text.chars().nth(self.cursor.col()).unwrap_or(' ');
                self.draw_scaled_char(c, cursor_x, cursor_y, COLOR_BLACK);
            }
        }
    }

    fn draw_scrollbars_scaled(&self) {
        let state = &self.scrollbar_state;
        let track_color = Color::new(0.3, 0.3, 0.3, 1.0);
        let handle_color = COLOR_WHITE;

        // Vertical scrollbar (right edge)
        if state.vertical_visible {
            let track_x = (SCREEN_WIDTH - SCROLLBAR_WIDTH) as f32;
            let track_y = 0.0;
            let track_height = if state.horizontal_visible {
                SCREEN_HEIGHT - SCROLLBAR_WIDTH
            } else {
                SCREEN_HEIGHT
            } as f32;

            self.draw_scaled_rect(
                track_x,
                track_y,
                SCROLLBAR_WIDTH as f32,
                track_height,
                track_color,
            );

            let handle_height = (track_height * state.vertical_size).max(TILE_HEIGHT as f32);
            let handle_y = track_y + (track_height - handle_height) * state.vertical_position;
            self.draw_scaled_rect(
                track_x,
                handle_y,
                SCROLLBAR_WIDTH as f32,
                handle_height,
                handle_color,
            );
        }

        // Horizontal scrollbar (bottom edge)
        if state.horizontal_visible {
            let track_x = 0.0;
            let track_y = (SCREEN_HEIGHT - SCROLLBAR_WIDTH) as f32;
            let track_width = if state.vertical_visible {
                SCREEN_WIDTH - SCROLLBAR_WIDTH
            } else {
                SCREEN_WIDTH
            } as f32;

            self.draw_scaled_rect(
                track_x,
                track_y,
                track_width,
                SCROLLBAR_WIDTH as f32,
                track_color,
            );

            let handle_width = (track_width * state.horizontal_size).max(TILE_WIDTH as f32);
            let handle_x = track_x + (track_width - handle_width) * state.horizontal_position;
            self.draw_scaled_rect(
                handle_x,
                track_y,
                handle_width,
                SCROLLBAR_WIDTH as f32,
                handle_color,
            );
        }

        // Corner square when both are visible
        if state.vertical_visible && state.horizontal_visible {
            let corner_x = (SCREEN_WIDTH - SCROLLBAR_WIDTH) as f32;
            let corner_y = (SCREEN_HEIGHT - SCROLLBAR_WIDTH) as f32;
            self.draw_scaled_rect(
                corner_x,
                corner_y,
                SCROLLBAR_WIDTH as f32,
                SCROLLBAR_WIDTH as f32,
                track_color,
            );
        }
    }

    fn draw_input_dialog(&self) {
        let dialog = &self.input_dialog;
        if !dialog.visible {
            return;
        }

        let dialog_width = 28 * TILE_WIDTH;
        let dialog_height = 5 * TILE_HEIGHT;
        let dialog_x = ((SCREEN_WIDTH - dialog_width) / 2) as f32;
        let dialog_y = ((SCREEN_HEIGHT - dialog_height) / 2) as f32;

        // Background
        self.draw_scaled_rect(
            dialog_x,
            dialog_y,
            dialog_width as f32,
            dialog_height as f32,
            COLOR_BLACK,
        );

        // Border
        let scale = SCALE as f32;
        draw_rectangle_lines(
            dialog_x * scale,
            dialog_y * scale,
            dialog_width as f32 * scale,
            dialog_height as f32 * scale,
            2.0,
            COLOR_WHITE,
        );

        // Title
        let title_x = dialog_x + TILE_WIDTH as f32;
        let title_y = dialog_y + TILE_HEIGHT as f32;
        for (i, c) in dialog.title.chars().enumerate() {
            self.draw_scaled_char_with_shadow(
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
        self.draw_scaled_rect(
            input_x,
            input_y,
            input_width,
            TILE_HEIGHT as f32,
            COLOR_GRAY,
        );

        // Input text
        let max_chars = ((input_width / TILE_WIDTH as f32) as usize).saturating_sub(1);
        let display_input: String = if dialog.input.len() > max_chars {
            dialog
                .input
                .chars()
                .skip(dialog.input.len() - max_chars)
                .collect()
        } else {
            dialog.input.clone()
        };

        for (i, c) in display_input.chars().enumerate() {
            self.draw_scaled_char(
                c,
                input_x + (i as f32 * TILE_WIDTH as f32),
                input_y,
                COLOR_WHITE,
            );
        }

        // Cursor
        let cursor_x = input_x + (display_input.len() as f32 * TILE_WIDTH as f32);
        self.draw_scaled_rect(
            cursor_x,
            input_y,
            TILE_WIDTH as f32,
            TILE_HEIGHT as f32,
            COLOR_WHITE,
        );
    }

    fn draw_confirm_dialog(&self) {
        let dialog = &self.confirm_dialog;
        if !dialog.visible {
            return;
        }

        let dialog_width = 28 * TILE_WIDTH;
        let dialog_height = 4 * TILE_HEIGHT;
        let dialog_x = ((SCREEN_WIDTH - dialog_width) / 2) as f32;
        let dialog_y = ((SCREEN_HEIGHT - dialog_height) / 2) as f32;

        // Background
        self.draw_scaled_rect(
            dialog_x,
            dialog_y,
            dialog_width as f32,
            dialog_height as f32,
            COLOR_BLACK,
        );

        // Border
        let scale = SCALE as f32;
        draw_rectangle_lines(
            dialog_x * scale,
            dialog_y * scale,
            dialog_width as f32 * scale,
            dialog_height as f32 * scale,
            2.0,
            COLOR_WHITE,
        );

        // Message
        let msg_x = dialog_x + TILE_WIDTH as f32;
        let msg_y = dialog_y + TILE_HEIGHT as f32;
        for (i, c) in dialog.message.chars().enumerate() {
            self.draw_scaled_char_with_shadow(
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
            self.draw_scaled_char_with_shadow(
                c,
                hint_x + (i as f32 * TILE_WIDTH as f32),
                hint_y,
                COLOR_GRAY,
                COLOR_BLACK,
            );
        }
    }

    fn draw_message_dialog(&self) {
        let dialog = &self.message_dialog;
        if !dialog.visible {
            return;
        }

        let dialog_width = 28 * TILE_WIDTH;
        let dialog_height = 4 * TILE_HEIGHT;
        let dialog_x = ((SCREEN_WIDTH - dialog_width) / 2) as f32;
        let dialog_y = ((SCREEN_HEIGHT - dialog_height) / 2) as f32;

        // Background
        self.draw_scaled_rect(
            dialog_x,
            dialog_y,
            dialog_width as f32,
            dialog_height as f32,
            COLOR_BLACK,
        );

        // Border
        let scale = SCALE as f32;
        draw_rectangle_lines(
            dialog_x * scale,
            dialog_y * scale,
            dialog_width as f32 * scale,
            dialog_height as f32 * scale,
            2.0,
            COLOR_WHITE,
        );

        // Message
        let msg_x = dialog_x + TILE_WIDTH as f32;
        let msg_y = dialog_y + TILE_HEIGHT as f32;
        for (i, c) in dialog.message.chars().enumerate() {
            self.draw_scaled_char_with_shadow(
                c,
                msg_x + (i as f32 * TILE_WIDTH as f32),
                msg_y,
                COLOR_WHITE,
                COLOR_GRAY,
            );
        }

        // Instructions
        let hint = "(press any key)";
        let hint_x = dialog_x + TILE_WIDTH as f32;
        let hint_y = dialog_y + (2.5 * TILE_HEIGHT as f32);
        for (i, c) in hint.chars().enumerate() {
            self.draw_scaled_char_with_shadow(
                c,
                hint_x + (i as f32 * TILE_WIDTH as f32),
                hint_y,
                COLOR_GRAY,
                COLOR_BLACK,
            );
        }
    }

    fn draw_file_picker(&self) {
        let picker = &self.file_picker;
        if !picker.visible {
            return;
        }

        let picker_width = 30 * TILE_WIDTH;
        let picker_height = 24 * TILE_HEIGHT;
        let picker_x = ((SCREEN_WIDTH - picker_width) / 2) as f32;
        let picker_y = ((SCREEN_HEIGHT - picker_height) / 2) as f32;

        // Background
        self.draw_scaled_rect(
            picker_x,
            picker_y,
            picker_width as f32,
            picker_height as f32,
            COLOR_BLACK,
        );

        // Border
        let scale = SCALE as f32;
        draw_rectangle_lines(
            picker_x * scale,
            picker_y * scale,
            picker_width as f32 * scale,
            picker_height as f32 * scale,
            2.0,
            COLOR_WHITE,
        );

        // Title
        let title = "Open File";
        let title_x = picker_x + TILE_WIDTH as f32;
        let title_y = picker_y + TILE_HEIGHT as f32;
        for (i, c) in title.chars().enumerate() {
            self.draw_scaled_char_with_shadow(
                c,
                title_x + (i as f32 * TILE_WIDTH as f32),
                title_y,
                COLOR_WHITE,
                COLOR_GRAY,
            );
        }

        if picker.files.is_empty() {
            let msg = "No files found";
            let msg_x = picker_x + TILE_WIDTH as f32;
            let msg_y = picker_y + (3 * TILE_HEIGHT) as f32;
            for (i, c) in msg.chars().enumerate() {
                self.draw_scaled_char_with_shadow(
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
        let visible_items = 20;
        let visible_end = (picker.scroll_offset + visible_items).min(picker.files.len());

        for (i, file_idx) in (picker.scroll_offset..visible_end).enumerate() {
            let file = &picker.files[file_idx];
            let y = list_y + (i as f32 * TILE_HEIGHT as f32);
            let is_selected = file_idx == picker.selected_index;

            let display_name: String = if file.len() > max_filename_len {
                format!("{}...", &file[..max_filename_len - 3])
            } else {
                file.clone()
            };

            if is_selected {
                self.draw_scaled_rect(
                    list_x,
                    y,
                    (display_name.len() * TILE_WIDTH as usize) as f32,
                    TILE_HEIGHT as f32,
                    COLOR_WHITE,
                );
                for (j, c) in display_name.chars().enumerate() {
                    self.draw_scaled_char(
                        c,
                        list_x + (j as f32 * TILE_WIDTH as f32),
                        y,
                        COLOR_BLACK,
                    );
                }
            } else {
                for (j, c) in display_name.chars().enumerate() {
                    self.draw_scaled_char_with_shadow(
                        c,
                        list_x + (j as f32 * TILE_WIDTH as f32),
                        y,
                        COLOR_WHITE,
                        COLOR_BLACK,
                    );
                }
            }
        }

        // Instructions or delete confirmation
        let hint_x = picker_x + TILE_WIDTH as f32;
        let hint_y = picker_y + picker_height as f32 - (1.5 * TILE_HEIGHT as f32);

        if picker.mode == FilePickerMode::ConfirmDelete {
            // Show delete confirmation prompt
            let prompt = "Delete file? (y/n)";
            for (i, c) in prompt.chars().enumerate() {
                self.draw_scaled_char_with_shadow(
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
                self.draw_scaled_char_with_shadow(
                    c,
                    hint_x + (i as f32 * TILE_WIDTH as f32),
                    hint1_y,
                    COLOR_GRAY,
                    COLOR_BLACK,
                );
            }
            for (i, c) in hint2.chars().enumerate() {
                self.draw_scaled_char_with_shadow(
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
