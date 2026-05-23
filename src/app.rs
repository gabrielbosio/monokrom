use macroquad::prelude::*;

use crate::compiler;
use crate::config::{
    COLOR_BLACK, COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE, CURSOR_BLINK_RATE, EDITOR_TILES_X,
    EDITOR_TILES_Y, MAP_REGION_START, SCREEN_HEIGHT, SCREEN_TILES_X, SCREEN_TILES_Y, SCREEN_WIDTH,
    SCROLLBAR_WIDTH, SPRITE_REGION_START, TILE_HEIGHT, TILE_WIDTH,
};
use crate::editor::{
    operations, Cursor, CursorPosition, History, MapEditor, MapEditorAction, Selection,
    SpriteEditor, SpriteEditorAction, TextBuffer,
};
use crate::filesystem;
use crate::input::{get_editor_action, get_terminal_action, is_shift_pressed, EditorAction};
use crate::render::highlight::{self, CharStyle};
use crate::render::{BitmapFont, DrawHelpers, ScrollbarState};
use crate::terminal::{complete, parse_command, TerminalCommand, TerminalState, COMMAND_NAMES};
use crate::ui::{
    ConfirmDialog, DialogResult, FilePicker, FilePickerResult, InputDialog, MessageDialog,
};
use crate::vm::{Vm, VmResult};

#[cfg(target_arch = "wasm32")]
use crate::filesystem::web_io;

const EXAMPLES: &[(&str, &str)] = &[
    ("bricks", include_str!("../examples/bricks.mkr")),
    ("tictactoe", include_str!("../examples/tictactoe.mkr")),
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum PendingAction {
    #[default]
    None,
    NewFile,
    OpenFile,
    OpenNamed(String),
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
    /// Running a program
    Running,
    /// Sprite editor
    SpriteEditor,
    /// Map editor
    MapEditor,
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
    message_return_mode: AppMode,
    file_picker: FilePicker,
    pending_action: PendingAction,

    // Clipboard
    clipboard: operations::Clipboard,
    paste_cache: Option<String>,

    // Cursor blink
    cursor_blink_timer: f64,
    cursor_visible: bool,

    // Rendering
    font: BitmapFont,
    render_target: RenderTarget,

    // Syntax highlighting
    highlight_styles: Vec<CharStyle>,
    highlight_valid: bool,

    // VM
    run_state: Option<Vm>,
    player_mode: bool,
    run_return_mode: AppMode,

    // Sprite editor
    sprite_editor: SpriteEditor,

    // Map editor
    map_editor: MapEditor,
}

impl App {
    pub async fn new() -> Self {
        let mut t = TerminalState::new();
        t.push_output("monokrom");
        t.push_output("type 'help' for commands");
        t.push_output("");
        Self::init(AppMode::Terminal, t, None, false).await
    }

    pub async fn new_player(vm: Vm) -> Self {
        Self::init(AppMode::Running, TerminalState::new(), Some(vm), true).await
    }

    async fn init(
        mode: AppMode,
        terminal: TerminalState,
        run_state: Option<Vm>,
        player_mode: bool,
    ) -> Self {
        let font = BitmapFont::new().await;
        let rt = render_target(SCREEN_WIDTH, SCREEN_HEIGHT);
        rt.texture.set_filter(FilterMode::Nearest);
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
            terminal,

            current_filename: None,
            is_modified: false,

            mode,
            input_dialog: InputDialog::new(),
            confirm_dialog: ConfirmDialog::new(),
            message_dialog: MessageDialog::new(),
            message_return_mode: AppMode::Editing,
            file_picker: FilePicker::new(),
            pending_action: PendingAction::None,

            clipboard,
            paste_cache: None,

            cursor_blink_timer: 0.0,
            cursor_visible: true,

            font,
            render_target: rt,

            highlight_styles: Vec::new(),
            highlight_valid: false,

            run_state,
            player_mode,
            run_return_mode: AppMode::Terminal,

            sprite_editor: SpriteEditor::new(),
            map_editor: MapEditor::new(),
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
        if self.mode == AppMode::Running {
            self.update_running();
            return;
        }
        if self.mode == AppMode::SpriteEditor {
            self.update_sprite_editor();
            return;
        }
        if self.mode == AppMode::MapEditor {
            self.update_map_editor();
            return;
        }

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
            AppMode::Running | AppMode::SpriteEditor | AppMode::MapEditor => unreachable!(),
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

        // Recompute syntax highlighting if needed
        if !self.highlight_valid {
            let source = self.editor.buffer.to_string();
            self.highlight_styles = highlight::highlight(&source);
            self.highlight_valid = true;
        }
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
                | EditorAction::SelectToLineStart
                | EditorAction::SelectToLineEnd
                | EditorAction::SelectAll => self.handle_selection(action),
                EditorAction::Backspace
                | EditorAction::BackspaceWord
                | EditorAction::Delete
                | EditorAction::DeleteWord
                | EditorAction::RemoveTab => self.handle_delete(action),
                EditorAction::Cut | EditorAction::Copy | EditorAction::Paste => {
                    self.handle_clipboard(action)
                }
                EditorAction::Undo | EditorAction::Redo => self.handle_history(action),
                EditorAction::ScrollUp
                | EditorAction::ScrollDown
                | EditorAction::ScrollLineUp
                | EditorAction::ScrollLineDown => self.handle_scroll(action),
                EditorAction::Save
                | EditorAction::Open
                | EditorAction::New
                | EditorAction::Find
                | EditorAction::Replace
                | EditorAction::GoToLine => self.handle_file_action(action),
                EditorAction::RunProgram => self.run_program(),
                EditorAction::DialogCancel => {
                    if !self.search.query.is_empty() {
                        self.search.query.clear();
                        self.search.replace_text.clear();
                        self.search.match_pos = None;
                        self.search.is_replacing = false;
                        self.editor.selection.clear();
                    } else if is_shift_pressed() {
                        self.mode = AppMode::Terminal;
                    } else {
                        self.mode = AppMode::SpriteEditor;
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
                self.highlight_valid = false;
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
                self.smart_indent();
                self.is_modified = true;
                self.highlight_valid = false;
                self.ensure_cursor_visible();
            }
            EditorAction::InsertTab => {
                if self.is_in_search_mode() {
                    return;
                }
                let multiline = self
                    .editor
                    .selection
                    .get_ordered_positions()
                    .is_some_and(|(s, e)| s.line != e.line);
                if multiline {
                    operations::indent_lines(
                        &mut self.editor.buffer,
                        &mut self.editor.cursor,
                        &mut self.editor.selection,
                        &mut self.editor.history,
                    );
                } else {
                    self.editor
                        .history
                        .push(&self.editor.buffer, self.editor.cursor.position);
                    if let Some((start, end)) = self.editor.selection.get_range(&self.editor.buffer)
                    {
                        if start != end {
                            let (line, col) = self.editor.buffer.char_to_line_col(start);
                            self.editor.buffer.delete_range(start, end);
                            self.editor.cursor.set_position(line, col);
                            self.editor.selection.clear();
                        }
                    }
                    let idx = self.editor.cursor.char_index(&self.editor.buffer);
                    self.editor.buffer.insert(idx, "  ");
                    self.editor
                        .cursor
                        .set_position(self.editor.cursor.line(), self.editor.cursor.col() + 2);
                }
                self.is_modified = true;
                self.highlight_valid = false;
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
            EditorAction::SelectToLineStart => self.editor.cursor.move_to_line_start(),
            EditorAction::SelectToLineEnd => {
                self.editor.cursor.move_to_line_end(&self.editor.buffer)
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
            EditorAction::DeleteWord => operations::delete_word_after(
                &mut self.editor.buffer,
                &mut self.editor.cursor,
                &mut self.editor.selection,
                &mut self.editor.history,
            ),
            EditorAction::RemoveTab => {
                let multiline = self
                    .editor
                    .selection
                    .get_ordered_positions()
                    .is_some_and(|(s, e)| s.line != e.line);
                if multiline {
                    operations::dedent_lines(
                        &mut self.editor.buffer,
                        &mut self.editor.cursor,
                        &mut self.editor.selection,
                        &mut self.editor.history,
                    );
                } else {
                    operations::dedent(
                        &mut self.editor.buffer,
                        &mut self.editor.cursor,
                        &mut self.editor.history,
                    );
                }
            }
            _ => return,
        }
        self.is_modified = true;
        self.highlight_valid = false;
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
                if let Some(ref text) = text {
                    self.paste_cache = Some(text.clone());
                    self.is_modified = true;
                    self.highlight_valid = false;
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
                if let Some(ref text) = text {
                    self.paste_cache = Some(text.clone());
                    if !clipboard_ok && self.clipboard.is_some() {
                        self.show_message("Warning: Clipboard error");
                    }
                }
            }
            EditorAction::Paste => {
                if self.is_in_search_mode() {
                    return;
                }
                #[cfg(not(target_arch = "wasm32"))]
                if is_key_pressed(KeyCode::V) {
                    if let Some(text) = try_read_system_clipboard() {
                        self.paste_cache = Some(text);
                    }
                }
                if let Some(text) = self.paste_cache.clone() {
                    operations::paste(
                        &mut self.editor.buffer,
                        &mut self.editor.cursor,
                        &mut self.editor.selection,
                        &mut self.editor.history,
                        &text,
                    );
                    self.is_modified = true;
                    self.highlight_valid = false;
                    self.ensure_cursor_visible();
                }
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
        self.highlight_valid = false;
        self.ensure_cursor_visible();
    }

    fn handle_scroll(&mut self, action: EditorAction) {
        match action {
            EditorAction::ScrollUp | EditorAction::ScrollDown => {
                let page = self.visible_lines().saturating_sub(1).max(1);
                let new_line = match action {
                    EditorAction::ScrollUp => self.editor.cursor.line().saturating_sub(page),
                    _ => {
                        let last = self.editor.buffer.line_count().saturating_sub(1);
                        (self.editor.cursor.line() + page).min(last)
                    }
                };
                let line_len = self.editor.buffer.line_len(new_line);
                let col = self.editor.cursor.col().min(line_len);
                self.editor.cursor.set_position(new_line, col);
                self.after_cursor_move();
            }
            EditorAction::ScrollLineUp if self.view.scroll_y > 0 => {
                self.view.scroll_y -= 1;
            }
            EditorAction::ScrollLineDown => {
                let max = self
                    .editor
                    .buffer
                    .line_count()
                    .saturating_sub(self.visible_lines());
                if self.view.scroll_y < max {
                    self.view.scroll_y += 1;
                }
            }
            _ => {}
        }
    }

    fn run_program(&mut self) {
        self.run_return_mode = self.mode;
        let source = self.editor.buffer.to_string();
        if source.is_empty() {
            self.terminal.push_output("(empty buffer)");
            self.mode = AppMode::Terminal;
        } else {
            match compiler::compile(&source) {
                Ok(bc) => match Vm::new(&bc, get_time()) {
                    Ok(mut vm) => {
                        vm.memory[SPRITE_REGION_START..SPRITE_REGION_START + 4096]
                            .copy_from_slice(&self.sprite_editor.data);
                        vm.memory[MAP_REGION_START..MAP_REGION_START + 4096]
                            .copy_from_slice(&self.map_editor.data);
                        self.run_state = Some(vm);
                        self.mode = AppMode::Running;
                    }
                    Err(e) => {
                        self.terminal.push_output(&format!("error: {e}"));
                        self.mode = AppMode::Terminal;
                    }
                },
                Err(errors) => {
                    for e in &errors {
                        self.terminal.push_output(&format!("error: {e}"));
                    }
                    self.mode = AppMode::Terminal;
                }
            }
        }
    }

    fn export_game(&mut self, name: &str) {
        let source = self.editor.buffer.to_string();
        if source.is_empty() {
            self.terminal.push_output("(empty buffer)");
            return;
        }
        let bc = match compiler::compile(&source) {
            Ok(bc) => bc,
            Err(errors) => {
                for e in &errors {
                    self.terminal.push_output(&format!("error: {e}"));
                }
                return;
            }
        };
        let bc_bytes = bc.serialize();
        let mut payload = Vec::new();
        payload.extend(&(bc_bytes.len() as u32).to_le_bytes());
        payload.extend(&bc_bytes);
        payload.extend(&self.sprite_editor.data);
        payload.extend(&self.map_editor.data);
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.export_native(name, &payload);
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.export_wasm(name, &source);
            let _ = payload; // compiled successfully, that's all we need
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn export_native(&mut self, name: &str, payload: &[u8]) {
        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                self.terminal
                    .push_output(&format!("error reading executable: {e}"));
                return;
            }
        };
        let exe_bytes = match std::fs::read(&exe) {
            Ok(b) => b,
            Err(e) => {
                self.terminal
                    .push_output(&format!("error reading executable: {e}"));
                return;
            }
        };
        let mut output = exe_bytes;
        output.extend(payload);
        output.extend(&(payload.len() as u32).to_le_bytes());
        output.extend(b"MKRM");

        let out_path = std::env::current_dir().unwrap_or_default().join(name);
        match std::fs::write(&out_path, &output) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ =
                        std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(0o755));
                }
                self.terminal
                    .push_output(&format!("exported: {}", out_path.display()));
            }
            Err(e) => {
                self.terminal
                    .push_output(&format!("error writing file: {e}"));
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn export_wasm(&mut self, name: &str, source: &str) {
        let mut full_source = source.to_string();
        self.sprite_editor.append_data(&mut full_source);
        self.map_editor.append_data(&mut full_source);
        let escaped = full_source
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>{name}</title>
<style>body{{margin:0;background:#000;display:flex;justify-content:center;align-items:center;height:100vh}}canvas{{image-rendering:pixelated}}</style>
</head>
<body>
<script>var MONOKROM_GAME_SOURCE = "{escaped}";</script>
<script src="gl.js"></script>
<script src="sapp_jsutils.js"></script>
<script src="quad-storage.js"></script>
<script src="monokrom.js"></script>
<script>load("monokrom.wasm");</script>
</body>
</html>"#
        );
        let filename = format!("{name}.html");
        web_io::download(&filename, &html);
        self.terminal.push_output(&format!("exported: {filename}"));
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
                    self.pending_action = PendingAction::NewFile;
                    self.mode = AppMode::CloseConfirm;
                    self.confirm_dialog.show("Save changes?");
                } else {
                    self.create_new_file();
                }
            }
            EditorAction::Find => {
                self.search.is_replacing = false;
                self.mode = AppMode::FindDialog;
                let initial = self
                    .editor
                    .selection
                    .get_text(&self.editor.buffer)
                    .filter(|s| !s.contains('\n'))
                    .unwrap_or_default();
                self.input_dialog.show_with_text("Find:", &initial);
            }
            EditorAction::Replace => {
                self.search.is_replacing = true;
                self.mode = AppMode::FindDialog;
                let initial = self
                    .editor
                    .selection
                    .get_text(&self.editor.buffer)
                    .filter(|s| !s.contains('\n'))
                    .unwrap_or_default();
                self.input_dialog.show_with_text("Find:", &initial);
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
        let Some(result) = self.input_dialog.update() else {
            return;
        };
        let filename = match result {
            DialogResult::Confirm(name) => name,
            DialogResult::Reject | DialogResult::Cancel => {
                self.pending_action = PendingAction::None;
                self.mode = AppMode::Editing;
                return;
            }
        };
        if filename.is_empty() {
            self.input_dialog.show("Save as:");
            self.show_message("Error: Filename cannot be empty");
            return;
        }
        if !filesystem::is_valid_filename(&filename) {
            self.input_dialog.show("Save as:");
            self.show_message("Error: Invalid filename");
            return;
        }
        self.current_filename = Some(filename.clone());
        self.mode = AppMode::Editing;
        if !self.save_current_file() {
            return;
        }
        if self.pending_action != PendingAction::None {
            self.after_close_confirm_action();
        } else {
            self.show_message(&format!("Saved {filename}"));
        }
    }

    fn update_open_picker(&mut self) {
        if let Some(result) = self.file_picker.update() {
            match result {
                FilePickerResult::Open(filename) => {
                    if self.open_file(&filename) {
                        self.mode = AppMode::Editing;
                    } else {
                        self.show_message("Error: Could not open file");
                    }
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
        let action = std::mem::take(&mut self.pending_action);
        match action {
            PendingAction::OpenNamed(name) => {
                if self.open_file(&name) {
                    self.terminal.push_output(&format!("opened {name}"));
                    self.mode = AppMode::Editing;
                } else {
                    self.terminal
                        .push_output(&format!("error: could not open {name}"));
                }
            }
            PendingAction::OpenFile => match filesystem::list_files() {
                Ok(files) => {
                    self.mode = AppMode::OpenPicker;
                    self.file_picker.show(files);
                }
                Err(_) => {
                    self.show_message("Error: Cannot list files");
                }
            },
            PendingAction::NewFile => {
                self.create_new_file();
                self.mode = AppMode::Editing;
            }
            PendingAction::None => {}
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
            self.mode = self.message_return_mode;
        }
    }

    fn show_message(&mut self, message: &str) {
        self.message_dialog.show(message);
        self.message_return_mode = self.mode;
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
        self.highlight_valid = false;
        self.search.match_pos = None;

        // Find next match
        self.find_next();
    }

    /// After insert_newline, adjust indent based on previous line content
    fn smart_indent(&mut self) {
        let cur_line = self.editor.cursor.line();
        if cur_line == 0 {
            return;
        }
        let prev = self.editor.buffer.get_line(cur_line - 1);
        let trimmed = prev.trim();

        if trimmed == "end" {
            self.dedent_closing_keyword(cur_line);
            return;
        }

        // Dedent else/else-if line if needed, then fall through to add body indent
        if trimmed == "else" || trimmed.starts_with("else if ") {
            self.dedent_closing_keyword(cur_line);
        }

        let opens_block = trimmed == "else"
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("if ")
            || trimmed.starts_with("else if ")
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

    /// Dedent both the previous (closing keyword) line and the current line by 2 spaces,
    /// if the closing keyword is over-indented relative to its block opener.
    fn dedent_closing_keyword(&mut self, cur_line: usize) {
        if !self.should_dedent(cur_line - 1) {
            return;
        }
        self.dedent_line(cur_line - 1);
        self.dedent_line(cur_line);
        let col = self.editor.cursor.col().saturating_sub(2);
        self.editor.cursor.set_position(cur_line, col);
    }

    /// Check if a line should be dedented by finding the matching block opener
    fn should_dedent(&self, line: usize) -> bool {
        self.leading_spaces(line) > self.block_opener_indent(line)
    }

    /// Scan upward to find the indent of the block opener matching this line
    fn block_opener_indent(&self, line: usize) -> usize {
        let mut depth = 0usize;
        for i in (0..line).rev() {
            let trimmed = self.editor.buffer.get_line(i).trim().to_string();
            if trimmed == "end" {
                depth += 1;
            } else if trimmed.starts_with("if ")
                || trimmed.starts_with("while ")
                || trimmed.starts_with("for ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("struct ")
            {
                if depth == 0 {
                    return self.leading_spaces(i);
                }
                depth -= 1;
            }
        }
        0
    }

    fn leading_spaces(&self, line: usize) -> usize {
        self.editor
            .buffer
            .get_line(line)
            .chars()
            .take_while(|c| *c == ' ')
            .count()
    }

    /// Remove up to 2 leading spaces from a line
    fn dedent_line(&mut self, line: usize) {
        let content = self.editor.buffer.get_line(line);
        let leading: usize = content.chars().take_while(|c| *c == ' ').count();
        if leading == 0 {
            return;
        }
        let remove = leading.min(2);
        let start = self.editor.buffer.line_col_to_char(line, 0);
        self.editor.buffer.delete_range(start, start + remove);
    }

    fn ensure_cursor_visible(&mut self) {
        let visible_cols = self.visible_cols();
        let mut visible_lines = self.visible_lines();
        // Search hint bar covers 2 rows at the bottom
        if self.is_in_search_mode() {
            visible_lines = visible_lines.saturating_sub(2);
        }

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
            let mut content = self.editor.buffer.to_string();
            self.sprite_editor.append_data(&mut content);
            self.map_editor.append_data(&mut content);
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
        self.highlight_valid = false;
    }

    fn open_file(&mut self, filename: &str) -> bool {
        match filesystem::read_file(filename) {
            Ok(content) => {
                let (source, spr, map) = split_data_sections(&content);
                self.sprite_editor.data = spr;
                self.map_editor.data = map;
                self.reset_editor(TextBuffer::from_str(source), Some(filename.to_string()));
                true
            }
            Err(_) => false,
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
                    self.mode = if is_shift_pressed() {
                        AppMode::MapEditor
                    } else {
                        AppMode::Editing
                    };
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
                EditorAction::ScrollUp | EditorAction::ScrollLineUp => self.terminal.scroll_up(),
                EditorAction::ScrollDown | EditorAction::ScrollLineDown => {
                    self.terminal.scroll_down()
                }
                EditorAction::Autocomplete => self.handle_terminal_autocomplete(),
                EditorAction::RunProgram => self.run_program(),
                _ => {}
            }
        }
    }

    fn update_running(&mut self) {
        // Drain char queue so keypresses during gameplay don't leak into terminal
        while get_char_pressed().is_some() {}

        if is_key_pressed(KeyCode::Escape) {
            if self.player_mode {
                // TODO: prompt for confirmation before quitting an exported game.
                std::process::exit(0);
            }
            self.terminal.push_output("stopped");
            self.run_state = None;
            self.mode = self.run_return_mode;
            return;
        }

        let vm = match self.run_state.as_mut() {
            Some(vm) => vm,
            None => return,
        };

        if vm.halted {
            return;
        }

        vm.prev_buttons = vm.buttons;
        vm.buttons = sample_buttons();
        vm.current_time = get_time();

        match vm.run_until_flip() {
            Ok(VmResult::Flip | VmResult::Halted) => {
                for line in vm.trace_output.drain(..) {
                    self.terminal.push_output(&line);
                }
            }
            Ok(VmResult::Continue) => {}
            Err(e) => {
                self.terminal.push_output(&format!("runtime error: {e}"));
                self.run_state = None;
                self.mode = AppMode::Terminal;
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
                if self.is_modified {
                    self.pending_action = PendingAction::NewFile;
                    self.mode = AppMode::CloseConfirm;
                    self.confirm_dialog.show("Save changes?");
                } else {
                    self.create_new_file();
                    self.terminal.push_output("new file created");
                    self.mode = AppMode::Editing;
                }
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
                let mut content = self.editor.buffer.to_string();
                self.sprite_editor.append_data(&mut content);
                self.map_editor.append_data(&mut content);
                match filesystem::write_file(&name, &content) {
                    Ok(()) => {
                        self.current_filename = Some(name.clone());
                        self.is_modified = false;
                        self.terminal.push_output(&format!("saved {name}"));
                    }
                    Err(e) => self.terminal.push_output(&format!("error: {e}")),
                }
            }
            TerminalCommand::Open(name) => {
                if self.is_modified {
                    self.pending_action = PendingAction::OpenNamed(name);
                    self.mode = AppMode::CloseConfirm;
                    self.confirm_dialog.show("Save changes?");
                } else if self.open_file(&name) {
                    self.terminal.push_output(&format!("opened {name}"));
                    self.mode = AppMode::Editing;
                } else {
                    self.terminal
                        .push_output(&format!("error: could not open {name}"));
                }
            }
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
            TerminalCommand::Example(name) => match name {
                None => {
                    for (name, _) in EXAMPLES {
                        self.terminal.push_output(&format!("  {name}"));
                    }
                }
                Some(name) => match EXAMPLES.iter().find(|(n, _)| *n == name) {
                    Some((_, content)) => {
                        let filename = format!("{name}.mkr");
                        match filesystem::write_file(&filename, content) {
                            Ok(()) => {
                                if self.is_modified {
                                    self.pending_action = PendingAction::OpenNamed(filename);
                                    self.mode = AppMode::CloseConfirm;
                                    self.confirm_dialog.show("Save changes?");
                                } else {
                                    self.open_file(&filename);
                                    self.terminal.push_output(&format!("opened {filename}"));
                                    self.mode = AppMode::Editing;
                                }
                            }
                            Err(e) => self.terminal.push_output(&format!("error: {e}")),
                        }
                    }
                    None => self
                        .terminal
                        .push_output(&format!("unknown example: {name}")),
                },
            },
            TerminalCommand::Export(name) => self.export_game(&name),
            TerminalCommand::Run => self.run_program(),
            TerminalCommand::Stat => {
                let source = self.editor.buffer.to_string();
                if source.is_empty() {
                    self.terminal.push_output("(empty buffer)");
                } else {
                    match compiler::compile(&source) {
                        Ok(bc) => {
                            let size = bc.code.len();
                            let pct = size * 100 / 32768;
                            self.terminal
                                .push_output(&format!("{size} / 32768 bytes ({pct}%)"));
                        }
                        Err(errors) => {
                            for e in &errors {
                                self.terminal.push_output(&format!("error: {e}"));
                            }
                        }
                    }
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
                self.terminal.push_output("  example     list/load example");
                self.terminal
                    .push_output("  export <n>  export standalone game");
                self.terminal.push_output("  run         run program");
                self.terminal.push_output("  stat        bytecode size");
                self.terminal.push_output("  clear       clear screen");
                self.terminal.push_output("  help        show this");
                self.terminal.push_output("");
                self.terminal.push_output("shortcuts:");
                self.terminal.push_output("  pgup/pgdn   scroll output");
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

    fn update_sprite_editor(&mut self) {
        while get_char_pressed().is_some() {}
        let output = self.sprite_editor.update();
        if output.modified {
            self.is_modified = true;
        }
        match output.action {
            SpriteEditorAction::None => {}
            SpriteEditorAction::ExitToTextEditor => self.mode = AppMode::Editing,
            SpriteEditorAction::ExitToMapEditor => self.mode = AppMode::MapEditor,
            SpriteEditorAction::Save => self.save_and_show_message(),
            SpriteEditorAction::Run => self.run_program(),
        }
    }

    fn update_map_editor(&mut self) {
        while get_char_pressed().is_some() {}
        let output = self.map_editor.update();
        if output.modified {
            self.is_modified = true;
        }
        match output.action {
            MapEditorAction::None => {}
            MapEditorAction::ExitToSpriteEditor => self.mode = AppMode::SpriteEditor,
            MapEditorAction::ExitToTerminal => self.mode = AppMode::Terminal,
            MapEditorAction::Save => self.save_and_show_message(),
            MapEditorAction::Run => self.run_program(),
        }
    }

    fn save_and_show_message(&mut self) {
        if self.current_filename.is_some() && self.save_current_file() {
            let name = self.current_filename.as_deref().unwrap_or("");
            self.show_message(&format!("Saved {name}"));
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
        let helpers = DrawHelpers::new(&self.font, 1.0);

        // Set camera to render target (160x144, top-left origin)
        let camera = Camera2D {
            render_target: Some(self.render_target.clone()),
            zoom: vec2(2.0 / SCREEN_WIDTH as f32, -2.0 / SCREEN_HEIGHT as f32),
            target: vec2(SCREEN_WIDTH as f32 / 2.0, SCREEN_HEIGHT as f32 / 2.0),
            ..Default::default()
        };
        set_camera(&camera);

        if self.mode == AppMode::Running {
            clear_background(COLOR_BLACK);
            self.draw_running(&helpers);
        } else if self.mode == AppMode::SpriteEditor
            || (self.mode == AppMode::Message && self.message_return_mode == AppMode::SpriteEditor)
        {
            clear_background(COLOR_BLACK);
            self.sprite_editor.draw(&helpers);
            if self.mode == AppMode::Message {
                self.message_dialog.draw_scaled(&helpers);
            }
        } else if self.mode == AppMode::MapEditor
            || (self.mode == AppMode::Message && self.message_return_mode == AppMode::MapEditor)
        {
            clear_background(COLOR_BLACK);
            self.map_editor.draw(&self.sprite_editor, &helpers);
            if self.mode == AppMode::Message {
                self.message_dialog.draw_scaled(&helpers);
            }
        } else if self.mode == AppMode::Terminal {
            clear_background(COLOR_BLACK);
            self.draw_terminal(&helpers);
        } else {
            clear_background(COLOR_DARK_GRAY);
            self.draw_editor(&helpers);
            self.draw_scrollbars_scaled(&helpers);

            match self.mode {
                AppMode::SaveDialog
                | AppMode::FindDialog
                | AppMode::ReplaceDialog
                | AppMode::GoToLineDialog => self.input_dialog.draw_scaled(&helpers),
                AppMode::OpenPicker => self.file_picker.draw_scaled(&helpers),
                AppMode::CloseConfirm => self.confirm_dialog.draw_scaled(&helpers),
                AppMode::Message => self.message_dialog.draw_scaled(&helpers),
                AppMode::Editing => {
                    if !self.search.query.is_empty() {
                        self.draw_search_hint(&helpers);
                    }
                }
                AppMode::Terminal
                | AppMode::Running
                | AppMode::SpriteEditor
                | AppMode::MapEditor => {
                    unreachable!()
                }
            }
        }

        // Blit render target to window with integer scaling + letterboxing
        set_default_camera();
        clear_background(BLACK);
        let sx = (screen_width() / SCREEN_WIDTH as f32).floor().max(1.0);
        let sy = (screen_height() / SCREEN_HEIGHT as f32).floor().max(1.0);
        let scale = sx.min(sy);
        let w = SCREEN_WIDTH as f32 * scale;
        let h = SCREEN_HEIGHT as f32 * scale;
        let x = (screen_width() - w) / 2.0;
        let y = (screen_height() - h) / 2.0;
        draw_texture_ex(
            &self.render_target.texture,
            x,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                flip_y: true,
                ..Default::default()
            },
        );
    }

    fn draw_running(&self, helpers: &DrawHelpers) {
        let vm = match self.run_state.as_ref() {
            Some(vm) => vm,
            None => return,
        };
        let palette = [COLOR_BLACK, COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE];
        for y in 0..144 {
            for x in 0..160 {
                let pixel = vm.framebuffer[y * 160 + x] as usize & 3;
                let color = palette[pixel];
                if pixel != 0 {
                    helpers.draw_rect(x as f32, y as f32, 1.0, 1.0, color);
                }
            }
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
            let line_start_char = self.editor.buffer.line_col_to_char(buffer_line, 0);

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
                    let char_idx = line_start_char + buffer_col;
                    let style = self
                        .highlight_styles
                        .get(char_idx)
                        .copied()
                        .unwrap_or(CharStyle::Normal);
                    if style == CharStyle::Comment {
                        helpers.draw_char(c, x, y, COLOR_LIGHT_GRAY);
                    } else {
                        let fg = if style == CharStyle::Keyword {
                            COLOR_LIGHT_GRAY
                        } else {
                            COLOR_WHITE
                        };
                        helpers.draw_char_with_shadow(c, x, y, fg, COLOR_BLACK);
                    }
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

pub fn split_data_sections(content: &str) -> (&str, [u8; 4096], [u8; 4096]) {
    const SPR: &str = "\n__spr__\n";
    const MAP: &str = "\n__map__\n";
    let mut spr = [0u8; 4096];
    let mut map = [0u8; 4096];

    let spr_marker = content.find(SPR);
    let map_marker = content.find(MAP);

    let source_end = spr_marker.or(map_marker).unwrap_or(content.len());
    let source = &content[..source_end];

    if let Some(pos) = spr_marker {
        let start = pos + SPR.len();
        let end = map_marker.unwrap_or(content.len());
        parse_hex_section(&content[start..end], &mut spr);
    }
    if let Some(pos) = map_marker {
        let start = pos + MAP.len();
        parse_hex_section(&content[start..], &mut map);
    }

    (source, spr, map)
}

fn parse_hex_section(hex: &str, buf: &mut [u8]) {
    let mut offset = 0;
    for line in hex.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut i = 0;
        while i + 1 < line.len() && offset < buf.len() {
            if let Ok(b) = u8::from_str_radix(&line[i..i + 2], 16) {
                buf[offset] = b;
                offset += 1;
            }
            i += 2;
        }
    }
}

/// Read the system clipboard with a short timeout, on a worker thread.
///
/// arboard on X11 can block indefinitely when our process owns the clipboard, so a fresh
/// instance is used inside a thread and the main loop drops the result if it takes too long.
#[cfg(not(target_arch = "wasm32"))]
fn try_read_system_clipboard() -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let text = arboard::Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_text().ok());
        let _ = tx.send(text);
    });
    rx.recv_timeout(std::time::Duration::from_millis(100))
        .ok()
        .flatten()
}

fn sample_buttons() -> u8 {
    let mut b = 0u8;
    if is_key_down(KeyCode::Left) {
        b |= 1;
    }
    if is_key_down(KeyCode::Right) {
        b |= 2;
    }
    if is_key_down(KeyCode::Up) {
        b |= 4;
    }
    if is_key_down(KeyCode::Down) {
        b |= 8;
    }
    if is_key_down(KeyCode::Z) {
        b |= 16;
    }
    if is_key_down(KeyCode::X) {
        b |= 32;
    }
    if is_key_down(KeyCode::Enter) {
        b |= 64;
    }
    if is_key_down(KeyCode::RightShift) {
        b |= 128;
    }
    b
}
