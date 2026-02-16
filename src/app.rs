use macroquad::prelude::*;

use crate::compiler::lexer;
use crate::config::{
    COLOR_BLACK, COLOR_DARK_GRAY, COLOR_WHITE, CURSOR_BLINK_RATE, EDITOR_TILES_X, EDITOR_TILES_Y,
    SCALE, SCREEN_HEIGHT, SCREEN_TILES_X, SCREEN_TILES_Y, SCREEN_WIDTH, SCROLLBAR_WIDTH,
    TILE_HEIGHT, TILE_WIDTH,
};
use crate::editor::{operations, Cursor, CursorPosition, History, Selection, TextBuffer};
use crate::filesystem;
use crate::input::{get_editor_action, get_terminal_action, EditorAction};
use crate::render::{BitmapFont, DrawHelpers, ScrollbarState};
use crate::terminal::{complete, parse_command, TerminalCommand, TerminalState, COMMAND_NAMES};
use crate::ui::{
    ConfirmDialog, DialogResult, FilePicker, FilePickerResult, InputDialog, MessageDialog,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PendingAction {
    #[default]
    None,
    OpenFile,
}

struct EditorState {
    buffer: TextBuffer,
    cursor: Cursor,
    selection: Selection,
    history: History,
}

struct ViewState {
    scroll_x: usize,
    scroll_y: usize,
    scrollbar_state: ScrollbarState,
}

struct SearchState {
    query: String,
    replace_text: String,
    match_pos: Option<usize>,
    is_replacing: bool,
}

/// Application state modes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMode {
    /// Terminal mode
    Terminal,
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
    /// Go to line dialog
    GoToLineDialog,
    /// Message dialog (error/info)
    Message,
}

pub struct App {
    editor: EditorState,
    view: ViewState,
    search: SearchState,
    terminal: TerminalState,

    // File state
    current_filename: Option<String>,
    is_modified: bool,

    // UI state
    mode: AppMode,
    input_dialog: InputDialog,
    confirm_dialog: ConfirmDialog,
    message_dialog: MessageDialog,
    file_picker: FilePicker,
    pending_action: PendingAction,

    // Clipboard
    clipboard: operations::Clipboard,

    // Cursor blink
    cursor_blink_timer: f64,
    cursor_visible: bool,

    // Rendering
    font: BitmapFont,
}

impl App {
    pub async fn new() -> Self {
        let font = BitmapFont::new().await;
        #[cfg(not(target_arch = "wasm32"))]
        let clipboard = arboard::Clipboard::new().ok();
        #[cfg(target_arch = "wasm32")]
        let clipboard: operations::Clipboard = None;

        Self {
            editor: EditorState {
                buffer: TextBuffer::new(),
                cursor: Cursor::new(),
                selection: Selection::new(),
                history: History::new(),
            },
            view: ViewState {
                scroll_x: 0,
                scroll_y: 0,
                scrollbar_state: ScrollbarState::default(),
            },
            search: SearchState {
                query: String::new(),
                replace_text: String::new(),
                match_pos: None,
                is_replacing: false,
            },
            terminal: {
                let mut t = TerminalState::new();
                t.push_output("monokrom");
                t.push_output("type 'help' for commands");
                t.push_output("");
                t
            },

            current_filename: None,
            is_modified: false,

            mode: AppMode::Terminal,
            input_dialog: InputDialog::new(),
            confirm_dialog: ConfirmDialog::new(),
            message_dialog: MessageDialog::new(),
            file_picker: FilePicker::new(),
            pending_action: PendingAction::None,

            clipboard,

            cursor_blink_timer: 0.0,
            cursor_visible: true,

            font,
        }
    }

    fn is_in_search_mode(&self) -> bool {
        !self.search.query.is_empty()
    }

    /// After cursor movement: clear selection and ensure cursor is visible
    fn after_cursor_move(&mut self) {
        self.editor
            .selection
            .clear_and_sync(self.editor.cursor.position);
        self.ensure_cursor_visible();
    }

    /// Start selection if not already selecting
    fn start_selection(&mut self) {
        if self.editor.selection.anchor.is_none() {
            self.editor.selection.anchor = Some(self.editor.cursor.position);
        }
    }

    /// After expanding selection: sync selection cursor and ensure visible
    fn after_selection_move(&mut self) {
        self.editor.selection.cursor = self.editor.cursor.position;
        self.ensure_cursor_visible();
    }

    pub fn update(&mut self) {
        // Update cursor blink
        self.cursor_blink_timer += get_frame_time() as f64;
        if self.cursor_blink_timer >= CURSOR_BLINK_RATE {
            self.cursor_blink_timer = 0.0;
            self.cursor_visible = !self.cursor_visible;
        }

        match self.mode {
            AppMode::Terminal => self.update_terminal(),
            AppMode::Editing => self.update_editing(),
            AppMode::SaveDialog => self.update_save_dialog(),
            AppMode::OpenPicker => self.update_open_picker(),
            AppMode::CloseConfirm => self.update_close_confirm(),
            AppMode::FindDialog => self.update_find_dialog(),
            AppMode::ReplaceDialog => self.update_replace_dialog(),
            AppMode::GoToLineDialog => self.update_goto_line_dialog(),
            AppMode::Message => self.update_message_dialog(),
        }

        // Update scrollbar state
        let visible_cols = self.visible_cols();
        let visible_lines = self.visible_lines();
        self.view.scrollbar_state.update(
            self.editor.buffer.line_count().max(1),
            visible_lines,
            self.view.scroll_y,
            self.editor.buffer.max_line_width(),
            visible_cols,
            self.view.scroll_x,
        );
    }

    fn visible_cols(&self) -> usize {
        (if self.view.scrollbar_state.vertical_visible {
            EDITOR_TILES_X
        } else {
            SCREEN_TILES_X
        }) as usize
    }

    fn visible_lines(&self) -> usize {
        (if self.view.scrollbar_state.horizontal_visible {
            EDITOR_TILES_Y
        } else {
            SCREEN_TILES_Y
        }) as usize
    }

    fn update_editing(&mut self) {
        if let Some(action) = get_editor_action() {
            self.cursor_visible = true;
            self.cursor_blink_timer = 0.0;

            match action {
                EditorAction::InsertChar(_)
                | EditorAction::InsertNewline
                | EditorAction::InsertTab => self.handle_text_input(action),
                EditorAction::MoveLeft
                | EditorAction::MoveRight
                | EditorAction::MoveUp
                | EditorAction::MoveDown
                | EditorAction::MoveWordLeft
                | EditorAction::MoveWordRight
                | EditorAction::MoveToLineStart
                | EditorAction::MoveToLineEnd => self.handle_movement(action),
                EditorAction::SelectLeft
                | EditorAction::SelectRight
                | EditorAction::SelectUp
                | EditorAction::SelectDown
                | EditorAction::SelectWordLeft
                | EditorAction::SelectWordRight
                | EditorAction::SelectAll => self.handle_selection(action),
                EditorAction::Backspace
                | EditorAction::BackspaceWord
                | EditorAction::Delete
                | EditorAction::RemoveTab
                | EditorAction::SwapLineUp
                | EditorAction::SwapLineDown => self.handle_delete(action),
                EditorAction::Cut | EditorAction::Copy | EditorAction::Paste => {
                    self.handle_clipboard(action)
                }
                EditorAction::Undo | EditorAction::Redo => self.handle_history(action),
                EditorAction::ScrollUp
                | EditorAction::ScrollDown
                | EditorAction::ScrollLeft
                | EditorAction::ScrollRight => self.handle_scroll(action),
                EditorAction::Save
                | EditorAction::Open
                | EditorAction::New
                | EditorAction::Find
                | EditorAction::Replace
                | EditorAction::GoToLine => self.handle_file_action(action),
                EditorAction::DialogCancel => {
                    if !self.search.query.is_empty() {
                        self.search.query.clear();
                        self.search.replace_text.clear();
                        self.search.match_pos = None;
                        self.search.is_replacing = false;
                        self.editor.selection.clear();
                    } else {
                        self.mode = AppMode::Terminal;
                    }
                }
                _ => {}
            }
        }
    }

    fn handle_text_input(&mut self, action: EditorAction) {
        match action {
            EditorAction::InsertChar(c) => {
                if self.is_in_search_mode() {
                    return;
                }
                operations::insert_char(
                    &mut self.editor.buffer,
                    &mut self.editor.cursor,
                    &mut self.editor.selection,
                    &mut self.editor.history,
                    c,
                );
                self.is_modified = true;
                self.ensure_cursor_visible();
            }
            EditorAction::InsertNewline => {
                if self.search.is_replacing && self.search.match_pos.is_some() {
                    self.replace_and_find_next();
                    return;
                }
                if self.is_in_search_mode() {
                    self.find_next();
                    return;
                }
                operations::insert_newline(
                    &mut self.editor.buffer,
                    &mut self.editor.cursor,
                    &mut self.editor.selection,
                    &mut self.editor.history,
                );
                if self.is_mkr_file() {
                    self.smart_indent();
                }
                self.is_modified = true;
                self.ensure_cursor_visible();
            }
            EditorAction::InsertTab => {
                if self.is_in_search_mode() {
                    return;
                }
                for c in [' ', ' '] {
                    operations::insert_char(
                        &mut self.editor.buffer,
                        &mut self.editor.cursor,
                        &mut self.editor.selection,
                        &mut self.editor.history,
                        c,
                    );
                }
                self.is_modified = true;
                self.ensure_cursor_visible();
            }
            _ => {}
        }
    }

    fn handle_movement(&mut self, action: EditorAction) {
        match action {
            EditorAction::MoveLeft => self.editor.cursor.move_left(&self.editor.buffer),
            EditorAction::MoveRight => self.editor.cursor.move_right(&self.editor.buffer),
            EditorAction::MoveUp => self.editor.cursor.move_up(&self.editor.buffer),
            EditorAction::MoveDown => self.editor.cursor.move_down(&self.editor.buffer),
            EditorAction::MoveWordLeft => self.editor.cursor.move_word_left(&self.editor.buffer),
            EditorAction::MoveWordRight => self.editor.cursor.move_word_right(&self.editor.buffer),
            EditorAction::MoveToLineStart => self.editor.cursor.move_to_line_start(),
            EditorAction::MoveToLineEnd => self.editor.cursor.move_to_line_end(&self.editor.buffer),
            _ => return,
        }
        self.after_cursor_move();
    }

    fn handle_selection(&mut self, action: EditorAction) {
        if action == EditorAction::SelectAll {
            self.editor.selection.anchor = Some(CursorPosition::new(0, 0));
            let last_line = self.editor.buffer.line_count().saturating_sub(1);
            let last_col = self.editor.buffer.line_len(last_line);
            self.editor.cursor.set_position(last_line, last_col);
            self.editor.selection.cursor = self.editor.cursor.position;
            return;
        }
        self.start_selection();
        match action {
            EditorAction::SelectLeft => self.editor.cursor.move_left(&self.editor.buffer),
            EditorAction::SelectRight => self.editor.cursor.move_right(&self.editor.buffer),
            EditorAction::SelectUp => self.editor.cursor.move_up(&self.editor.buffer),
            EditorAction::SelectDown => self.editor.cursor.move_down(&self.editor.buffer),
            EditorAction::SelectWordLeft => self.editor.cursor.move_word_left(&self.editor.buffer),
            EditorAction::SelectWordRight => {
                self.editor.cursor.move_word_right(&self.editor.buffer)
            }
            _ => return,
        }
        self.after_selection_move();
    }

    fn handle_delete(&mut self, action: EditorAction) {
        if self.is_in_search_mode() {
            return;
        }
        match action {
            EditorAction::Backspace => operations::delete_before(
                &mut self.editor.buffer,
                &mut self.editor.cursor,
                &mut self.editor.selection,
                &mut self.editor.history,
            ),
            EditorAction::BackspaceWord => operations::delete_word_before(
                &mut self.editor.buffer,
                &mut self.editor.cursor,
                &mut self.editor.selection,
                &mut self.editor.history,
            ),
            EditorAction::Delete => operations::delete_at(
                &mut self.editor.buffer,
                &mut self.editor.cursor,
                &mut self.editor.selection,
                &mut self.editor.history,
            ),
            EditorAction::RemoveTab => operations::dedent(
                &mut self.editor.buffer,
                &mut self.editor.cursor,
                &mut self.editor.history,
            ),
            EditorAction::SwapLineUp => operations::swap_line_up(
                &mut self.editor.buffer,
                &mut self.editor.cursor,
                &mut self.editor.selection,
                &mut self.editor.history,
            ),
            EditorAction::SwapLineDown => operations::swap_line_down(
                &mut self.editor.buffer,
                &mut self.editor.cursor,
                &mut self.editor.selection,
                &mut self.editor.history,
            ),
            _ => return,
        }
        self.is_modified = true;
        self.ensure_cursor_visible();
    }

    fn handle_clipboard(&mut self, action: EditorAction) {
        match action {
            EditorAction::Cut => {
                if self.is_in_search_mode() {
                    return;
                }
                let (text, clipboard_ok) = operations::cut_selection(
                    &mut self.editor.buffer,
                    &mut self.editor.cursor,
                    &mut self.editor.selection,
                    &mut self.editor.history,
                    &mut self.clipboard,
                );
                if text.is_some() {
                    self.is_modified = true;
                    if !clipboard_ok && self.clipboard.is_some() {
                        self.show_message("Warning: Clipboard error");
                    }
                }
            }
            EditorAction::Copy => {
                let (text, clipboard_ok) = operations::copy_selection(
                    &self.editor.buffer,
                    &self.editor.selection,
                    &mut self.clipboard,
                );
                if text.is_some() && !clipboard_ok && self.clipboard.is_some() {
                    self.show_message("Warning: Clipboard error");
                }
            }
            EditorAction::Paste => {
                if self.is_in_search_mode() {
                    return;
                }
                operations::paste(
                    &mut self.editor.buffer,
                    &mut self.editor.cursor,
                    &mut self.editor.selection,
                    &mut self.editor.history,
                    &mut self.clipboard,
                );
                self.is_modified = true;
                self.ensure_cursor_visible();
            }
            _ => {}
        }
    }

    fn handle_history(&mut self, action: EditorAction) {
        if self.is_in_search_mode() {
            return;
        }
        match action {
            EditorAction::Undo => operations::undo(
                &mut self.editor.buffer,
                &mut self.editor.cursor,
                &mut self.editor.selection,
                &mut self.editor.history,
            ),
            EditorAction::Redo => operations::redo(
                &mut self.editor.buffer,
                &mut self.editor.cursor,
                &mut self.editor.selection,
                &mut self.editor.history,
            ),
            _ => return,
        }
        self.ensure_cursor_visible();
    }

    fn handle_scroll(&mut self, action: EditorAction) {
        match action {
            EditorAction::ScrollUp => {
                if self.view.scroll_y > 0 {
                    self.view.scroll_y -= 1;
                }
            }
            EditorAction::ScrollDown => {
                let max = self
                    .editor
                    .buffer
                    .line_count()
                    .saturating_sub(self.visible_lines());
                if self.view.scroll_y < max {
                    self.view.scroll_y += 1;
                }
            }
            EditorAction::ScrollLeft => {
                if self.view.scroll_x > 0 {
                    self.view.scroll_x -= 1;
                }
            }
            EditorAction::ScrollRight => {
                let max = self
                    .editor
                    .buffer
                    .max_line_width()
                    .saturating_sub(self.visible_cols());
                if self.view.scroll_x < max {
                    self.view.scroll_x += 1;
                }
            }
            _ => {}
        }
    }

    fn handle_file_action(&mut self, action: EditorAction) {
        match action {
            EditorAction::Save => {
                if self.current_filename.is_some() {
                    if self.save_current_file() {
                        let name = self.current_filename.as_deref().unwrap_or("");
                        self.show_message(&format!("Saved {name}"));
                    }
                } else {
                    self.mode = AppMode::SaveDialog;
                    self.input_dialog.show("Save as:");
                }
            }
            EditorAction::Open => {
                if self.is_modified {
                    self.pending_action = PendingAction::OpenFile;
                    self.mode = AppMode::CloseConfirm;
                    self.confirm_dialog.show("Save changes?");
                } else {
                    match filesystem::list_files() {
                        Ok(files) => {
                            self.mode = AppMode::OpenPicker;
                            self.file_picker.show(files);
                        }
                        Err(_) => {
                            self.show_message("Error: Cannot list files");
                        }
                    }
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
                self.search.is_replacing = false;
                self.mode = AppMode::FindDialog;
                self.input_dialog.show("Find:");
            }
            EditorAction::Replace => {
                self.search.is_replacing = true;
                self.mode = AppMode::FindDialog;
                self.input_dialog.show("Find:");
            }
            EditorAction::GoToLine => {
                let line = self.editor.cursor.line() + 1;
                let col = self.editor.cursor.col() + 1;
                self.mode = AppMode::GoToLineDialog;
                self.input_dialog.show(&format!("Line ({line}:{col}):"));
            }
            _ => {}
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
                    self.current_filename = Some(filename.clone());
                    if self.save_current_file() {
                        // Check if we have a pending action after saving
                        if self.pending_action != PendingAction::None {
                            self.after_close_confirm_action();
                            return;
                        }
                        self.show_message(&format!("Saved {filename}"));
                        return;
                    } else {
                        // Save failed, error message already shown
                        return;
                    }
                }
                DialogResult::Reject | DialogResult::Cancel => {
                    self.pending_action = PendingAction::None;
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
                #[cfg(target_arch = "wasm32")]
                FilePickerResult::Export(filename) => {
                    if let Ok(content) = filesystem::read_file(&filename) {
                        filesystem::web_io::download(&filename, &content);
                    }
                }
                #[cfg(target_arch = "wasm32")]
                FilePickerResult::Import => {
                    filesystem::web_io::request_upload();
                }
                FilePickerResult::Cancel => {
                    self.mode = AppMode::Editing;
                }
            }
        }

        // Poll for pending file uploads (WASM only)
        #[cfg(target_arch = "wasm32")]
        if self.mode == AppMode::OpenPicker {
            if let Some((name, content)) = filesystem::web_io::take_pending_upload() {
                if filesystem::is_valid_filename(&name) {
                    let _ = filesystem::write_file(&name, &content);
                    if let Ok(files) = filesystem::list_files() {
                        let new_index = files.iter().position(|f| f == &name).unwrap_or(0);
                        self.file_picker.show(files);
                        self.file_picker.selected_index = new_index;
                    }
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
                    self.pending_action = PendingAction::None;
                }
            }
            self.mode = AppMode::Editing;
        }
    }

    fn after_close_confirm_action(&mut self) {
        let action = self.pending_action;
        self.pending_action = PendingAction::None;
        match action {
            PendingAction::OpenFile => match filesystem::list_files() {
                Ok(files) => {
                    self.mode = AppMode::OpenPicker;
                    self.file_picker.show(files);
                }
                Err(_) => {
                    self.show_message("Error: Cannot list files");
                }
            },
            _ => {
                self.create_new_file();
                self.mode = AppMode::Editing;
            }
        }
    }

    fn update_find_dialog(&mut self) {
        if let Some(result) = self.input_dialog.update() {
            match result {
                DialogResult::Confirm(query) => {
                    self.search.query = query;
                    if self.search.is_replacing {
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
                    self.search.replace_text = replacement;
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

    fn update_goto_line_dialog(&mut self) {
        if let Some(result) = self.input_dialog.update() {
            match result {
                DialogResult::Confirm(input) => {
                    let input = input.trim();
                    let (line_str, col_str) = input
                        .split_once(':')
                        .map(|(l, c)| (l, Some(c)))
                        .unwrap_or((input, None));
                    if let Ok(n) = line_str.parse::<usize>() {
                        let line = n
                            .saturating_sub(1)
                            .min(self.editor.buffer.line_count().saturating_sub(1));
                        let col = col_str
                            .and_then(|s| s.parse::<usize>().ok())
                            .map(|c| c.saturating_sub(1).min(self.editor.buffer.line_len(line)))
                            .unwrap_or(0);
                        self.editor.cursor.set_position(line, col);
                        self.after_cursor_move();
                    }
                }
                DialogResult::Reject | DialogResult::Cancel => {}
            }
            self.mode = AppMode::Editing;
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
        if self.search.query.is_empty() {
            return;
        }

        let text = self.editor.buffer.to_string();
        let query = &self.search.query;

        // Start searching from current cursor position
        let cursor_idx = self.editor.cursor.char_index(&self.editor.buffer);
        let search_start = if self.search.match_pos == Some(cursor_idx) {
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
        self.search.match_pos = None;
    }

    fn select_match(&mut self, char_idx: usize, len: usize) {
        self.search.match_pos = Some(char_idx);

        // Move cursor to start of match
        let (line, col) = self.editor.buffer.char_to_line_col(char_idx);
        self.editor.cursor.set_position(line, col);

        // Select the match
        self.editor.selection.start(self.editor.cursor.position);
        let end_idx = char_idx + len;
        let (end_line, end_col) = self.editor.buffer.char_to_line_col(end_idx);
        self.editor.selection.cursor = CursorPosition {
            line: end_line,
            col: end_col,
        };

        self.ensure_cursor_visible();
    }

    fn replace_and_find_next(&mut self) {
        let Some(match_pos) = self.search.match_pos else {
            return;
        };
        if self.search.query.is_empty() {
            return;
        }

        let query_len = self.search.query.len();

        // Save for undo
        self.editor
            .history
            .push(&self.editor.buffer, self.editor.cursor.position);

        // Delete the matched text
        self.editor
            .buffer
            .delete_range(match_pos, match_pos + query_len);

        // Insert replacement
        self.editor
            .buffer
            .insert(match_pos, &self.search.replace_text);

        // Update cursor position
        let new_pos = match_pos + self.search.replace_text.len();
        let (line, col) = self.editor.buffer.char_to_line_col(new_pos);
        self.editor.cursor.set_position(line, col);
        self.editor.selection.clear();

        self.is_modified = true;
        self.search.match_pos = None;

        // Find next match
        self.find_next();
    }

    fn is_mkr_file(&self) -> bool {
        self.current_filename
            .as_ref()
            .is_some_and(|f| f.ends_with(".mkr"))
    }

    /// After insert_newline, add extra indent if the previous line opens a block
    fn smart_indent(&mut self) {
        let cur_line = self.editor.cursor.line();
        if cur_line == 0 {
            return;
        }
        let prev = self.editor.buffer.get_line(cur_line - 1);
        let trimmed = prev.trim();
        let opens_block = trimmed.ends_with(')')
            || trimmed.ends_with("then")
            || trimmed == "else"
            || trimmed.starts_with("while ")
            || trimmed.starts_with("for ")
            || trimmed.starts_with("struct ");
        if opens_block {
            let idx = self.editor.cursor.char_index(&self.editor.buffer);
            self.editor.buffer.insert(idx, "  ");
            self.editor
                .cursor
                .set_position(cur_line, self.editor.cursor.col() + 2);
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let visible_cols = self.visible_cols();
        let visible_lines = self.visible_lines();

        // Vertical scrolling
        if self.editor.cursor.line() < self.view.scroll_y {
            self.view.scroll_y = self.editor.cursor.line();
        } else if self.editor.cursor.line() >= self.view.scroll_y + visible_lines {
            self.view.scroll_y = self.editor.cursor.line() - visible_lines + 1;
        }

        // Horizontal scrolling
        if self.editor.cursor.col() < self.view.scroll_x {
            self.view.scroll_x = self.editor.cursor.col();
        } else if self.editor.cursor.col() >= self.view.scroll_x + visible_cols {
            self.view.scroll_x = self.editor.cursor.col() - visible_cols + 1;
        }
    }

    fn save_current_file(&mut self) -> bool {
        if let Some(ref filename) = self.current_filename {
            let content = self.editor.buffer.to_string();
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

    fn reset_editor(&mut self, buffer: TextBuffer, filename: Option<String>) {
        self.editor.buffer = buffer;
        self.editor.cursor = Cursor::new();
        self.editor.selection = Selection::new();
        self.editor.history.clear();
        self.view.scroll_x = 0;
        self.view.scroll_y = 0;
        self.current_filename = filename;
        self.is_modified = false;
    }

    fn open_file(&mut self, filename: &str) {
        match filesystem::read_file(filename) {
            Ok(content) => {
                self.reset_editor(TextBuffer::from_str(&content), Some(filename.to_string()));
            }
            Err(_) => {
                self.show_message("Error: Could not open file");
            }
        }
    }

    fn create_new_file(&mut self) {
        self.reset_editor(TextBuffer::new(), None);
    }

    fn update_terminal(&mut self) {
        // Update cursor blink (reuse existing timer)
        if let Some(action) = get_terminal_action() {
            self.cursor_visible = true;
            self.cursor_blink_timer = 0.0;

            match action {
                EditorAction::DialogConfirm => {
                    let input = self.terminal.submit_input();
                    self.terminal.push_output(&format!("> {input}"));
                    if !input.is_empty() {
                        self.execute_terminal_command(&input);
                    }
                }
                EditorAction::DialogCancel => {
                    self.mode = AppMode::Editing;
                }
                EditorAction::InsertChar(c) => self.terminal.insert_char(c),
                EditorAction::Backspace => self.terminal.backspace(),
                EditorAction::Delete => self.terminal.delete(),
                EditorAction::MoveLeft => self.terminal.move_left(),
                EditorAction::MoveRight => self.terminal.move_right(),
                EditorAction::MoveToLineStart => self.terminal.move_to_start(),
                EditorAction::MoveToLineEnd => self.terminal.move_to_end(),
                EditorAction::MoveUp => self.terminal.recall_prev(),
                EditorAction::MoveDown => self.terminal.recall_next(),
                EditorAction::ScrollUp => self.terminal.scroll_up(),
                EditorAction::ScrollDown => self.terminal.scroll_down(),
                EditorAction::Autocomplete => self.handle_terminal_autocomplete(),
                _ => {}
            }
        }
    }

    fn execute_terminal_command(&mut self, input: &str) {
        match parse_command(input) {
            TerminalCommand::Ls => match filesystem::list_files() {
                Ok(files) => {
                    if files.is_empty() {
                        self.terminal.push_output("(no files)");
                    } else {
                        for f in &files {
                            self.terminal.push_output(f);
                        }
                    }
                }
                Err(e) => self.terminal.push_output(&format!("error: {e}")),
            },
            TerminalCommand::New => {
                self.create_new_file();
                self.terminal.push_output("new file created");
                self.mode = AppMode::Editing;
            }
            TerminalCommand::Save(name_arg) => {
                let name = match name_arg {
                    Some(n) => n,
                    None => match self.current_filename.clone() {
                        Some(n) => n,
                        None => {
                            self.terminal.push_output("save: missing filename");
                            return;
                        }
                    },
                };
                if !filesystem::is_valid_filename(&name) {
                    self.terminal.push_output("error: invalid filename");
                    return;
                }
                match filesystem::write_file(&name, &self.editor.buffer.to_string()) {
                    Ok(()) => {
                        self.current_filename = Some(name.clone());
                        self.is_modified = false;
                        self.terminal.push_output(&format!("saved {name}"));
                    }
                    Err(e) => self.terminal.push_output(&format!("error: {e}")),
                }
            }
            TerminalCommand::Open(name) => match filesystem::read_file(&name) {
                Ok(content) => {
                    let buffer = TextBuffer::from_str(&content);
                    self.reset_editor(buffer, Some(name.clone()));
                    self.terminal.push_output(&format!("opened {name}"));
                    self.mode = AppMode::Editing;
                }
                Err(e) => self.terminal.push_output(&format!("error: {e}")),
            },
            TerminalCommand::Rm(name) => match filesystem::delete_file(&name) {
                Ok(()) => self.terminal.push_output(&format!("removed {name}")),
                Err(e) => self.terminal.push_output(&format!("error: {e}")),
            },
            TerminalCommand::Cp(src, dst) => {
                if !filesystem::is_valid_filename(&dst) {
                    self.terminal.push_output("error: invalid destination name");
                    return;
                }
                match filesystem::read_file(&src) {
                    Ok(content) => match filesystem::write_file(&dst, &content) {
                        Ok(()) => self.terminal.push_output(&format!("copied {src} -> {dst}")),
                        Err(e) => self.terminal.push_output(&format!("error: {e}")),
                    },
                    Err(e) => self.terminal.push_output(&format!("error: {e}")),
                }
            }
            TerminalCommand::Run => {
                self.terminal.push_output("run: not implemented yet");
            }
            TerminalCommand::Lex => {
                let source = self.editor.buffer.to_string();
                if source.is_empty() {
                    self.terminal.push_output("(empty buffer)");
                } else {
                    let tokens = lexer::tokenize(&source);
                    for (tok, span) in &tokens {
                        self.terminal.push_output(&format!("{:?} {:?}", tok, span));
                    }
                    self.terminal
                        .push_output(&format!("{} tokens", tokens.len()));
                }
            }
            TerminalCommand::Clear => {
                self.terminal.clear();
            }
            TerminalCommand::Help => {
                self.terminal.push_output("commands:");
                self.terminal.push_output("  ls          list files");
                self.terminal.push_output("  new         new file");
                self.terminal.push_output("  save <name> save file");
                self.terminal.push_output("  open <name> open file");
                self.terminal.push_output("  rm <name>   remove file");
                self.terminal.push_output("  cp <s> <d>  copy file");
                self.terminal.push_output("  run         run program");
                self.terminal.push_output("  lex         tokenize buffer");
                self.terminal.push_output("  clear       clear screen");
                self.terminal.push_output("  help        show this");
                self.terminal.push_output("");
                self.terminal.push_output("shortcuts:");
                self.terminal.push_output("  cmd+up/down scroll output");
                self.terminal.push_output("  tab         autocomplete");
                self.terminal.push_output("  escape      back to editor");
            }
            TerminalCommand::Unknown(msg) => {
                if !msg.is_empty() {
                    self.terminal.push_output(&msg);
                }
            }
        }
    }

    fn handle_terminal_autocomplete(&mut self) {
        let input = &self.terminal.input_line;
        let already_shown = self.terminal.last_tab_input.as_deref() == Some(input);
        if input.contains(' ') {
            // Complete filename argument
            let space_pos = input.find(' ').unwrap();
            let prefix = &input[space_pos + 1..];
            let files = match filesystem::list_files() {
                Ok(f) => f,
                Err(_) => return,
            };
            let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
            if let Some((common, matches)) = complete(prefix, &file_refs) {
                let new_input = format!("{} {common}", &input[..space_pos]);
                self.terminal.input_line = new_input;
                self.terminal.cursor_pos = self.terminal.input_line.len();
                self.terminal.last_tab_input = Some(self.terminal.input_line.clone());
                if matches.len() > 1 && !already_shown {
                    for m in &matches {
                        self.terminal.push_output(m);
                    }
                }
            }
        } else {
            // Complete command name
            if let Some((common, matches)) = complete(input, COMMAND_NAMES) {
                self.terminal.input_line = common;
                self.terminal.cursor_pos = self.terminal.input_line.len();
                self.terminal.last_tab_input = Some(self.terminal.input_line.clone());
                if matches.len() > 1 && !already_shown {
                    for m in &matches {
                        self.terminal.push_output(m);
                    }
                }
            }
        }
    }

    fn draw_terminal(&self, helpers: &DrawHelpers) {
        let visible_lines = SCREEN_TILES_Y as usize - 1; // bottom row for input
        let tw = TILE_WIDTH as f32;
        let th = TILE_HEIGHT as f32;

        // Draw output lines
        for i in 0..visible_lines {
            let line_idx = self.terminal.scroll_offset + i;
            if line_idx >= self.terminal.output_lines.len() {
                break;
            }
            let line = &self.terminal.output_lines[line_idx];
            let y = (i as f32) * th;
            for (j, c) in line.chars().enumerate() {
                if j >= SCREEN_TILES_X as usize {
                    break;
                }
                helpers.draw_char(c, j as f32 * tw, y, COLOR_WHITE);
            }
        }

        // Draw input line at bottom
        let input_y = (visible_lines as f32) * th;
        helpers.draw_char('>', 0.0, input_y, COLOR_WHITE);
        for (j, c) in self.terminal.input_line.chars().enumerate() {
            let x = (j + 1) as f32 * tw;
            if j + 1 >= SCREEN_TILES_X as usize {
                break;
            }
            helpers.draw_char(c, x, input_y, COLOR_WHITE);
        }

        // Draw cursor
        if self.cursor_visible {
            let cursor_x = (self.terminal.cursor_pos + 1) as f32 * tw;
            helpers.draw_rect(cursor_x, input_y, tw + 1.0, th + 1.0, COLOR_WHITE);
            // Draw char under cursor in inverted color
            let c = self
                .terminal
                .input_line
                .chars()
                .nth(self.terminal.cursor_pos)
                .unwrap_or(' ');
            if c != ' ' {
                helpers.draw_char(c, cursor_x, input_y, COLOR_BLACK);
            }
        }
    }

    pub fn draw(&self) {
        // Clear with background color
        clear_background(if self.mode == AppMode::Terminal {
            COLOR_BLACK
        } else {
            COLOR_DARK_GRAY
        });

        let helpers = DrawHelpers::new(&self.font, SCALE as f32);

        if self.mode == AppMode::Terminal {
            self.draw_terminal(&helpers);
            return;
        }

        // Draw editor content (scaled)
        self.draw_editor(&helpers);

        // Draw scrollbars (scaled)
        self.draw_scrollbars_scaled(&helpers);

        // Draw dialogs (scaled)
        match self.mode {
            AppMode::SaveDialog
            | AppMode::FindDialog
            | AppMode::ReplaceDialog
            | AppMode::GoToLineDialog => self.input_dialog.draw_scaled(&helpers),
            AppMode::OpenPicker => self.file_picker.draw_scaled(&helpers),
            AppMode::CloseConfirm => self.confirm_dialog.draw_scaled(&helpers),
            AppMode::Message => self.message_dialog.draw_scaled(&helpers),
            AppMode::Editing => {
                // Draw search hint if we have an active search
                if !self.search.query.is_empty() {
                    self.draw_search_hint(&helpers);
                }
            }
            AppMode::Terminal => unreachable!(),
        }
    }

    fn draw_search_hint(&self, helpers: &DrawHelpers) {
        // Draw hint at bottom of screen for active search
        let hint = if self.search.is_replacing {
            "Enter:Replace+Next  Esc:Done"
        } else {
            "Enter:Find Next  Esc:Done"
        };
        let hint_x = TILE_WIDTH as f32;
        let hint_y = (SCREEN_HEIGHT - TILE_HEIGHT * 2) as f32;

        // Draw background
        helpers.draw_rect(
            0.0,
            hint_y - 2.0,
            SCREEN_WIDTH as f32,
            (TILE_HEIGHT + 4) as f32,
            COLOR_BLACK,
        );

        for (i, c) in hint.chars().enumerate() {
            helpers.draw_char_with_shadow(
                c,
                hint_x + (i as f32 * TILE_WIDTH as f32),
                hint_y,
                COLOR_WHITE,
                COLOR_DARK_GRAY,
            );
        }
    }

    fn draw_editor(&self, helpers: &DrawHelpers) {
        let visible_cols = self.visible_cols();
        let visible_lines = self.visible_lines();

        // Draw each visible line
        for screen_line in 0..visible_lines {
            let buffer_line = self.view.scroll_y + screen_line;
            if buffer_line >= self.editor.buffer.line_count() {
                break;
            }

            let line_text = self.editor.buffer.get_line(buffer_line);
            let y = (screen_line * TILE_HEIGHT as usize) as f32;

            // Get selection range for this line if any
            let selection_range = self
                .editor
                .selection
                .get_line_selection(buffer_line, &self.editor.buffer);

            // Draw each visible character
            for screen_col in 0..visible_cols {
                let buffer_col = self.view.scroll_x + screen_col;
                let x = (screen_col * TILE_WIDTH as usize) as f32;

                let c = line_text.chars().nth(buffer_col).unwrap_or(' ');

                let is_selected = match selection_range {
                    Some((start, end)) => buffer_col >= start && buffer_col < end,
                    None => false,
                };

                if is_selected {
                    // Draw selection background
                    helpers.draw_rect(x, y, TILE_WIDTH as f32, TILE_HEIGHT as f32, COLOR_WHITE);
                    // Draw character in inverted colors
                    helpers.draw_char(c, x, y, COLOR_BLACK);
                } else if c != ' ' {
                    // Draw character with shadow
                    helpers.draw_char_with_shadow(c, x, y, COLOR_WHITE, COLOR_BLACK);
                }
            }
        }

        // Draw cursor (only in editing mode and when visible)
        if self.mode == AppMode::Editing && self.cursor_visible {
            let cursor_screen_line = self.editor.cursor.line().saturating_sub(self.view.scroll_y);
            let cursor_screen_col = self.editor.cursor.col().saturating_sub(self.view.scroll_x);

            // Only draw if cursor is in visible area
            if self.editor.cursor.line() >= self.view.scroll_y
                && self.editor.cursor.line() < self.view.scroll_y + visible_lines
                && self.editor.cursor.col() >= self.view.scroll_x
                && self.editor.cursor.col() < self.view.scroll_x + visible_cols
            {
                let cursor_x = (cursor_screen_col * TILE_WIDTH as usize) as f32;
                let cursor_y = (cursor_screen_line * TILE_HEIGHT as usize) as f32;

                // Draw cursor as a block (slightly larger to cover text shadow)
                helpers.draw_rect(
                    cursor_x,
                    cursor_y,
                    TILE_WIDTH as f32 + 1.0,
                    TILE_HEIGHT as f32 + 1.0,
                    COLOR_WHITE,
                );

                // Draw character under cursor in inverted colors
                let line_text = self.editor.buffer.get_line(self.editor.cursor.line());
                let c = line_text
                    .chars()
                    .nth(self.editor.cursor.col())
                    .unwrap_or(' ');
                helpers.draw_char(c, cursor_x, cursor_y, COLOR_BLACK);
            }
        }
    }

    fn draw_scrollbars_scaled(&self, helpers: &DrawHelpers) {
        let state = &self.view.scrollbar_state;
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

            helpers.draw_rect(
                track_x,
                track_y,
                SCROLLBAR_WIDTH as f32,
                track_height,
                track_color,
            );

            let handle_height = (track_height * state.vertical_size).max(TILE_HEIGHT as f32);
            let handle_y = track_y + (track_height - handle_height) * state.vertical_position;
            helpers.draw_rect(
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

            helpers.draw_rect(
                track_x,
                track_y,
                track_width,
                SCROLLBAR_WIDTH as f32,
                track_color,
            );

            let handle_width = (track_width * state.horizontal_size).max(TILE_WIDTH as f32);
            let handle_x = track_x + (track_width - handle_width) * state.horizontal_position;
            helpers.draw_rect(
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
            helpers.draw_rect(
                corner_x,
                corner_y,
                SCROLLBAR_WIDTH as f32,
                SCROLLBAR_WIDTH as f32,
                track_color,
            );
        }
    }
}
