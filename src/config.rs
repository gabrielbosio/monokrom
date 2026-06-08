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

// Sprites
pub const SPRITE_COUNT: usize = 256;
pub const SPRITE_SIZE: usize = 16; // bytes per sprite (8x8, 2bpp)
pub const SPRITE_REGION_START: usize = 0x3000; // 12288

// Map
pub const MAP_WIDTH: usize = 128;
pub const MAP_HEIGHT: usize = 32;
pub const MAP_REGION_START: usize = 0x4000; // 16384

// SFX
pub const SFX_COUNT: usize = 16;
pub const SFX_CELLS: usize = 32;
pub const SFX_HEADER_BYTES: usize = 4;
pub const SFX_SIZE: usize = SFX_HEADER_BYTES + SFX_CELLS * 2; // 68
pub const SFX_REGION_START: usize = 0x5000; // 20480
pub const SFX_REGION_SIZE: usize = SFX_COUNT * SFX_SIZE; // 1088
