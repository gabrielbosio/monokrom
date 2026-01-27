#![allow(dead_code)]

use crate::editor::buffer::TextBuffer;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CursorPosition {
    pub line: usize,
    pub col: usize,
}

impl CursorPosition {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    /// Clamp cursor position to valid buffer bounds
    pub fn clamp(&self, buffer: &TextBuffer) -> Self {
        let line = self.line.min(buffer.line_count().saturating_sub(1).max(0));
        let col = self.col.min(buffer.line_len(line));
        Self { line, col }
    }
}

pub struct Cursor {
    pub position: CursorPosition,
    /// Desired column when moving vertically (to handle lines shorter than current column)
    desired_col: usize,
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            position: CursorPosition::default(),
            desired_col: 0,
        }
    }

    pub fn line(&self) -> usize {
        self.position.line
    }

    pub fn col(&self) -> usize {
        self.position.col
    }

    pub fn set_position(&mut self, line: usize, col: usize) {
        self.position.line = line;
        self.position.col = col;
        self.desired_col = col;
    }

    /// Move cursor left
    pub fn move_left(&mut self, buffer: &TextBuffer) {
        if self.position.col > 0 {
            self.position.col -= 1;
        } else if self.position.line > 0 {
            // Move to end of previous line
            self.position.line -= 1;
            self.position.col = buffer.line_len(self.position.line);
        }
        self.desired_col = self.position.col;
    }

    /// Move cursor right
    pub fn move_right(&mut self, buffer: &TextBuffer) {
        let line_len = buffer.line_len(self.position.line);
        if self.position.col < line_len {
            self.position.col += 1;
        } else if self.position.line < buffer.line_count().saturating_sub(1) {
            // Move to start of next line
            self.position.line += 1;
            self.position.col = 0;
        }
        self.desired_col = self.position.col;
    }

    /// Move cursor up
    pub fn move_up(&mut self, buffer: &TextBuffer) {
        if self.position.line > 0 {
            self.position.line -= 1;
            // Try to maintain desired column, but clamp to line length
            let line_len = buffer.line_len(self.position.line);
            self.position.col = self.desired_col.min(line_len);
        }
    }

    /// Move cursor down
    pub fn move_down(&mut self, buffer: &TextBuffer) {
        if self.position.line < buffer.line_count().saturating_sub(1) {
            self.position.line += 1;
            // Try to maintain desired column, but clamp to line length
            let line_len = buffer.line_len(self.position.line);
            self.position.col = self.desired_col.min(line_len);
        }
    }

    /// Move cursor to start of line
    pub fn move_to_line_start(&mut self) {
        self.position.col = 0;
        self.desired_col = 0;
    }

    /// Move cursor to end of line
    pub fn move_to_line_end(&mut self, buffer: &TextBuffer) {
        self.position.col = buffer.line_len(self.position.line);
        self.desired_col = self.position.col;
    }

    /// Move cursor left by word
    pub fn move_word_left(&mut self, buffer: &TextBuffer) {
        if self.position.col == 0 {
            if self.position.line > 0 {
                self.position.line -= 1;
                self.position.col = buffer.line_len(self.position.line);
            }
        } else {
            let line = buffer.get_line(self.position.line);
            let chars: Vec<char> = line.chars().collect();
            let mut col = self.position.col;

            // Skip whitespace going backwards
            while col > 0 && chars.get(col - 1).is_some_and(|c| c.is_whitespace()) {
                col -= 1;
            }

            // Skip word characters going backwards
            while col > 0 && chars.get(col - 1).is_some_and(|c| !c.is_whitespace()) {
                col -= 1;
            }

            self.position.col = col;
        }
        self.desired_col = self.position.col;
    }

    /// Move cursor right by word
    pub fn move_word_right(&mut self, buffer: &TextBuffer) {
        let line_len = buffer.line_len(self.position.line);

        if self.position.col >= line_len {
            if self.position.line < buffer.line_count().saturating_sub(1) {
                self.position.line += 1;
                self.position.col = 0;
            }
        } else {
            let line = buffer.get_line(self.position.line);
            let chars: Vec<char> = line.chars().collect();
            let mut col = self.position.col;

            // Skip current word characters
            while col < chars.len() && !chars[col].is_whitespace() {
                col += 1;
            }

            // Skip whitespace
            while col < chars.len() && chars[col].is_whitespace() {
                col += 1;
            }

            self.position.col = col;
        }
        self.desired_col = self.position.col;
    }

    /// Get char index in buffer
    pub fn char_index(&self, buffer: &TextBuffer) -> usize {
        buffer.line_col_to_char(self.position.line, self.position.col)
    }
}
