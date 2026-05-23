use super::buffer::TextBuffer;
use super::cursor::CursorPosition;

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

    /// Start a new selection at the current cursor position
    pub fn start(&mut self, cursor: CursorPosition) {
        self.anchor = Some(cursor);
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

#[cfg(test)]
impl Selection {
    pub fn is_active(&self) -> bool {
        self.anchor.is_some()
    }

    pub fn extend_to(&mut self, cursor: CursorPosition) {
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        self.cursor = cursor;
    }

    pub fn contains(&self, line: usize, col: usize, buffer: &TextBuffer) -> bool {
        if let Some((start, end)) = self.get_range(buffer) {
            let idx = buffer.line_col_to_char(line, col);
            idx >= start && idx < end
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_new() {
        let sel = Selection::new();
        assert!(!sel.is_active());
        assert!(sel.anchor.is_none());
    }

    #[test]
    fn test_selection_start() {
        let mut sel = Selection::new();
        let pos = CursorPosition::new(1, 5);
        sel.start(pos);
        assert!(sel.is_active());
        assert_eq!(sel.anchor, Some(pos));
        assert_eq!(sel.cursor, pos);
    }

    #[test]
    fn test_selection_extend_to() {
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(0, 0));
        sel.extend_to(CursorPosition::new(0, 5));
        assert_eq!(sel.anchor, Some(CursorPosition::new(0, 0)));
        assert_eq!(sel.cursor, CursorPosition::new(0, 5));
    }

    #[test]
    fn test_selection_extend_to_without_anchor() {
        let mut sel = Selection::new();
        sel.cursor = CursorPosition::new(0, 3);
        sel.extend_to(CursorPosition::new(0, 5));
        // anchor should be set from current cursor
        assert_eq!(sel.anchor, Some(CursorPosition::new(0, 3)));
        assert_eq!(sel.cursor, CursorPosition::new(0, 5));
    }

    #[test]
    fn test_selection_clear() {
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(1, 5));
        sel.clear();
        assert!(!sel.is_active());
        assert!(sel.anchor.is_none());
    }

    #[test]
    fn test_selection_clear_and_sync() {
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(1, 5));
        sel.clear_and_sync(CursorPosition::new(2, 3));
        assert!(!sel.is_active());
        assert_eq!(sel.cursor, CursorPosition::new(2, 3));
    }

    #[test]
    fn test_get_range_forward() {
        let buffer = TextBuffer::from_str("hello world");
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(0, 0));
        sel.cursor = CursorPosition::new(0, 5);
        let range = sel.get_range(&buffer);
        assert_eq!(range, Some((0, 5)));
    }

    #[test]
    fn test_get_range_backward() {
        let buffer = TextBuffer::from_str("hello world");
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(0, 5));
        sel.cursor = CursorPosition::new(0, 0);
        let range = sel.get_range(&buffer);
        assert_eq!(range, Some((0, 5))); // should be ordered
    }

    #[test]
    fn test_get_range_no_selection() {
        let buffer = TextBuffer::from_str("hello");
        let sel = Selection::new();
        assert!(sel.get_range(&buffer).is_none());
    }

    #[test]
    fn test_get_text() {
        let buffer = TextBuffer::from_str("hello world");
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(0, 0));
        sel.cursor = CursorPosition::new(0, 5);
        let text = sel.get_text(&buffer);
        assert_eq!(text, Some("hello".to_string()));
    }

    #[test]
    fn test_get_text_multiline() {
        let buffer = TextBuffer::from_str("hello\nworld");
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(0, 3));
        sel.cursor = CursorPosition::new(1, 3);
        let text = sel.get_text(&buffer);
        assert_eq!(text, Some("lo\nwor".to_string()));
    }

    #[test]
    fn test_get_text_same_position() {
        let buffer = TextBuffer::from_str("hello");
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(0, 2));
        sel.cursor = CursorPosition::new(0, 2);
        assert!(sel.get_text(&buffer).is_none());
    }

    #[test]
    fn test_contains() {
        let buffer = TextBuffer::from_str("hello world");
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(0, 2));
        sel.cursor = CursorPosition::new(0, 7);
        assert!(sel.contains(0, 3, &buffer));
        assert!(sel.contains(0, 5, &buffer));
        assert!(!sel.contains(0, 0, &buffer));
        assert!(!sel.contains(0, 8, &buffer));
    }

    #[test]
    fn test_get_line_selection_single_line() {
        let buffer = TextBuffer::from_str("hello world");
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(0, 2));
        sel.cursor = CursorPosition::new(0, 7);
        let line_sel = sel.get_line_selection(0, &buffer);
        assert_eq!(line_sel, Some((2, 7)));
    }

    #[test]
    fn test_get_line_selection_multiline() {
        let buffer = TextBuffer::from_str("hello\nworld\ntest");
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(0, 3));
        sel.cursor = CursorPosition::new(2, 2);

        // First line: from col 3 to end
        let line0 = sel.get_line_selection(0, &buffer);
        assert_eq!(line0, Some((3, 5)));

        // Middle line: entire line
        let line1 = sel.get_line_selection(1, &buffer);
        assert_eq!(line1, Some((0, 5)));

        // Last line: from start to col 2
        let line2 = sel.get_line_selection(2, &buffer);
        assert_eq!(line2, Some((0, 2)));
    }

    #[test]
    fn test_get_line_selection_outside_range() {
        let buffer = TextBuffer::from_str("hello\nworld");
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(0, 0));
        sel.cursor = CursorPosition::new(0, 3);
        assert!(sel.get_line_selection(1, &buffer).is_none());
    }

    #[test]
    fn test_get_ordered_positions() {
        let mut sel = Selection::new();
        sel.start(CursorPosition::new(1, 5));
        sel.cursor = CursorPosition::new(0, 2);
        let (start, end) = sel.get_ordered_positions().unwrap();
        assert_eq!(start, CursorPosition::new(0, 2));
        assert_eq!(end, CursorPosition::new(1, 5));
    }
}
