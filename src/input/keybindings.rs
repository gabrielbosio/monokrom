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
    SelectAll,

    // Editing
    Backspace,
    Delete,
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
    ScrollLeft,
    ScrollRight,

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

    // Dialog navigation
    DialogConfirm,
    DialogCancel,
}

#[cfg(any(target_os = "macos", target_arch = "wasm32"))]
fn is_cmd_pressed() -> bool {
    is_key_down(KeyCode::LeftSuper) || is_key_down(KeyCode::RightSuper)
}

#[cfg(any(not(target_os = "macos"), target_arch = "wasm32"))]
fn is_ctrl_pressed() -> bool {
    is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
}

/// Check if modifier key is pressed.
/// Native macOS: Cmd. Native non-macOS: Ctrl.
/// WASM: Cmd or Ctrl (gl.js maps MetaLeft/MetaRight to LeftSuper/RightSuper).
#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
pub fn is_modifier_pressed() -> bool {
    is_cmd_pressed()
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos")))]
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
