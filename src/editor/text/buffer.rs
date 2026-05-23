use ropey::Rope;
use std::fmt;

#[derive(Clone)]
pub struct TextBuffer {
    rope: Rope,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TextBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.rope)
    }
}

impl TextBuffer {
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    /// Get the total number of lines
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Get the length of a specific line (excluding newline)
    pub fn line_len(&self, line_idx: usize) -> usize {
        if line_idx >= self.rope.len_lines() {
            return 0;
        }
        let line = self.rope.line(line_idx);
        let len = line.len_chars();
        // Remove trailing newline from count
        if len > 0 && line.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        }
    }

    /// Get a line as a string (without trailing newline)
    pub fn get_line(&self, line_idx: usize) -> String {
        if line_idx >= self.rope.len_lines() {
            return String::new();
        }
        let line = self.rope.line(line_idx);
        let mut s: String = line.chars().collect();
        // Remove trailing newline
        if s.ends_with('\n') {
            s.pop();
        }
        s
    }

    /// Get the maximum line width in the buffer
    pub fn max_line_width(&self) -> usize {
        (0..self.line_count())
            .map(|i| self.line_len(i))
            .max()
            .unwrap_or(0)
    }

    /// Convert line/column to char index
    pub fn line_col_to_char(&self, line: usize, col: usize) -> usize {
        if line >= self.rope.len_lines() {
            return self.rope.len_chars();
        }
        let line_start = self.rope.line_to_char(line);
        let line_len = self.line_len(line);
        line_start + col.min(line_len)
    }

    /// Convert char index to line/column
    pub fn char_to_line_col(&self, char_idx: usize) -> (usize, usize) {
        let char_idx = char_idx.min(self.rope.len_chars());
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        let col = char_idx - line_start;
        (line, col)
    }

    /// Insert text at the given char index
    pub fn insert(&mut self, char_idx: usize, text: &str) {
        let idx = char_idx.min(self.rope.len_chars());
        self.rope.insert(idx, text);
    }

    /// Delete a range of characters
    pub fn delete_range(&mut self, start_char: usize, end_char: usize) {
        let start = start_char.min(self.rope.len_chars());
        let end = end_char.min(self.rope.len_chars());
        if start < end {
            self.rope.remove(start..end);
        }
    }

    /// Delete character before the given position (backspace)
    pub fn delete_char_before(&mut self, line: usize, col: usize) -> bool {
        let char_idx = self.line_col_to_char(line, col);
        if char_idx > 0 {
            self.rope.remove(char_idx - 1..char_idx);
            true
        } else {
            false
        }
    }

    /// Delete character at the given position (delete key)
    pub fn delete_char_at(&mut self, line: usize, col: usize) -> bool {
        let char_idx = self.line_col_to_char(line, col);
        if char_idx < self.rope.len_chars() {
            self.rope.remove(char_idx..char_idx + 1);
            true
        } else {
            false
        }
    }

    /// Get text in a range
    pub fn get_range(&self, start_char: usize, end_char: usize) -> String {
        let start = start_char.min(self.rope.len_chars());
        let end = end_char.min(self.rope.len_chars());
        if start < end {
            self.rope.slice(start..end).to_string()
        } else {
            String::new()
        }
    }

    /// Get total character count
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Clone the underlying rope for undo snapshots
    pub fn snapshot(&self) -> Self {
        Self {
            rope: self.rope.clone(),
        }
    }

    /// Restore from a snapshot
    pub fn restore(&mut self, snapshot: &Self) {
        self.rope = snapshot.rope.clone();
    }
}

#[cfg(test)]
impl TextBuffer {
    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    pub fn clear(&mut self) {
        self.rope = Rope::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer_is_empty() {
        let buffer = TextBuffer::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len_chars(), 0);
        assert_eq!(buffer.line_count(), 1); // Empty buffer has 1 line
    }

    #[test]
    fn test_from_str_single_line() {
        let buffer = TextBuffer::from_str("hello");
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.line_len(0), 5);
        assert_eq!(buffer.get_line(0), "hello");
    }

    #[test]
    fn test_from_str_multi_line() {
        let buffer = TextBuffer::from_str("hello\nworld");
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.line_len(0), 5);
        assert_eq!(buffer.line_len(1), 5);
        assert_eq!(buffer.get_line(0), "hello");
        assert_eq!(buffer.get_line(1), "world");
    }

    #[test]
    fn test_from_str_with_trailing_newline() {
        let buffer = TextBuffer::from_str("hello\n");
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.line_len(0), 5);
        assert_eq!(buffer.line_len(1), 0);
    }

    #[test]
    fn test_line_len_out_of_bounds() {
        let buffer = TextBuffer::from_str("hello");
        assert_eq!(buffer.line_len(999), 0);
    }

    #[test]
    fn test_get_line_out_of_bounds() {
        let buffer = TextBuffer::from_str("hello");
        assert_eq!(buffer.get_line(999), "");
    }

    #[test]
    fn test_max_line_width() {
        let buffer = TextBuffer::from_str("hi\nhello\nworld!");
        assert_eq!(buffer.max_line_width(), 6); // "world!" is longest
    }

    #[test]
    fn test_insert_at_start() {
        let mut buffer = TextBuffer::from_str("world");
        buffer.insert(0, "hello ");
        assert_eq!(buffer.to_string(), "hello world");
    }

    #[test]
    fn test_insert_at_end() {
        let mut buffer = TextBuffer::from_str("hello");
        buffer.insert(5, " world");
        assert_eq!(buffer.to_string(), "hello world");
    }

    #[test]
    fn test_insert_newline() {
        let mut buffer = TextBuffer::from_str("helloworld");
        buffer.insert(5, "\n");
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.get_line(0), "hello");
        assert_eq!(buffer.get_line(1), "world");
    }

    #[test]
    fn test_delete_range() {
        let mut buffer = TextBuffer::from_str("hello world");
        buffer.delete_range(5, 11); // delete " world"
        assert_eq!(buffer.to_string(), "hello");
    }

    #[test]
    fn test_delete_range_empty() {
        let mut buffer = TextBuffer::from_str("hello");
        buffer.delete_range(2, 2); // empty range
        assert_eq!(buffer.to_string(), "hello");
    }

    #[test]
    fn test_delete_char_before() {
        let mut buffer = TextBuffer::from_str("hello");
        assert!(buffer.delete_char_before(0, 3)); // delete 'l'
        assert_eq!(buffer.to_string(), "helo");
    }

    #[test]
    fn test_delete_char_before_at_start() {
        let mut buffer = TextBuffer::from_str("hello");
        assert!(!buffer.delete_char_before(0, 0)); // nothing to delete
        assert_eq!(buffer.to_string(), "hello");
    }

    #[test]
    fn test_delete_char_at() {
        let mut buffer = TextBuffer::from_str("hello");
        assert!(buffer.delete_char_at(0, 2)); // delete 'l'
        assert_eq!(buffer.to_string(), "helo");
    }

    #[test]
    fn test_delete_char_at_end() {
        let mut buffer = TextBuffer::from_str("hello");
        assert!(!buffer.delete_char_at(0, 5)); // nothing to delete
        assert_eq!(buffer.to_string(), "hello");
    }

    #[test]
    fn test_line_col_to_char() {
        let buffer = TextBuffer::from_str("hello\nworld");
        assert_eq!(buffer.line_col_to_char(0, 0), 0);
        assert_eq!(buffer.line_col_to_char(0, 5), 5);
        assert_eq!(buffer.line_col_to_char(1, 0), 6);
        assert_eq!(buffer.line_col_to_char(1, 3), 9);
    }

    #[test]
    fn test_char_to_line_col() {
        let buffer = TextBuffer::from_str("hello\nworld");
        assert_eq!(buffer.char_to_line_col(0), (0, 0));
        assert_eq!(buffer.char_to_line_col(5), (0, 5));
        assert_eq!(buffer.char_to_line_col(6), (1, 0));
        assert_eq!(buffer.char_to_line_col(9), (1, 3));
    }

    #[test]
    fn test_get_range() {
        let buffer = TextBuffer::from_str("hello world");
        assert_eq!(buffer.get_range(0, 5), "hello");
        assert_eq!(buffer.get_range(6, 11), "world");
        assert_eq!(buffer.get_range(0, 11), "hello world");
    }

    #[test]
    fn test_snapshot_and_restore() {
        let mut buffer = TextBuffer::from_str("hello");
        let snapshot = buffer.snapshot();
        buffer.insert(5, " world");
        assert_eq!(buffer.to_string(), "hello world");
        buffer.restore(&snapshot);
        assert_eq!(buffer.to_string(), "hello");
    }

    #[test]
    fn test_clear() {
        let mut buffer = TextBuffer::from_str("hello world");
        buffer.clear();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len_chars(), 0);
    }
}
