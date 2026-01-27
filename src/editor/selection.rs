#![allow(dead_code)]

use crate::editor::buffer::TextBuffer;
use crate::editor::cursor::CursorPosition;

#[derive(Clone, Copy, Debug, Default)]
pub struct Selection {
    /// Anchor point where selection started
    pub anchor: Option<CursorPosition>,
    /// Current cursor position (selection extends from anchor to cursor)
    pub cursor: CursorPosition,
}

impl Selection {
    pub fn new() -> Self {
        Self {
            anchor: None,
            cursor: CursorPosition::default(),
        }
    }

    /// Check if there is an active selection
    pub fn is_active(&self) -> bool {
        self.anchor.is_some()
    }

    /// Start a new selection at the current cursor position
    pub fn start(&mut self, cursor: CursorPosition) {
        self.anchor = Some(cursor);
        self.cursor = cursor;
    }

    /// Extend selection to the given cursor position
    pub fn extend_to(&mut self, cursor: CursorPosition) {
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        self.cursor = cursor;
    }

    /// Clear the selection
    pub fn clear(&mut self) {
        self.anchor = None;
    }

    /// Clear the selection and sync cursor position
    pub fn clear_and_sync(&mut self, cursor: CursorPosition) {
        self.anchor = None;
        self.cursor = cursor;
    }

    /// Get the ordered start and end of selection as char indices
    pub fn get_range(&self, buffer: &TextBuffer) -> Option<(usize, usize)> {
        let anchor = self.anchor?;

        let anchor_idx = buffer.line_col_to_char(anchor.line, anchor.col);
        let cursor_idx = buffer.line_col_to_char(self.cursor.line, self.cursor.col);

        if anchor_idx <= cursor_idx {
            Some((anchor_idx, cursor_idx))
        } else {
            Some((cursor_idx, anchor_idx))
        }
    }

    /// Get the ordered start and end positions
    pub fn get_ordered_positions(&self) -> Option<(CursorPosition, CursorPosition)> {
        let anchor = self.anchor?;

        if anchor.line < self.cursor.line
            || (anchor.line == self.cursor.line && anchor.col <= self.cursor.col)
        {
            Some((anchor, self.cursor))
        } else {
            Some((self.cursor, anchor))
        }
    }

    /// Get selected text from buffer
    pub fn get_text(&self, buffer: &TextBuffer) -> Option<String> {
        let (start, end) = self.get_range(buffer)?;
        if start == end {
            return None;
        }
        Some(buffer.get_range(start, end))
    }

    /// Check if a given line/col is within the selection
    pub fn contains(&self, line: usize, col: usize, buffer: &TextBuffer) -> bool {
        if let Some((start, end)) = self.get_range(buffer) {
            let idx = buffer.line_col_to_char(line, col);
            idx >= start && idx < end
        } else {
            false
        }
    }

    /// Get selection info for a specific line (start_col, end_col within the line)
    pub fn get_line_selection(&self, line: usize, buffer: &TextBuffer) -> Option<(usize, usize)> {
        let (start_pos, end_pos) = self.get_ordered_positions()?;

        // Check if line is within selection range
        if line < start_pos.line || line > end_pos.line {
            return None;
        }

        let line_len = buffer.line_len(line);

        let start_col = if line == start_pos.line {
            start_pos.col
        } else {
            0
        };

        let end_col = if line == end_pos.line {
            end_pos.col
        } else {
            line_len
        };

        if start_col == end_col {
            return None;
        }

        Some((start_col, end_col))
    }
}
