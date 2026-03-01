use crate::editor::buffer::TextBuffer;
use crate::editor::cursor::Cursor;
use crate::editor::history::History;
use crate::editor::selection::Selection;

#[cfg(not(target_arch = "wasm32"))]
pub type Clipboard = Option<arboard::Clipboard>;

#[cfg(target_arch = "wasm32")]
pub type Clipboard = Option<String>;

/// If a selection is active, save undo state, delete the selected range, reposition cursor, and
/// clear the selection. Returns true if a selection was deleted.
fn delete_selection(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) -> bool {
    if let Some((start, end)) = selection.get_range(buffer) {
        if start != end {
            history.push(buffer, cursor.position);
            let (line, col) = buffer.char_to_line_col(start);
            buffer.delete_range(start, end);
            cursor.set_position(line, col);
            selection.clear();
            return true;
        }
    }
    false
}

/// Insert a character at the cursor position
pub fn insert_char(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
    c: char,
) {
    if !delete_selection(buffer, cursor, selection, history) {
        history.push(buffer, cursor.position);
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

/// Insert a newline at the cursor position, preserving the current line's indentation
pub fn insert_newline(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    if !delete_selection(buffer, cursor, selection, history) {
        history.push(buffer, cursor.position);
    }

    let indent: String = buffer
        .get_line(cursor.line())
        .chars()
        .take_while(|c| *c == ' ')
        .collect();

    let char_idx = cursor.char_index(buffer);
    let text = format!("\n{indent}");
    buffer.insert(char_idx, &text);
    cursor.set_position(cursor.line() + 1, indent.len());
}

/// Delete character before cursor (backspace)
pub fn delete_before(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    if delete_selection(buffer, cursor, selection, history) {
        return;
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

/// Delete word before cursor (Alt+Backspace)
pub fn delete_word_before(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    if delete_selection(buffer, cursor, selection, history) {
        return;
    }

    if cursor.line() == 0 && cursor.col() == 0 {
        return;
    }

    history.push(buffer, cursor.position);

    let end_idx = cursor.char_index(buffer);
    cursor.move_word_left(buffer);
    let start_idx = cursor.char_index(buffer);

    if start_idx < end_idx {
        buffer.delete_range(start_idx, end_idx);
        let (line, col) = buffer.char_to_line_col(start_idx);
        cursor.set_position(line, col);
    }
    selection.clear();
}

pub fn delete_word_after(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    if delete_selection(buffer, cursor, selection, history) {
        return;
    }

    let last_line = buffer.line_count().saturating_sub(1);
    let last_col = buffer.line_len(last_line);
    if cursor.line() == last_line && cursor.col() == last_col {
        return;
    }

    history.push(buffer, cursor.position);

    let start_idx = cursor.char_index(buffer);
    // Temporarily move cursor to find word boundary
    let saved = cursor.position;
    cursor.move_word_right(buffer);
    let end_idx = cursor.char_index(buffer);
    cursor.set_position(saved.line, saved.col);

    if start_idx < end_idx {
        buffer.delete_range(start_idx, end_idx);
    }
    selection.clear();
}

/// Indent all lines in a multi-line selection by 2 spaces
pub fn indent_lines(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    let (start, end) = match selection.get_ordered_positions() {
        Some(p) => p,
        None => return,
    };
    history.push(buffer, cursor.position);
    // Insert from last line to first so char indices stay valid
    for line in (start.line..=end.line).rev() {
        let idx = buffer.line_col_to_char(line, 0);
        buffer.insert(idx, "  ");
    }
    // Adjust anchor and cursor cols
    if let Some(ref mut anchor) = selection.anchor {
        anchor.col += 2;
    }
    selection.cursor.col += 2;
    cursor.set_position(cursor.line(), cursor.col() + 2);
}

/// Dedent all lines in a multi-line selection by up to 2 spaces
pub fn dedent_lines(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    let (start, end) = match selection.get_ordered_positions() {
        Some(p) => p,
        None => return,
    };
    history.push(buffer, cursor.position);
    // Track removals for anchor and cursor lines
    let anchor_line = selection.anchor.map(|a| a.line);
    let cursor_line = selection.cursor.line;
    let mut anchor_removed = 0usize;
    let mut cursor_removed = 0usize;
    // Remove from last line to first so char indices stay valid
    for line in (start.line..=end.line).rev() {
        let content = buffer.get_line(line);
        let leading: usize = content.chars().take_while(|c| *c == ' ').count();
        if leading == 0 {
            continue;
        }
        let remove = leading.min(2);
        let line_start = buffer.line_col_to_char(line, 0);
        buffer.delete_range(line_start, line_start + remove);
        if Some(line) == anchor_line {
            anchor_removed = remove;
        }
        if line == cursor_line {
            cursor_removed = remove;
        }
    }
    if let Some(ref mut anchor) = selection.anchor {
        anchor.col = anchor.col.saturating_sub(anchor_removed);
    }
    selection.cursor.col = selection.cursor.col.saturating_sub(cursor_removed);
    let editor_cursor_removed = if cursor.line() == cursor_line {
        cursor_removed
    } else if Some(cursor.line()) == anchor_line {
        anchor_removed
    } else {
        0
    };
    cursor.set_position(
        cursor.line(),
        cursor.col().saturating_sub(editor_cursor_removed),
    );
}

/// Remove up to 2 leading spaces from the current line (Shift+Tab)
pub fn dedent(buffer: &mut TextBuffer, cursor: &mut Cursor, history: &mut History) {
    let line = buffer.get_line(cursor.line());
    let leading: usize = line.chars().take_while(|c| *c == ' ').count();
    if leading == 0 {
        return;
    }
    let remove = leading.min(2);
    history.push(buffer, cursor.position);
    let line_start = buffer.line_col_to_char(cursor.line(), 0);
    buffer.delete_range(line_start, line_start + remove);
    cursor.set_position(cursor.line(), cursor.col().saturating_sub(remove));
}

/// Delete character at cursor (delete key)
pub fn delete_at(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
) {
    if delete_selection(buffer, cursor, selection, history) {
        return;
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

    delete_selection(buffer, cursor, selection, history);

    (Some(text), clipboard_ok)
}

/// Paste text into the buffer at cursor position
pub fn paste(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
    text: &str,
) {
    if text.is_empty() {
        return;
    }

    if !delete_selection(buffer, cursor, selection, history) {
        history.push(buffer, cursor.position);
    }

    // Insert pasted text
    let char_idx = cursor.char_index(buffer);
    buffer.insert(char_idx, text);

    // Move cursor to end of pasted text
    let new_idx = char_idx + text.chars().count();
    let (new_line, new_col) = buffer.char_to_line_col(new_idx);
    cursor.set_position(new_line, new_col);

    selection.clear();
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

    #[test]
    fn test_indent_lines() {
        let mut buffer = TextBuffer::from_str("aaa\nbbb\nccc");
        let mut cursor = Cursor::new();
        let mut selection = Selection::new();
        let mut history = History::new();
        // Select lines 0-2
        selection.start(CursorPosition::new(0, 1));
        selection.cursor = CursorPosition::new(2, 1);
        cursor.set_position(2, 1);

        indent_lines(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.get_line(0), "  aaa");
        assert_eq!(buffer.get_line(1), "  bbb");
        assert_eq!(buffer.get_line(2), "  ccc");
        assert_eq!(cursor.col(), 3);
    }

    #[test]
    fn test_dedent_lines() {
        let mut buffer = TextBuffer::from_str("  aaa\n  bbb\n  ccc");
        let mut cursor = Cursor::new();
        let mut selection = Selection::new();
        let mut history = History::new();
        selection.start(CursorPosition::new(0, 2));
        selection.cursor = CursorPosition::new(2, 4);
        cursor.set_position(2, 4);

        dedent_lines(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.get_line(0), "aaa");
        assert_eq!(buffer.get_line(1), "bbb");
        assert_eq!(buffer.get_line(2), "ccc");
        assert_eq!(cursor.col(), 2);
    }

    #[test]
    fn test_dedent_lines_partial() {
        let mut buffer = TextBuffer::from_str(" aaa\n   bbb\nccc");
        let mut cursor = Cursor::new();
        let mut selection = Selection::new();
        let mut history = History::new();
        selection.start(CursorPosition::new(0, 0));
        selection.cursor = CursorPosition::new(2, 1);
        cursor.set_position(2, 1);

        dedent_lines(&mut buffer, &mut cursor, &mut selection, &mut history);

        assert_eq!(buffer.get_line(0), "aaa"); // 1 space removed
        assert_eq!(buffer.get_line(1), " bbb"); // 2 spaces removed
        assert_eq!(buffer.get_line(2), "ccc"); // 0 removed
        assert_eq!(cursor.col(), 1); // line 2 had 0 removed
    }

    #[test]
    fn test_indent_lines_undo() {
        let mut buffer = TextBuffer::from_str("aaa\nbbb\nccc");
        let mut cursor = Cursor::new();
        let mut selection = Selection::new();
        let mut history = History::new();
        selection.start(CursorPosition::new(0, 0));
        selection.cursor = CursorPosition::new(2, 0);
        cursor.set_position(2, 0);

        indent_lines(&mut buffer, &mut cursor, &mut selection, &mut history);
        assert_eq!(buffer.get_line(0), "  aaa");

        undo(&mut buffer, &mut cursor, &mut selection, &mut history);
        assert_eq!(buffer.get_line(0), "aaa");
        assert_eq!(buffer.get_line(1), "bbb");
        assert_eq!(buffer.get_line(2), "ccc");
    }
}
