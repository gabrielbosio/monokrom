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

// Audio
pub const MUSIC_CHANNELS: usize = 4;

pub const WAVE_TABLE_COUNT: usize = 16;
pub const WAVE_TABLE_SIZE: usize = 16; // 32 samples of 4 bits
pub const WAVE_TABLE_REGION_START: usize = 0x5000;
pub const WAVE_TABLE_REGION_SIZE: usize = WAVE_TABLE_COUNT * WAVE_TABLE_SIZE; // 256

pub const MAX_SONG_COUNT: usize = 256;
pub const SONG_LENGTH_TABLE_START: usize = WAVE_TABLE_REGION_START + WAVE_TABLE_REGION_SIZE;
pub const SONG_LENGTH_TABLE_SIZE: usize = MAX_SONG_COUNT; // 256

pub const EVENT_SIZE: usize = 3;
pub const FRAME_SIZE: usize = MUSIC_CHANNELS * EVENT_SIZE; // 12
pub const FRAME_POOL_START: usize = SONG_LENGTH_TABLE_START + SONG_LENGTH_TABLE_SIZE;
pub const FRAME_POOL_END: usize = 0x7FE0;
pub const FRAME_POOL_SIZE: usize = FRAME_POOL_END - FRAME_POOL_START; // 11744
pub const MAX_TOTAL_FRAMES: usize = 978;

pub const WAVE_RAM_START: usize = FRAME_POOL_END;
pub const WAVE_RAM_SIZE: usize = 16;

pub const APU_REGION_START: usize = 0x7FF0;
pub const APU_REGION_SIZE: usize = 16;
pub const APU_MASTER_VOLUME: usize = APU_REGION_START;
pub const APU_PANNING: usize = APU_REGION_START + 1;

pub const SFX_REGION_START: usize = SONG_LENGTH_TABLE_START;
pub const SFX_REGION_SIZE: usize = SONG_LENGTH_TABLE_SIZE + FRAME_POOL_SIZE; // 12000
pub const UI_SECTION_SIZE: usize = MAX_SONG_COUNT; // 256

pub const DEFAULT_PREVIEW_SPEED: u8 = 60;

const _: () = assert!(FRAME_POOL_SIZE == 11744);
const _: () = assert!(MAX_TOTAL_FRAMES * FRAME_SIZE <= FRAME_POOL_SIZE);
const _: () = assert!(SFX_REGION_SIZE == 12000);
const _: () = assert!(APU_REGION_START + APU_REGION_SIZE == 0x8000);
