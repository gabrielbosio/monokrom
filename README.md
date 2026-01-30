# Monokrom

A fantasy console text editor built in Rust using Macroquad.

## Overview

Monokrom is a fantasy console inspired by Pico-8 and TIC-80. Unlike those systems which limit source code by character count, Monokrom uses a high-level language compiled to bytecode, so the memory limit depends on bytecode size rather than source file length.

Currently in **Version 1**, Monokrom functions as a retro-styled plain text editor with an 8x8 pixel tile-based display.

## Features

- NES-style bitmap font rendering
- Full text editing with cursor navigation
- Text selection (Shift+Arrow keys)
- Word navigation (Option+Left/Right)
- Line swapping (Option+Up/Down)
- Copy, cut, paste (Cmd+C/X/V)
- Undo/redo (Cmd+Z/Y)
- Find and replace (Cmd+F/R)
- File operations: save, open, new (Cmd+S/O/N)
- Automatic scrollbars for large documents

## Specifications

- **Tile size:** 8x8 pixels
- **Screen size:** 32x30 tiles (256x240 pixels, scaled 3x)
- **Color palette:**
  - Black (0)
  - White (1)
  - Gray (2)

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Cmd+S | Save file |
| Cmd+O | Open file |
| Cmd+N | New file |
| Cmd+Z | Undo |
| Cmd+Y | Redo |
| Cmd+X | Cut |
| Cmd+C | Copy |
| Cmd+V | Paste |
| Cmd+F | Find |
| Cmd+R | Replace |
| Cmd+A | Select all |
| Cmd+Arrow | Scroll viewport |
| Shift+Arrow | Extend selection |
| Option+Left/Right | Move by word |
| Option+Up/Down | Swap lines |

### File Picker Shortcuts

| Shortcut | Action |
|----------|--------|
| Enter | Open selected file |
| Cmd+D | Duplicate file |
| Cmd+Delete | Delete file |
| Escape | Cancel |

## Building

### Prerequisites

- Rust (stable)
- On Linux: `libasound2-dev`, `libxcb-shape0-dev`, `libxcb-xfixes0-dev`

### Build & Run

```bash
# Debug build (files stored in ./fs/)
cargo run

# Release build (files stored in ~/.monokrom/)
cargo build --release
./target/release/monokrom
```

### Run Tests

```bash
cargo test
```

### Run Linter

```bash
cargo clippy -- -D warnings
```

## File Storage

- **Debug mode:** Files are stored in `./fs/` relative to the binary
- **Release mode:** Files are stored in `~/.monokrom/`

## Project Structure

```
src/
├── main.rs           # Entry point
├── app.rs            # Application state and main loop
├── config.rs         # Constants (colors, dimensions)
├── editor/           # Text editing logic
│   ├── buffer.rs     # Text buffer (rope-based)
│   ├── cursor.rs     # Cursor positioning
│   ├── selection.rs  # Text selection
│   ├── history.rs    # Undo/redo
│   └── operations.rs # Edit operations
├── filesystem/       # File I/O
│   └── storage.rs    # Read/write/list files
├── input/            # Keyboard handling
│   └── mod.rs        # Key mappings
├── render/           # Graphics
│   ├── font.rs       # Bitmap font
│   ├── scrollbar.rs  # Scrollbar rendering
│   └── draw_helpers.rs # Scaled drawing utilities
└── ui/               # UI components
    ├── dialog.rs     # Input/confirm/message dialogs
    └── file_picker.rs # File selection UI
```

