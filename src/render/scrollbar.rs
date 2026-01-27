use macroquad::prelude::*;

use crate::config::*;

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

pub fn draw_scrollbars(state: &ScrollbarState) {
    let track_color = Color::new(0.3, 0.3, 0.3, 1.0);
    let handle_color = COLOR_WHITE;

    // Vertical scrollbar (right edge)
    if state.vertical_visible {
        let track_x = (SCREEN_WIDTH - SCROLLBAR_WIDTH) as f32;
        let track_y = 0.0;
        let track_height = if state.horizontal_visible {
            SCREEN_HEIGHT - SCROLLBAR_WIDTH
        } else {
            SCREEN_HEIGHT
        } as f32;

        // Draw track
        draw_rectangle(track_x, track_y, SCROLLBAR_WIDTH as f32, track_height, track_color);

        // Draw handle
        let handle_height = (track_height * state.vertical_size).max(TILE_HEIGHT as f32);
        let handle_y = track_y + (track_height - handle_height) * state.vertical_position;
        draw_rectangle(track_x, handle_y, SCROLLBAR_WIDTH as f32, handle_height, handle_color);
    }

    // Horizontal scrollbar (bottom edge)
    if state.horizontal_visible {
        let track_x = 0.0;
        let track_y = (SCREEN_HEIGHT - SCROLLBAR_WIDTH) as f32;
        let track_width = if state.vertical_visible {
            SCREEN_WIDTH - SCROLLBAR_WIDTH
        } else {
            SCREEN_WIDTH
        } as f32;

        // Draw track
        draw_rectangle(track_x, track_y, track_width, SCROLLBAR_WIDTH as f32, track_color);

        // Draw handle
        let handle_width = (track_width * state.horizontal_size).max(TILE_WIDTH as f32);
        let handle_x = track_x + (track_width - handle_width) * state.horizontal_position;
        draw_rectangle(handle_x, track_y, handle_width, SCROLLBAR_WIDTH as f32, handle_color);
    }

    // Corner square when both are visible
    if state.vertical_visible && state.horizontal_visible {
        let corner_x = (SCREEN_WIDTH - SCROLLBAR_WIDTH) as f32;
        let corner_y = (SCREEN_HEIGHT - SCROLLBAR_WIDTH) as f32;
        draw_rectangle(corner_x, corner_y, SCROLLBAR_WIDTH as f32, SCROLLBAR_WIDTH as f32, track_color);
    }
}
