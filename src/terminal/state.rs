use crate::config::TERMINAL_MAX_SCROLLBACK;

pub struct TerminalState {
    pub output_lines: Vec<String>,
    pub input_line: String,
    pub cursor_pos: usize,
    pub command_history: Vec<String>,
    pub history_index: Option<usize>,
    pub scroll_offset: usize,
    pub last_tab_input: Option<String>,
}

impl TerminalState {
    pub fn new() -> Self {
        Self {
            output_lines: Vec::new(),
            input_line: String::new(),
            cursor_pos: 0,
            command_history: Vec::new(),
            history_index: None,
            scroll_offset: 0,
            last_tab_input: None,
        }
    }

    pub fn push_output(&mut self, line: &str) {
        self.output_lines.push(line.to_string());
        if self.output_lines.len() > TERMINAL_MAX_SCROLLBACK {
            self.output_lines.remove(0);
        }
        self.scroll_to_bottom();
    }

    pub fn clear(&mut self) {
        self.output_lines.clear();
        self.scroll_offset = 0;
    }

    pub fn submit_input(&mut self) -> String {
        let input = self.input_line.clone();
        if !input.is_empty() {
            self.command_history.push(input.clone());
        }
        self.input_line.clear();
        self.cursor_pos = 0;
        self.history_index = None;
        self.last_tab_input = None;
        input
    }

    pub fn recall_prev(&mut self) {
        if self.command_history.is_empty() {
            return;
        }
        let idx = match self.history_index {
            Some(0) => return,
            Some(i) => i - 1,
            None => self.command_history.len() - 1,
        };
        self.history_index = Some(idx);
        self.input_line = self.command_history[idx].clone();
        self.cursor_pos = self.input_line.len();
        self.last_tab_input = None;
    }

    pub fn recall_next(&mut self) {
        let idx = match self.history_index {
            Some(i) => i + 1,
            None => return,
        };
        if idx >= self.command_history.len() {
            self.history_index = None;
            self.input_line.clear();
            self.cursor_pos = 0;
        } else {
            self.history_index = Some(idx);
            self.input_line = self.command_history[idx].clone();
            self.cursor_pos = self.input_line.len();
        }
        self.last_tab_input = None;
    }

    pub fn insert_char(&mut self, c: char) {
        self.input_line.insert(self.cursor_pos, c);
        self.cursor_pos += 1;
        self.last_tab_input = None;
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.input_line.remove(self.cursor_pos);
            self.last_tab_input = None;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor_pos < self.input_line.len() {
            self.input_line.remove(self.cursor_pos);
            self.last_tab_input = None;
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_pos < self.input_line.len() {
            self.cursor_pos += 1;
        }
    }

    pub fn move_to_start(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_to_end(&mut self) {
        self.cursor_pos = self.input_line.len();
    }

    pub fn scroll_to_bottom(&mut self) {
        let visible_lines = crate::config::SCREEN_TILES_Y as usize - 1; // reserve 1 row for input
        if self.output_lines.len() > visible_lines {
            self.scroll_offset = self.output_lines.len() - visible_lines;
        } else {
            self.scroll_offset = 0;
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        let visible_lines = crate::config::SCREEN_TILES_Y as usize - 1;
        let max_offset = self.output_lines.len().saturating_sub(visible_lines);
        if self.scroll_offset < max_offset {
            self.scroll_offset += 1;
        }
    }
}
