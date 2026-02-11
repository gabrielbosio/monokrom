use crate::editor::buffer::TextBuffer;
use crate::editor::cursor::Cursor;
use crate::editor::history::History;
use crate::editor::selection::Selection;

#[cfg(not(target_arch = "wasm32"))]
pub type Clipboard = Option<arboard::Clipboard>;

#[cfg(target_arch = "wasm32")]
pub type Clipboard = Option<String>;

/// Insert a character at the cursor position
pub fn insert_char(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
    c: char,
) {
    // Save state before change
    history.push(buffer, cursor.position);

    // Delete selection if active
    if let Some((start, end)) = selection.get_range(buffer) {
        if start != end {
            let (line, col) = buffer.char_to_line_col(start);
            buffer.delete_range(start, end);
            cursor.set_position(line, col);
            selection.clear();
        }
    }

    // Insert character
    let char_idx = cursor.char_index(buffer);
    buffer.insert(char_idx, &c.to_string());

    // Move cursor forward
    if c == '\n' {
        cursor.set_position(cursor.line() + 1, 0);
    } else {
        cursor.set_position(cursor.line(), cursor.col() + 1);
    }
}

/// Delete character before cursor (backspace)
pub fn delete_before(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    // If there's a selection, delete it
    if let Some((start, end)) = selection.get_range(buffer) {
        if start != end {
            history.push(buffer, cursor.position);
            let (line, col) = buffer.char_to_line_col(start);
            buffer.delete_range(start, end);
            cursor.set_position(line, col);
            selection.clear();
            return;
        }
    }

    // No selection, delete char before cursor
    if cursor.line() == 0 && cursor.col() == 0 {
        return; // Nothing to delete
    }

    history.push(buffer, cursor.position);

    if cursor.col() > 0 {
        // Delete character before cursor on same line
        buffer.delete_char_before(cursor.line(), cursor.col());
        cursor.set_position(cursor.line(), cursor.col() - 1);
    } else {
        // At beginning of line, merge with previous line
        let prev_line_len = buffer.line_len(cursor.line() - 1);
        buffer.delete_char_before(cursor.line(), cursor.col());
        cursor.set_position(cursor.line() - 1, prev_line_len);
    }

    selection.clear();
}

/// Delete character at cursor (delete key)
pub fn delete_at(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    // If there's a selection, delete it
    if let Some((start, end)) = selection.get_range(buffer) {
        if start != end {
            history.push(buffer, cursor.position);
            let (line, col) = buffer.char_to_line_col(start);
            buffer.delete_range(start, end);
            cursor.set_position(line, col);
            selection.clear();
            return;
        }
    }

    // No selection, delete char at cursor
    let char_idx = cursor.char_index(buffer);
    if char_idx >= buffer.len_chars() {
        return; // Nothing to delete
    }

    history.push(buffer, cursor.position);
    buffer.delete_char_at(cursor.line(), cursor.col());
    selection.clear();
}

/// Swap current line with line above (Option+Up)
pub fn swap_line_up(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    if cursor.line() == 0 {
        return; // Can't swap up from first line
    }

    history.push(buffer, cursor.position);

    let current_line = buffer.get_line(cursor.line());
    let prev_line = buffer.get_line(cursor.line() - 1);

    // Calculate char indices for the lines (including newlines)
    let prev_start = buffer.line_col_to_char(cursor.line() - 1, 0);
    let current_end = buffer.line_col_to_char(cursor.line(), buffer.line_len(cursor.line()));

    // Add newline if current line is not the last line
    let current_has_newline = cursor.line() < buffer.line_count() - 1;
    let end_idx = if current_has_newline {
        current_end + 1
    } else {
        current_end
    };

    // Delete both lines
    buffer.delete_range(prev_start, end_idx);

    // Insert in swapped order
    let new_content = if current_has_newline {
        format!("{}\n{}\n", current_line, prev_line)
    } else {
        format!("{}\n{}", current_line, prev_line)
    };
    buffer.insert(prev_start, &new_content);

    // Move cursor up
    cursor.set_position(cursor.line() - 1, cursor.col());
    selection.clear();
}

/// Swap current line with line below (Option+Down)
pub fn swap_line_down(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    if cursor.line() >= buffer.line_count() - 1 {
        return; // Can't swap down from last line
    }

    history.push(buffer, cursor.position);

    let current_line = buffer.get_line(cursor.line());
    let next_line = buffer.get_line(cursor.line() + 1);

    // Calculate char indices for the lines
    let current_start = buffer.line_col_to_char(cursor.line(), 0);
    let next_end = buffer.line_col_to_char(cursor.line() + 1, buffer.line_len(cursor.line() + 1));

    // Check if next line is the last line
    let next_has_newline = cursor.line() + 1 < buffer.line_count() - 1;
    let end_idx = if next_has_newline {
        next_end + 1
    } else {
        next_end
    };

    // Delete both lines
    buffer.delete_range(current_start, end_idx);

    // Insert in swapped order
    let new_content = if next_has_newline {
        format!("{}\n{}\n", next_line, current_line)
    } else {
        format!("{}\n{}", next_line, current_line)
    };
    buffer.insert(current_start, &new_content);

    // Move cursor down
    cursor.set_position(cursor.line() + 1, cursor.col());
    selection.clear();
}

/// Copy selected text to clipboard
/// Returns (copied_text, clipboard_success)
pub fn copy_selection(
    buffer: &TextBuffer,
    selection: &Selection,
    clipboard: &mut Clipboard,
) -> (Option<String>, bool) {
    let text = match selection.get_text(buffer) {
        Some(t) => t,
        None => return (None, true), // No selection is not an error
    };
    #[cfg(not(target_arch = "wasm32"))]
    let clipboard_ok = if let Some(cb) = clipboard.as_mut() {
        cb.set_text(&text).is_ok()
    } else {
        false
    };
    #[cfg(target_arch = "wasm32")]
    let clipboard_ok = {
        *clipboard = Some(text.clone());
        true
    };
    (Some(text), clipboard_ok)
}

/// Cut selected text to clipboard
/// Returns (cut_text, clipboard_success)
pub fn cut_selection(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
    clipboard: &mut Clipboard,
) -> (Option<String>, bool) {
    let text = match selection.get_text(buffer) {
        Some(t) => t,
        None => return (None, true), // No selection is not an error
    };

    // Copy to clipboard
    #[cfg(not(target_arch = "wasm32"))]
    let clipboard_ok = if let Some(cb) = clipboard.as_mut() {
        cb.set_text(&text).is_ok()
    } else {
        false
    };
    #[cfg(target_arch = "wasm32")]
    let clipboard_ok = {
        *clipboard = Some(text.clone());
        true
    };

    // Delete the selection
    if let Some((start, end)) = selection.get_range(buffer) {
        if start != end {
            history.push(buffer, cursor.position);
            let (line, col) = buffer.char_to_line_col(start);
            buffer.delete_range(start, end);
            cursor.set_position(line, col);
            selection.clear();
        }
    }

    (Some(text), clipboard_ok)
}

/// Paste from clipboard
pub fn paste(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
    clipboard: &mut Clipboard,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let text = clipboard.as_mut().and_then(|cb| cb.get_text().ok());
    #[cfg(target_arch = "wasm32")]
    let text = clipboard.clone();
    if let Some(text) = text {
        if text.is_empty() {
            return;
        }

        history.push(buffer, cursor.position);

        // Delete selection if active
        if let Some((start, end)) = selection.get_range(buffer) {
            if start != end {
                let (line, col) = buffer.char_to_line_col(start);
                buffer.delete_range(start, end);
                cursor.set_position(line, col);
            }
        }

        // Insert pasted text
        let char_idx = cursor.char_index(buffer);
        buffer.insert(char_idx, &text);

        // Move cursor to end of pasted text
        let new_idx = char_idx + text.chars().count();
        let (new_line, new_col) = buffer.char_to_line_col(new_idx);
        cursor.set_position(new_line, new_col);

        selection.clear();
    }
}

/// Undo last operation
pub fn undo(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    if let Some(entry) = history.undo(buffer, cursor.position) {
        buffer.restore(&entry.buffer);
        cursor.set_position(entry.cursor.line, entry.cursor.col);
        selection.clear();
    }
}

/// Redo last undone operation
pub fn redo(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    if let Some(entry) = history.redo(buffer, cursor.position) {
        buffer.restore(&entry.buffer);
        cursor.set_position(entry.cursor.line, entry.cursor.col);
        selection.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::cursor::CursorPosition;

    fn setup() -> (TextBuffer, Cursor, Selection, History) {
        (
            TextBuffer::from_str("hello world"),
            Cursor::new(),
            Selection::new(),
            History::new(),
        )
    }

    #[test]
    fn test_insert_char() {
        let (mut buffer, mut cursor, mut selection, mut history) = setup();
        cursor.set_position(0, 5);

        insert_char(&mut buffer, &mut cursor, &mut selection, &mut history, '!');

        assert_eq!(buffer.to_string(), "hello! world");
        assert_eq!(cursor.col(), 6);
    }

    #[test]
    fn test_insert_char_newline() {
        let (mut buffer, mut cursor, mut selection, mut history) = setup();
        cursor.set_position(0, 5);

        insert_char(&mut buffer, &mut cursor, &mut selection, &mut history, '\n');

        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.get_line(0), "hello");
        assert_eq!(buffer.get_line(1), " world");
        assert_eq!(cursor.line(), 1);
        assert_eq!(cursor.col(), 0);
    }

    #[test]
    fn test_insert_char_replaces_selection() {
        let (mut buffer, mut cursor, mut selection, mut history) = setup();
        selection.start(CursorPosition::new(0, 0));
        selection.cursor = CursorPosition::new(0, 5);
        cursor.set_position(0, 5);

        insert_char(&mut buffer, &mut cursor, &mut selection, &mut history, 'X');

        assert_eq!(buffer.to_string(), "X world");
        assert!(!selection.is_active());
    }

    #[test]
    fn test_delete_before() {
        let (mut buffer, mut cursor, mut selection, mut history) = setup();
        cursor.set_position(0, 5);

        delete_before(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.to_string(), "hell world");
        assert_eq!(cursor.col(), 4);
    }

    #[test]
    fn test_delete_before_at_start() {
        let (mut buffer, mut cursor, mut selection, mut history) = setup();

        delete_before(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.to_string(), "hello world");
        assert_eq!(cursor.col(), 0);
    }

    #[test]
    fn test_delete_before_at_line_start() {
        let mut buffer = TextBuffer::from_str("hello\nworld");
        let mut cursor = Cursor::new();
        let mut selection = Selection::new();
        let mut history = History::new();
        cursor.set_position(1, 0);

        delete_before(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.to_string(), "helloworld");
        assert_eq!(cursor.line(), 0);
        assert_eq!(cursor.col(), 5);
    }

    #[test]
    fn test_delete_before_with_selection() {
        let (mut buffer, mut cursor, mut selection, mut history) = setup();
        selection.start(CursorPosition::new(0, 0));
        selection.cursor = CursorPosition::new(0, 6);
        cursor.set_position(0, 6);

        delete_before(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.to_string(), "world");
        assert_eq!(cursor.col(), 0);
    }

    #[test]
    fn test_delete_at() {
        let (mut buffer, mut cursor, mut selection, mut history) = setup();
        cursor.set_position(0, 5);

        delete_at(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.to_string(), "helloworld");
    }

    #[test]
    fn test_delete_at_end() {
        let (mut buffer, mut cursor, mut selection, mut history) = setup();
        cursor.set_position(0, 11);

        delete_at(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.to_string(), "hello world");
    }

    #[test]
    fn test_swap_line_up() {
        let mut buffer = TextBuffer::from_str("line1\nline2\nline3");
        let mut cursor = Cursor::new();
        let mut selection = Selection::new();
        let mut history = History::new();
        cursor.set_position(1, 0);

        swap_line_up(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.get_line(0), "line2");
        assert_eq!(buffer.get_line(1), "line1");
        assert_eq!(cursor.line(), 0);
    }

    #[test]
    fn test_swap_line_up_at_first_line() {
        let mut buffer = TextBuffer::from_str("line1\nline2");
        let mut cursor = Cursor::new();
        let mut selection = Selection::new();
        let mut history = History::new();

        swap_line_up(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.get_line(0), "line1");
        assert_eq!(buffer.get_line(1), "line2");
    }

    #[test]
    fn test_swap_line_down() {
        let mut buffer = TextBuffer::from_str("line1\nline2\nline3");
        let mut cursor = Cursor::new();
        let mut selection = Selection::new();
        let mut history = History::new();
        cursor.set_position(0, 0);

        swap_line_down(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.get_line(0), "line2");
        assert_eq!(buffer.get_line(1), "line1");
        assert_eq!(cursor.line(), 1);
    }

    #[test]
    fn test_swap_line_down_at_last_line() {
        let mut buffer = TextBuffer::from_str("line1\nline2");
        let mut cursor = Cursor::new();
        let mut selection = Selection::new();
        let mut history = History::new();
        cursor.set_position(1, 0);

        swap_line_down(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.get_line(0), "line1");
        assert_eq!(buffer.get_line(1), "line2");
    }

    #[test]
    fn test_undo_redo() {
        let (mut buffer, mut cursor, mut selection, mut history) = setup();
        cursor.set_position(0, 5);

        insert_char(&mut buffer, &mut cursor, &mut selection, &mut history, '!');
        assert_eq!(buffer.to_string(), "hello! world");

        undo(&mut buffer, &mut cursor, &mut selection, &mut history);
        assert_eq!(buffer.to_string(), "hello world");

        redo(&mut buffer, &mut cursor, &mut selection, &mut history);
        assert_eq!(buffer.to_string(), "hello! world");
    }

    #[test]
    fn test_copy_selection_no_clipboard() {
        let buffer = TextBuffer::from_str("hello world");
        let mut selection = Selection::new();
        selection.start(CursorPosition::new(0, 0));
        selection.cursor = CursorPosition::new(0, 5);
        let mut clipboard: Clipboard = None;

        let (text, clipboard_ok) = copy_selection(&buffer, &selection, &mut clipboard);

        assert_eq!(text, Some("hello".to_string()));
        assert!(!clipboard_ok); // No clipboard available
    }

    #[test]
    fn test_copy_selection_no_selection() {
        let buffer = TextBuffer::from_str("hello");
        let selection = Selection::new();
        let mut clipboard: Clipboard = None;

        let (text, clipboard_ok) = copy_selection(&buffer, &selection, &mut clipboard);

        assert!(text.is_none());
        assert!(clipboard_ok); // No selection is not an error
    }

    #[test]
    fn test_cut_selection_no_clipboard() {
        let mut buffer = TextBuffer::from_str("hello world");
        let mut cursor = Cursor::new();
        let mut selection = Selection::new();
        let mut history = History::new();
        selection.start(CursorPosition::new(0, 0));
        selection.cursor = CursorPosition::new(0, 6);
        cursor.set_position(0, 6);
        let mut clipboard: Clipboard = None;

        let (text, clipboard_ok) = cut_selection(
            &mut buffer,
            &mut cursor,
            &mut selection,
            &mut history,
            &mut clipboard,
        );

        assert_eq!(text, Some("hello ".to_string()));
        assert!(!clipboard_ok); // No clipboard
        assert_eq!(buffer.to_string(), "world");
        assert!(!selection.is_active());
    }
}
