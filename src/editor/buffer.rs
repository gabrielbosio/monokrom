#![allow(dead_code)]

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

    /// Insert a character at line/column position
    pub fn insert_at(&mut self, line: usize, col: usize, text: &str) {
        let char_idx = self.line_col_to_char(line, col);
        self.insert(char_idx, text);
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

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// Get total character count
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.rope = Rope::new();
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
