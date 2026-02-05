use macroquad::prelude::Color;

// Tile dimensions
pub const TILE_WIDTH: u32 = 8;
pub const TILE_HEIGHT: u32 = 8;

// Screen dimensions in tiles
pub const SCREEN_TILES_X: u32 = 32;
pub const SCREEN_TILES_Y: u32 = 30;

// Screen dimensions in pixels (native resolution)
pub const SCREEN_WIDTH: u32 = SCREEN_TILES_X * TILE_WIDTH; // 256
pub const SCREEN_HEIGHT: u32 = SCREEN_TILES_Y * TILE_HEIGHT; // 240

// Window scale factor
pub const SCALE: u32 = 3;

// Window dimensions
pub const WINDOW_WIDTH: u32 = SCREEN_WIDTH * SCALE; // 768
pub const WINDOW_HEIGHT: u32 = SCREEN_HEIGHT * SCALE; // 720

// Color palette
pub const COLOR_BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
pub const COLOR_WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
pub const COLOR_GRAY: Color = Color::new(0.5, 0.5, 0.5, 1.0);

// Cursor blink rate (in seconds)
pub const CURSOR_BLINK_RATE: f64 = 0.5;

// Scrollbar dimensions
pub const SCROLLBAR_WIDTH: u32 = 8;

// Editor area (accounting for potential scrollbars)
pub const EDITOR_TILES_X: u32 = SCREEN_TILES_X - 1; // 31 tiles for text, 1 for scrollbar
pub const EDITOR_TILES_Y: u32 = SCREEN_TILES_Y - 1; // 29 tiles for text, 1 for scrollbar
