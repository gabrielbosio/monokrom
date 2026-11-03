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

// Music
pub const MUSIC_ROWS: usize = 32;
pub const MUSIC_CHANNELS: usize = 4;

// Audio
pub const AUDIO_WAVE_COUNT: usize = 16;
pub const AUDIO_WAVE_SIZE: usize = 16; // 32 samples of 4 bits
pub const AUDIO_WAVE_REGION_START: usize = 0x5000;
pub const AUDIO_WAVE_REGION_SIZE: usize = AUDIO_WAVE_COUNT * AUDIO_WAVE_SIZE;

pub const AUDIO_INSTRUMENT_COUNT: usize = 32;
pub const AUDIO_INSTRUMENT_SIZE: usize = 16;
pub const AUDIO_INSTRUMENT_REGION_START: usize = AUDIO_WAVE_REGION_START + AUDIO_WAVE_REGION_SIZE;
pub const AUDIO_INSTRUMENT_REGION_SIZE: usize = AUDIO_INSTRUMENT_COUNT * AUDIO_INSTRUMENT_SIZE;

pub const AUDIO_TABLE_COUNT: usize = 16;
pub const AUDIO_TABLE_STEPS: usize = 32;
pub const AUDIO_TABLE_STEP_SIZE: usize = 3;
pub const AUDIO_TABLE_HEADER_BYTES: usize = 4;
pub const AUDIO_TABLE_SIZE: usize =
    AUDIO_TABLE_HEADER_BYTES + AUDIO_TABLE_STEPS * AUDIO_TABLE_STEP_SIZE;
pub const AUDIO_TABLE_REGION_START: usize =
    AUDIO_INSTRUMENT_REGION_START + AUDIO_INSTRUMENT_REGION_SIZE;
pub const AUDIO_TABLE_REGION_SIZE: usize = AUDIO_TABLE_COUNT * AUDIO_TABLE_SIZE;

pub const AUDIO_SFX_COUNT: usize = 16;
pub const AUDIO_SFX_SIZE: usize = 6;
pub const AUDIO_SFX_REGION_START: usize = AUDIO_TABLE_REGION_START + AUDIO_TABLE_REGION_SIZE;

pub const AUDIO_REGION_START: usize = AUDIO_WAVE_REGION_START;
pub const AUDIO_REGION_END: usize = 0x6000;
pub const AUDIO_REGION_SIZE: usize = AUDIO_REGION_END - AUDIO_REGION_START;
pub const AUDIO_INSTRUMENT_OFFSET: usize = AUDIO_INSTRUMENT_REGION_START - AUDIO_REGION_START;
pub const AUDIO_TABLE_OFFSET: usize = AUDIO_TABLE_REGION_START - AUDIO_REGION_START;
pub const AUDIO_SFX_OFFSET: usize = AUDIO_SFX_REGION_START - AUDIO_REGION_START;

pub const SONG_REGION_START: usize = 0x6000;
pub const SONG_REGION_SIZE: usize = 0x2000;
pub const SONG_PATTERN_COUNT: usize = 16;
pub const SONG_CELL_SIZE: usize = 3;
pub const SONG_PATTERN_SIZE: usize = MUSIC_ROWS * SONG_CELL_SIZE;
pub const SONG_PATTERN_REGION_SIZE: usize = MUSIC_CHANNELS * SONG_PATTERN_COUNT * SONG_PATTERN_SIZE;
pub const SONG_ORDER_COUNT: usize = 64;
pub const SONG_ORDER_SIZE: usize = MUSIC_CHANNELS;
pub const SONG_ORDER_REGION_SIZE: usize = SONG_ORDER_COUNT * SONG_ORDER_SIZE;
pub const SONG_COUNT: usize = 16;
pub const SONG_SIZE: usize = 3;
pub const SONG_TABLE_SIZE: usize = SONG_COUNT * SONG_SIZE;
pub const SONG_HEADER_BYTES: usize = 16;
pub const SONG_HEADER_START: usize =
    SONG_REGION_START + SONG_PATTERN_REGION_SIZE + SONG_ORDER_REGION_SIZE;
pub const SONG_TABLE_START: usize = SONG_HEADER_START + SONG_HEADER_BYTES;

pub const SONG_LOOP_ENABLED: u8 = 0x80;
pub const SONG_ORDER_MASK: u8 = 0x3F;
pub const SONG_END_MASK: u8 = 0x7F;
