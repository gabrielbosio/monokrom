use macroquad::prelude::*;

/// Editor actions that can be triggered by keyboard
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorAction {
    // Text input
    InsertChar(char),
    InsertNewline,

    // Cursor movement
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveWordLeft,
    MoveWordRight,
    MoveToLineStart,
    MoveToLineEnd,

    // Selection
    SelectLeft,
    SelectRight,
    SelectUp,
    SelectDown,
    SelectWordLeft,
    SelectWordRight,
    SelectToLineStart,
    SelectToLineEnd,
    SelectAll,

    // Editing
    Backspace,
    BackspaceWord,
    Delete,
    InsertTab,
    RemoveTab,
    SwapLineUp,
    SwapLineDown,

    // Clipboard
    Cut,
    Copy,
    Paste,

    // History
    Undo,
    Redo,

    // Scrolling
    ScrollUp,
    ScrollDown,

    // File operations
    Save,
    Open,
    New,

    // File picker operations
    DuplicateFile,
    DeleteFile,

    // File export/import (WASM only)
    #[cfg(target_arch = "wasm32")]
    Export,
    #[cfg(target_arch = "wasm32")]
    Import,

    // Find and replace
    Find,
    Replace,
    GoToLine,

    // Run
    RunProgram,

    // Terminal
    Autocomplete,

    // Dialog navigation
    DialogConfirm,
    DialogCancel,
}

// --- WASM Cmd timer (browser needs Cmd detection with timeout) ---

#[cfg(target_arch = "wasm32")]
static CMD_LAST_PRESS: std::sync::Mutex<f64> = std::sync::Mutex::new(0.0);

#[cfg(target_arch = "wasm32")]
pub fn refresh_cmd_timer() {
    if let Ok(mut t) = CMD_LAST_PRESS.lock() {
        *t = get_time();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn refresh_cmd_timer() {}

#[cfg(target_arch = "wasm32")]
fn is_cmd_pressed() -> bool {
    const TIMEOUT: f64 = 5.0;

    if is_key_pressed(KeyCode::LeftSuper) || is_key_pressed(KeyCode::RightSuper) {
        refresh_cmd_timer();
    }

    if !is_key_down(KeyCode::LeftSuper) && !is_key_down(KeyCode::RightSuper) {
        return false;
    }

    if is_key_down(KeyCode::Up)
        || is_key_down(KeyCode::Down)
        || is_key_down(KeyCode::Left)
        || is_key_down(KeyCode::Right)
    {
        refresh_cmd_timer();
    }

    if let Ok(t) = CMD_LAST_PRESS.lock() {
        if get_time() - *t > TIMEOUT {
            return false;
        }
    }

    true
}

// --- Modifier detection ---

fn is_ctrl_pressed() -> bool {
    is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
}

/// Check if modifier key is pressed.
/// Native: Ctrl. WASM: Cmd or Ctrl.
#[cfg(not(target_arch = "wasm32"))]
pub fn is_modifier_pressed() -> bool {
    is_ctrl_pressed()
}

#[cfg(target_arch = "wasm32")]
pub fn is_modifier_pressed() -> bool {
    is_cmd_pressed() || is_ctrl_pressed()
}

/// Check if Shift key is pressed
pub fn is_shift_pressed() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

/// Check if Alt/Option key is pressed
pub fn is_alt_pressed() -> bool {
    is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt)
}

/// Shortcut modifier for letter-key combos: Alt on WASM, Ctrl on native.
#[cfg(not(target_arch = "wasm32"))]
pub fn is_shortcut_modifier_pressed() -> bool {
    is_modifier_pressed()
}

#[cfg(target_arch = "wasm32")]
pub fn is_shortcut_modifier_pressed() -> bool {
    is_alt_pressed()
}
