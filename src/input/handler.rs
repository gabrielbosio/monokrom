use macroquad::prelude::*;
use std::sync::Mutex;

use crate::input::keybindings::{
    is_alt_pressed, is_modifier_pressed, is_shift_pressed, is_shortcut_modifier_pressed,
    refresh_cmd_timer, EditorAction,
};

/// Key repeat timing constants
const KEY_REPEAT_DELAY: f32 = 0.4; // Initial delay before repeat starts
const KEY_REPEAT_RATE: f32 = 0.03; // Time between repeats

/// State for key repeat tracking
struct KeyRepeatState {
    last_key: Option<KeyCode>,
    time_held: f32,
    is_repeating: bool,
    with_modifier: bool,
}

static KEY_REPEAT: Mutex<KeyRepeatState> = Mutex::new(KeyRepeatState {
    last_key: None,
    time_held: 0.0,
    is_repeating: false,
    with_modifier: false,
});

/// Drain all pending characters from the input queue
fn drain_char_queue() {
    while get_char_pressed().is_some() {}
}

/// Check if a key should fire (either just pressed or repeating)
fn should_key_fire(key: KeyCode, with_modifier: bool) -> bool {
    let mut state = match KEY_REPEAT.lock() {
        Ok(s) => s,
        Err(_) => return is_key_pressed(key),
    };

    // Key just pressed - reset and fire
    if is_key_pressed(key) {
        state.last_key = Some(key);
        state.time_held = 0.0;
        state.is_repeating = false;
        state.with_modifier = with_modifier;
        return true;
    }

    // Key released - clear state if tracked
    if !is_key_down(key) {
        if state.last_key == Some(key) {
            state.last_key = None;
            state.time_held = 0.0;
            state.is_repeating = false;
        }
        return false;
    }

    // Key held but not tracked
    if state.last_key != Some(key) {
        return false;
    }

    // Modifier context changed (e.g. released Ctrl while arrow still held) - reset
    if state.with_modifier != with_modifier {
        state.last_key = None;
        state.time_held = 0.0;
        state.is_repeating = false;
        return false;
    }

    let dt = get_frame_time();
    state.time_held += dt;

    // Check threshold
    let threshold = if state.is_repeating {
        KEY_REPEAT_RATE
    } else {
        KEY_REPEAT_DELAY
    };
    if state.time_held < threshold {
        return false;
    }

    // Fire repeat
    state.is_repeating = true;
    state.time_held = 0.0;
    true
}

// Ctrl + letter bindings (single-fire, no repeat).
// On WASM, these are triggered by Alt instead (see is_shortcut_modifier_pressed).
const MODIFIER_BINDINGS: &[(KeyCode, EditorAction)] = &[
    (KeyCode::S, EditorAction::Save),
    (KeyCode::O, EditorAction::Open),
    (KeyCode::N, EditorAction::New),
    (KeyCode::X, EditorAction::Cut),
    (KeyCode::C, EditorAction::Copy),
    (KeyCode::A, EditorAction::SelectAll),
    (KeyCode::F, EditorAction::Find),
    (KeyCode::R, EditorAction::Replace),
    (KeyCode::L, EditorAction::GoToLine),
];

// Ctrl + letter bindings (with key repeat for hold-to-repeat)
const MODIFIER_REPEAT_BINDINGS: &[(KeyCode, EditorAction)] = &[
    (KeyCode::Z, EditorAction::Undo),
    (KeyCode::Y, EditorAction::Redo),
    (KeyCode::V, EditorAction::Paste),
];

// Alt/Option + arrow bindings (with key repeat)
const ALT_BINDINGS: &[(KeyCode, EditorAction)] = &[
    (KeyCode::Left, EditorAction::MoveWordLeft),
    (KeyCode::Right, EditorAction::MoveWordRight),
    (KeyCode::Up, EditorAction::SwapLineUp),
    (KeyCode::Down, EditorAction::SwapLineDown),
    (KeyCode::Backspace, EditorAction::BackspaceWord),
];

// Shift + Alt bindings (select by word)
const SHIFT_ALT_BINDINGS: &[(KeyCode, EditorAction)] = &[
    (KeyCode::Left, EditorAction::SelectWordLeft),
    (KeyCode::Right, EditorAction::SelectWordRight),
];

// Shift + key bindings (with key repeat)
const SHIFT_BINDINGS: &[(KeyCode, EditorAction)] = &[
    (KeyCode::Home, EditorAction::SelectToLineStart),
    (KeyCode::End, EditorAction::SelectToLineEnd),
    (KeyCode::Left, EditorAction::SelectLeft),
    (KeyCode::Right, EditorAction::SelectRight),
    (KeyCode::Up, EditorAction::SelectUp),
    (KeyCode::Down, EditorAction::SelectDown),
];

// No-modifier bindings (with key repeat)
const PLAIN_BINDINGS: &[(KeyCode, EditorAction)] = &[
    (KeyCode::Left, EditorAction::MoveLeft),
    (KeyCode::Right, EditorAction::MoveRight),
    (KeyCode::Up, EditorAction::MoveUp),
    (KeyCode::Down, EditorAction::MoveDown),
    (KeyCode::Home, EditorAction::MoveToLineStart),
    (KeyCode::End, EditorAction::MoveToLineEnd),
    (KeyCode::PageUp, EditorAction::ScrollUp),
    (KeyCode::PageDown, EditorAction::ScrollDown),
    (KeyCode::Backspace, EditorAction::Backspace),
    (KeyCode::Delete, EditorAction::Delete),
];

fn check_pressed(bindings: &[(KeyCode, EditorAction)]) -> Option<EditorAction> {
    for &(key, action) in bindings {
        if is_key_pressed(key) {
            drain_char_queue();
            return Some(action);
        }
    }
    None
}

fn check_repeating(
    bindings: &[(KeyCode, EditorAction)],
    with_modifier: bool,
) -> Option<EditorAction> {
    for &(key, action) in bindings {
        if should_key_fire(key, with_modifier) {
            drain_char_queue();
            return Some(action);
        }
    }
    None
}

/// Process keyboard input and return the corresponding action
pub fn get_editor_action() -> Option<EditorAction> {
    let modifier = is_modifier_pressed();
    let shift = is_shift_pressed();
    let alt = is_alt_pressed();
    let shortcut_mod = is_shortcut_modifier_pressed();

    // Letter-key shortcuts: Alt on WASM, Ctrl on native
    #[cfg(not(target_arch = "wasm32"))]
    let shortcut_only = shortcut_mod && !shift && !alt;
    #[cfg(target_arch = "wasm32")]
    let shortcut_only = shortcut_mod && !shift && !modifier;

    if shortcut_only {
        if let Some(action) = check_repeating(MODIFIER_REPEAT_BINDINGS, true) {
            refresh_cmd_timer();
            return Some(action);
        }
        if let Some(action) = check_pressed(MODIFIER_BINDINGS) {
            refresh_cmd_timer();
            return Some(action);
        }
    }

    // Ctrl+Enter to run program
    if modifier
        && !shift
        && !alt
        && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter))
    {
        drain_char_queue();
        return Some(EditorAction::RunProgram);
    }

    // Shift + shortcut + Z for Redo (alternative)
    #[cfg(not(target_arch = "wasm32"))]
    let redo_alt = shortcut_mod && shift && !alt && is_key_pressed(KeyCode::Z);
    #[cfg(target_arch = "wasm32")]
    let redo_alt = shortcut_mod && shift && !modifier && is_key_pressed(KeyCode::Z);
    if redo_alt {
        drain_char_queue();
        refresh_cmd_timer();
        return Some(EditorAction::Redo);
    }

    if shift && alt && !modifier {
        if let Some(action) = check_repeating(SHIFT_ALT_BINDINGS, false) {
            return Some(action);
        }
    }

    if alt && !modifier && !shift {
        if let Some(action) = check_repeating(ALT_BINDINGS, false) {
            return Some(action);
        }
    }

    if shift && !modifier && !alt {
        if is_key_pressed(KeyCode::Tab) {
            drain_char_queue();
            return Some(EditorAction::RemoveTab);
        }
        if let Some(action) = check_repeating(SHIFT_BINDINGS, false) {
            return Some(action);
        }
    }

    if !modifier && !alt && !shift {
        if let Some(action) = check_repeating(PLAIN_BINDINGS, false) {
            return Some(action);
        }
        if should_key_fire(KeyCode::Enter, false) || should_key_fire(KeyCode::KpEnter, false) {
            drain_char_queue();
            return Some(EditorAction::InsertNewline);
        }
        if is_key_pressed(KeyCode::Escape) {
            drain_char_queue();
            return Some(EditorAction::DialogCancel);
        }
        if is_key_pressed(KeyCode::Tab) {
            drain_char_queue();
            return Some(EditorAction::InsertTab);
        }
    }

    // Text input - check for typed characters
    if let Some(c) = get_char_pressed() {
        if (' '..='~').contains(&c) {
            return Some(EditorAction::InsertChar(c));
        }
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
            if (' '..='~').contains(&c) {
                return Some(EditorAction::InsertChar(c));
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            return Some(EditorAction::Backspace);
        }
    }

    None
}

/// Process keyboard input for terminal
pub fn get_terminal_action() -> Option<EditorAction> {
    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
        return Some(EditorAction::DialogConfirm);
    }
    if is_key_pressed(KeyCode::Escape) {
        return Some(EditorAction::DialogCancel);
    }

    let modifier = is_modifier_pressed();

    // PageUp/PageDown for scrollback (with hold-to-repeat)
    if should_key_fire(KeyCode::PageUp, false) {
        return Some(EditorAction::ScrollUp);
    }
    if should_key_fire(KeyCode::PageDown, false) {
        return Some(EditorAction::ScrollDown);
    }

    if !modifier && is_key_pressed(KeyCode::Tab) {
        drain_char_queue();
        return Some(EditorAction::Autocomplete);
    }

    if !modifier {
        // Up/Down for command history
        if is_key_pressed(KeyCode::Up) {
            return Some(EditorAction::MoveUp);
        }
        if is_key_pressed(KeyCode::Down) {
            return Some(EditorAction::MoveDown);
        }

        if should_key_fire(KeyCode::Left, false) {
            return Some(EditorAction::MoveLeft);
        }
        if should_key_fire(KeyCode::Right, false) {
            return Some(EditorAction::MoveRight);
        }
        if should_key_fire(KeyCode::Home, false) {
            return Some(EditorAction::MoveToLineStart);
        }
        if should_key_fire(KeyCode::End, false) {
            return Some(EditorAction::MoveToLineEnd);
        }
        if should_key_fire(KeyCode::Backspace, false) {
            return Some(EditorAction::Backspace);
        }
        if should_key_fire(KeyCode::Delete, false) {
            return Some(EditorAction::Delete);
        }

        if let Some(c) = get_char_pressed() {
            if (' '..='~').contains(&c) {
                return Some(EditorAction::InsertChar(c));
            }
        }
    }

    None
}

/// Process keyboard input for file picker
pub fn get_file_picker_action() -> Option<EditorAction> {
    let shortcut_mod = is_shortcut_modifier_pressed();

    // File operations: Alt on WASM, Ctrl on native
    if shortcut_mod {
        if is_key_pressed(KeyCode::D) {
            return Some(EditorAction::DuplicateFile);
        }
        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            return Some(EditorAction::DeleteFile);
        }
        #[cfg(target_arch = "wasm32")]
        {
            if is_key_pressed(KeyCode::E) {
                return Some(EditorAction::Export);
            }
            if is_key_pressed(KeyCode::I) {
                return Some(EditorAction::Import);
            }
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
