use macroquad::prelude::*;
use std::sync::Mutex;

use crate::input::keybindings::*;

/// Key repeat timing constants
const KEY_REPEAT_DELAY: f32 = 0.4;  // Initial delay before repeat starts
const KEY_REPEAT_RATE: f32 = 0.03;  // Time between repeats

/// State for key repeat tracking
struct KeyRepeatState {
    last_key: Option<KeyCode>,
    time_held: f32,
    is_repeating: bool,
}

static KEY_REPEAT: Mutex<KeyRepeatState> = Mutex::new(KeyRepeatState {
    last_key: None,
    time_held: 0.0,
    is_repeating: false,
});

/// Drain all pending characters from the input queue
fn drain_char_queue() {
    while get_char_pressed().is_some() {}
}

/// Check if any navigation or control key is being held down
fn is_navigation_key_held() -> bool {
    is_key_down(KeyCode::Left) ||
    is_key_down(KeyCode::Right) ||
    is_key_down(KeyCode::Up) ||
    is_key_down(KeyCode::Down) ||
    is_key_down(KeyCode::Home) ||
    is_key_down(KeyCode::End) ||
    is_key_down(KeyCode::PageUp) ||
    is_key_down(KeyCode::PageDown) ||
    is_key_down(KeyCode::Backspace) ||
    is_key_down(KeyCode::Delete) ||
    is_key_down(KeyCode::Escape) ||
    is_key_down(KeyCode::Tab) ||
    is_key_down(KeyCode::Enter) ||
    is_key_down(KeyCode::KpEnter)
}

/// Check if a key should fire (either just pressed or repeating)
fn should_key_fire(key: KeyCode) -> bool {
    if is_key_pressed(key) {
        // Key was just pressed - reset repeat state
        if let Ok(mut state) = KEY_REPEAT.lock() {
            state.last_key = Some(key);
            state.time_held = 0.0;
            state.is_repeating = false;
        }
        return true;
    }

    if is_key_down(key) {
        // Key is being held - check for repeat
        if let Ok(mut state) = KEY_REPEAT.lock() {
            if state.last_key == Some(key) {
                let dt = get_frame_time();
                state.time_held += dt;

                if !state.is_repeating {
                    // Still in initial delay
                    if state.time_held >= KEY_REPEAT_DELAY {
                        state.is_repeating = true;
                        state.time_held = 0.0;
                        return true;
                    }
                } else {
                    // In repeat mode
                    if state.time_held >= KEY_REPEAT_RATE {
                        state.time_held = 0.0;
                        return true;
                    }
                }
            }
        }
    } else {
        // Key was released - clear state if it was this key
        if let Ok(mut state) = KEY_REPEAT.lock() {
            if state.last_key == Some(key) {
                state.last_key = None;
                state.time_held = 0.0;
                state.is_repeating = false;
            }
        }
    }

    false
}

/// Process keyboard input and return the corresponding action
pub fn get_editor_action() -> Option<EditorAction> {
    let modifier = is_modifier_pressed();
    let shift = is_shift_pressed();
    let alt = is_alt_pressed();

    // Check for special key combinations first

    // File operations (Cmd/Ctrl + key)
    if modifier && !shift && !alt {
        if is_key_pressed(KeyCode::S) {
            drain_char_queue();
            return Some(EditorAction::Save);
        }
        if is_key_pressed(KeyCode::O) {
            drain_char_queue();
            return Some(EditorAction::Open);
        }
        if is_key_pressed(KeyCode::N) {
            drain_char_queue();
            return Some(EditorAction::New);
        }
        if is_key_pressed(KeyCode::Z) {
            drain_char_queue();
            return Some(EditorAction::Undo);
        }
        if is_key_pressed(KeyCode::Y) {
            drain_char_queue();
            return Some(EditorAction::Redo);
        }
        if is_key_pressed(KeyCode::X) {
            drain_char_queue();
            return Some(EditorAction::Cut);
        }
        if is_key_pressed(KeyCode::C) {
            drain_char_queue();
            return Some(EditorAction::Copy);
        }
        if is_key_pressed(KeyCode::V) {
            drain_char_queue();
            return Some(EditorAction::Paste);
        }
        if is_key_pressed(KeyCode::A) {
            drain_char_queue();
            return Some(EditorAction::SelectAll);
        }

        // Scrolling with Cmd/Ctrl + Arrow
        if is_key_pressed(KeyCode::Up) {
            drain_char_queue();
            return Some(EditorAction::ScrollUp);
        }
        if is_key_pressed(KeyCode::Down) {
            drain_char_queue();
            return Some(EditorAction::ScrollDown);
        }
        if is_key_pressed(KeyCode::Left) {
            drain_char_queue();
            return Some(EditorAction::ScrollLeft);
        }
        if is_key_pressed(KeyCode::Right) {
            drain_char_queue();
            return Some(EditorAction::ScrollRight);
        }
    }

    // Shift + Cmd + Z for Redo (alternative)
    if modifier && shift && !alt && is_key_pressed(KeyCode::Z) {
        drain_char_queue();
        return Some(EditorAction::Redo);
    }

    // Option/Alt + Arrow for word navigation and line swapping
    if alt && !modifier {
        if is_key_pressed(KeyCode::Left) {
            drain_char_queue();
            return Some(EditorAction::MoveWordLeft);
        }
        if is_key_pressed(KeyCode::Right) {
            drain_char_queue();
            return Some(EditorAction::MoveWordRight);
        }
        if is_key_pressed(KeyCode::Up) {
            drain_char_queue();
            return Some(EditorAction::SwapLineUp);
        }
        if is_key_pressed(KeyCode::Down) {
            drain_char_queue();
            return Some(EditorAction::SwapLineDown);
        }
    }

    // Shift + Arrow for selection (with key repeat)
    if shift && !modifier && !alt {
        if should_key_fire(KeyCode::Left) {
            drain_char_queue();
            return Some(EditorAction::SelectLeft);
        }
        if should_key_fire(KeyCode::Right) {
            drain_char_queue();
            return Some(EditorAction::SelectRight);
        }
        if should_key_fire(KeyCode::Up) {
            drain_char_queue();
            return Some(EditorAction::SelectUp);
        }
        if should_key_fire(KeyCode::Down) {
            drain_char_queue();
            return Some(EditorAction::SelectDown);
        }
    }

    // Basic navigation (no modifiers, with key repeat)
    if !modifier && !alt && !shift {
        if should_key_fire(KeyCode::Left) {
            drain_char_queue();
            return Some(EditorAction::MoveLeft);
        }
        if should_key_fire(KeyCode::Right) {
            drain_char_queue();
            return Some(EditorAction::MoveRight);
        }
        if should_key_fire(KeyCode::Up) {
            drain_char_queue();
            return Some(EditorAction::MoveUp);
        }
        if should_key_fire(KeyCode::Down) {
            drain_char_queue();
            return Some(EditorAction::MoveDown);
        }
        if should_key_fire(KeyCode::Home) {
            drain_char_queue();
            return Some(EditorAction::MoveToLineStart);
        }
        if should_key_fire(KeyCode::End) {
            drain_char_queue();
            return Some(EditorAction::MoveToLineEnd);
        }
        if should_key_fire(KeyCode::Backspace) {
            drain_char_queue();
            return Some(EditorAction::Backspace);
        }
        if should_key_fire(KeyCode::Delete) {
            drain_char_queue();
            return Some(EditorAction::Delete);
        }
        if should_key_fire(KeyCode::Enter) || should_key_fire(KeyCode::KpEnter) {
            drain_char_queue();
            return Some(EditorAction::InsertNewline);
        }
        if is_key_pressed(KeyCode::Escape) {
            drain_char_queue();
            return Some(EditorAction::DialogCancel);
        }
    }

    // Tab key (no modifiers)
    if !modifier && !alt && is_key_pressed(KeyCode::Tab) {
        drain_char_queue();
        // Insert 4 spaces for tab
        return None; // We'll handle tab separately
    }

    // Text input - check for typed characters
    // Skip if any navigation key is held (to prevent OS key repeat from inserting chars)
    if !modifier && !is_navigation_key_held() {
        if let Some(c) = get_char_pressed() {
            // Filter out control characters except what we handle
            if c >= ' ' && c != '\x7f' {
                return Some(EditorAction::InsertChar(c));
            }
        }
    } else {
        // Drain any characters generated while navigation keys are held
        drain_char_queue();
    }

    None
}

/// Process keyboard input for dialogs
pub fn get_dialog_action() -> Option<EditorAction> {
    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
        return Some(EditorAction::DialogConfirm);
    }
    if is_key_pressed(KeyCode::Escape) {
        return Some(EditorAction::DialogCancel);
    }

    // For text input in dialogs
    if !is_modifier_pressed() {
        if let Some(c) = get_char_pressed() {
            if c >= ' ' && c != '\x7f' {
                return Some(EditorAction::InsertChar(c));
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            return Some(EditorAction::Backspace);
        }
    }

    None
}

/// Process keyboard input for file picker
pub fn get_file_picker_action() -> Option<EditorAction> {
    let modifier = is_modifier_pressed();

    // File operations with Cmd/Ctrl
    if modifier {
        if is_key_pressed(KeyCode::D) {
            return Some(EditorAction::DuplicateFile);
        }
        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            return Some(EditorAction::DeleteFile);
        }
    }

    if is_key_pressed(KeyCode::Up) {
        return Some(EditorAction::MoveUp);
    }
    if is_key_pressed(KeyCode::Down) {
        return Some(EditorAction::MoveDown);
    }
    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
        return Some(EditorAction::DialogConfirm);
    }
    if is_key_pressed(KeyCode::Escape) {
        return Some(EditorAction::DialogCancel);
    }

    None
}
