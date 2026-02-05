pub struct ScrollbarState {
    pub vertical_visible: bool,
    pub horizontal_visible: bool,
    pub vertical_position: f32,   // 0.0 to 1.0
    pub horizontal_position: f32, // 0.0 to 1.0
    pub vertical_size: f32,       // 0.0 to 1.0 (size of handle relative to track)
    pub horizontal_size: f32,     // 0.0 to 1.0
}

impl Default for ScrollbarState {
    fn default() -> Self {
        Self {
            vertical_visible: false,
            horizontal_visible: false,
            vertical_position: 0.0,
            horizontal_position: 0.0,
            vertical_size: 1.0,
            horizontal_size: 1.0,
        }
    }
}

impl ScrollbarState {
    pub fn update(
        &mut self,
        total_lines: usize,
        visible_lines: usize,
        scroll_y: usize,
        max_line_width: usize,
        visible_cols: usize,
        scroll_x: usize,
    ) {
        // Vertical scrollbar
        if total_lines > visible_lines {
            self.vertical_visible = true;
            self.vertical_size = (visible_lines as f32) / (total_lines as f32);
            self.vertical_size = self.vertical_size.clamp(0.1, 1.0);

            let max_scroll = total_lines.saturating_sub(visible_lines);
            self.vertical_position = if max_scroll > 0 {
                (scroll_y as f32) / (max_scroll as f32)
            } else {
                0.0
            };
        } else {
            self.vertical_visible = false;
        }

        // Horizontal scrollbar
        if max_line_width > visible_cols {
            self.horizontal_visible = true;
            self.horizontal_size = (visible_cols as f32) / (max_line_width as f32);
            self.horizontal_size = self.horizontal_size.clamp(0.1, 1.0);

            let max_scroll = max_line_width.saturating_sub(visible_cols);
            self.horizontal_position = if max_scroll > 0 {
                (scroll_x as f32) / (max_scroll as f32)
            } else {
                0.0
            };
        } else {
            self.horizontal_visible = false;
        }
    }
}
