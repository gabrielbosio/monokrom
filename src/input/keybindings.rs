#![allow(dead_code)]

use macroquad::prelude::*;

/// Key combination for hotkey detection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyCombo {
    pub key: KeyCode,
    pub ctrl: bool, // Cmd on macOS
    pub shift: bool,
    pub alt: bool, // Option on macOS
}

impl KeyCombo {
    pub const fn new(key: KeyCode) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    pub const fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub const fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    pub const fn alt(mut self) -> Self {
        self.alt = true;
        self
    }
}

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

    // Find and replace
    Find,
    Replace,
    FindNext,

    // Dialog navigation
    DialogConfirm,
    DialogCancel,
}

/// Check if Cmd (Super) key is pressed (for macOS)
pub fn is_cmd_pressed() -> bool {
    is_key_down(KeyCode::LeftSuper) || is_key_down(KeyCode::RightSuper)
}

/// Check if Ctrl key is pressed
pub fn is_ctrl_pressed() -> bool {
    is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
}

/// Check if modifier key (Cmd on macOS, Ctrl otherwise) is pressed
pub fn is_modifier_pressed() -> bool {
    // On macOS, we use Cmd (Super) as the modifier
    #[cfg(target_os = "macos")]
    {
        is_cmd_pressed()
    }
    #[cfg(not(target_os = "macos"))]
    {
        is_ctrl_pressed()
    }
}

/// Check if Shift key is pressed
pub fn is_shift_pressed() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

/// Check if Alt/Option key is pressed
pub fn is_alt_pressed() -> bool {
    is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt)
}
