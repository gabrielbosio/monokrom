use macroquad::prelude::Color;

// Tile dimensions
pub const TILE_WIDTH: u32 = 4;
pub const TILE_HEIGHT: u32 = 6;

// Screen dimensions in tiles
pub const SCREEN_TILES_X: u32 = 40;
pub const SCREEN_TILES_Y: u32 = 24;

// Screen dimensions in pixels (native resolution)
pub const SCREEN_WIDTH: u32 = SCREEN_TILES_X * TILE_WIDTH; // 160
pub const SCREEN_HEIGHT: u32 = SCREEN_TILES_Y * TILE_HEIGHT; // 144

// Window scale factor
pub const SCALE: u32 = 4;

// Window dimensions
pub const WINDOW_WIDTH: u32 = SCREEN_WIDTH * SCALE; // 640
pub const WINDOW_HEIGHT: u32 = SCREEN_HEIGHT * SCALE; // 576

// Color palette (4-color monochrome)
pub const COLOR_BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
pub const COLOR_DARK_GRAY: Color = Color::new(1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0, 1.0);
pub const COLOR_LIGHT_GRAY: Color = Color::new(2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 1.0);
pub const COLOR_WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);

// Cursor blink rate (in seconds)
pub const CURSOR_BLINK_RATE: f64 = 0.5;

// Scrollbar dimensions
pub const SCROLLBAR_WIDTH: u32 = 4;

// Editor area (accounting for potential scrollbars)
pub const EDITOR_TILES_X: u32 = SCREEN_TILES_X - 1; // 39 tiles for text, 1 for scrollbar
pub const EDITOR_TILES_Y: u32 = SCREEN_TILES_Y - 1; // 23 tiles for text, 1 for scrollbar

// Terminal
pub const TERMINAL_MAX_SCROLLBACK: usize = 200;
