use crate::editor::buffer::TextBuffer;
use crate::editor::cursor::Cursor;
use crate::editor::history::History;
use crate::editor::selection::Selection;

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
pub fn copy_selection(
    buffer: &TextBuffer,
    selection: &Selection,
    clipboard: &mut Option<arboard::Clipboard>,
) -> Option<String> {
    let text = selection.get_text(buffer)?;
    if let Some(cb) = clipboard.as_mut() {
        let _ = cb.set_text(&text);
    }
    Some(text)
}

/// Cut selected text to clipboard
pub fn cut_selection(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
    clipboard: &mut Option<arboard::Clipboard>,
) -> Option<String> {
    let text = selection.get_text(buffer)?;

    // Copy to clipboard
    if let Some(cb) = clipboard.as_mut() {
        let _ = cb.set_text(&text);
    }

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

    Some(text)
}

/// Paste from clipboard
pub fn paste(
    buffer: &mut TextBuffer,
    cursor: &mut Cursor,
    selection: &mut Selection,
    history: &mut History,
    clipboard: &mut Option<arboard::Clipboard>,
) {
    let text = clipboard.as_mut().and_then(|cb| cb.get_text().ok());
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
