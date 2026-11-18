# Monokrom

Monokrom is a fantasy console where its imposed technical limitations make it ressemble a game framework for retro monochrome handhelds. Its interface is heavily inspired by PICO-8 and TIC-80. The main difference with the reference consoles is that the code limit is imposed to the bytecode size instead of the source code size or token count.

## Specs

- **Screen:** 160x144 pixels (40x24 chars with 4x6 font). Actual size will depend on the window size but the ratio is always preserved.
- **Palette:** 4-color monochrome (black, dark gray, light gray, white)
- **Bytecode limit:** 32KB
- **Runtime memory:** 32KB flat
- **Sprites:** 256 sprites, 8x8 pixels, 2bpp (4 colors)
- **Map:** 128 cols x 32 rows, 1 byte per cell

## Language

See [LANGUAGE.md](LANGUAGE.md) for the full language reference.

## Building

Requires [Rust](https://rust-lang.org/) (stable).

```bash
cargo run                          # debug
cargo run --release                # release
```

Debug stores files in `./fs/`, release in `~/.monokrom/`.

## Usage

> *NOTE:* The docs show that some hotkeys involve pressing the Ctrl key. Due to context-specific restrictions, some hotkeys are mapped differently depending where Monokrom is running:
>
> - On macOS, Ctrl+Arrow, Ctrl+Backspace, and Ctrl+Delete shortcuts use Option instead of Ctrl.
> - On WASM, all Ctrl hotkeys use Alt instead.

Monokrom boots into a terminal. Press **Escape** to switch between edit mode and terminal. Edit mode consists of different editors that can be accessed through the following hotkeys:

- **F1/Ctrl+1** Code editor
- **F2/Ctrl+2** Sprite editor
- **F3/Ctrl+3** Map editor
- **F4/Ctrl+4** Wave editor
- **F5/Ctrl+5** Sfx editor

**Ctrl+Enter** runs the current file from any editor mode or the terminal. **Escape** while running stops the program and switches to the terminal, as does returning from `main()`. Any error triggered during program execution is shown there too.

This section shows how to navigate through each screen. Depending on the screen, navigation may consist of running commands by writing them and/or triggering them using specific hotkeys.

### Terminal

- `run`: compile and run the current file
- `stat`: show bytecode size vs limit
- `new` / `open <file>` / `save <file>`: file operations
- `ls` / `rm <file>` / `cp <src> <dst>`: file management
- `export <name>`: export game as a standalone binary
- `example`: list/load built-in examples
- `clear`: clear the terminal
- `help`: show all commands

### Code editor

- **Ctrl+S** save, **Ctrl+O** open, **Ctrl+N** new file
- **Ctrl+Z** undo, **Ctrl+Y** redo
- **Ctrl+X/C/V** cut, copy, paste
- **Ctrl+A** select all
- **Ctrl+F** find, **Ctrl+R** replace, **Ctrl+L** go to line
- **Ctrl+Left/Right** word movement, **Ctrl+Backspace** word delete
- **Ctrl+Up/Down** scroll viewport one line
- **Shift+arrows** selection, **Shift+Home/End** select to line start/end
- **Ctrl+Shift+Left/Right** select by word
- **PageUp/PageDown** move cursor by page
- **Tab/Shift+Tab** indent/dedent

### Sprite editor

- **Arrow keys** move sprite cursor
- **Space** paint pixel (hold while moving to draw continuously)
- **1/2/3/4** select color (1=black, 2=dark, 3=light, 4=white)
- **C** eyedropper (pick color under cursor)
- **Ctrl+Arrow** move spritesheet cursor by 1 tile
- **Ctrl+Shift+Arrow** move spritesheet cursor by 4 tiles
- **Ctrl+Z** undo, **Ctrl+Y** redo
- **Ctrl+C/V** copy/paste sprite
- **Ctrl+S** save
- **F** flip horizontal, **Shift+F** flip vertical
- **Shift+Arrow** shift sprite contents
- **Delete** clear sprite
- **+/=** zoom in
- **-** zoom out

### Map editor

- **Arrow keys** move map cursor by 1 tile on editor mode, or move map by 1 tile on full map mode
- **Shift+Arrow** move map cursor by 8 tiles on editor mode, or move map by 8 tiles on full map mode
- **Space** paint tile (hold while moving to paint continuously)
- **Delete/Backspace** clear tile (place tile 0)
- **C** eyedropper (pick tile under cursor)
- **Ctrl+Arrow** move tile cursor by 1 tile
- **Ctrl+Shift+Arrow** move tile cursor by 4 tiles
- **Tab** toggle editor/full map mode
- **Ctrl+Z** undo, **Ctrl+Y** redo
- **Ctrl+S** save

### Wave editor

- **Left/Right** move cursor by 1 sample
- **Shift+Left/Right** move cursor by 8 samples
- **Up/Down** increase/decrease selected sample amplitude by 1
- **Shift+Up/Down** increase/decrease selected sample amplitude by 4
- **Ctrl+Left/Right** switch current wave
- **Ctrl+Z** undo, **Ctrl+Y** redo
- **Ctrl+S** save
- **Space** play/stop

### Sfx editor

#### Global

- **Tab** switch between header and grid
- **Ctrl+S** save
- **Ctrl+Z** undo, **Ctrl+Y** redo
- **Space** play from start, stop
- **Shift+Space** play from cursor position

#### Header

- **Left/Right** switch selected value
- **Up/Down** increase/decrease selected value by 1 step
- **Shift+Up/Down** increase/decrease selected value by big steps

#### Grid

- **Left/Right** move cursor by 1 frame
- **Shift+Left/Right** move cursor by 8 frames
- **Ctrl+Left/Right** increase/decrease selected parameter value by 1 step
- **Ctrl+Shift+Left/Right** increase/decrease selected parameter value by big steps
- **Up/Down** switch selected channel
- **Ctrl+Up/Down** switch selected parameter
- **A/B/C/D/E/F/G** put A, B, C, D, E, F or G note in selected cell in P1, P2 and WV channels, gate the noise on in NS channel
- **Shift+A/B/C/D/E/F/G** put A#, B# (C), C#, D#, E# (F), F# or G# note in selected cell in P1, P2 and WV channels, gate the noise on in NS channel
- **2/3/4/5/6/7** set octave in selected cell in P1, P2 and WV channels
- **Delete/Backspace** remove note, put rest or remove rest in selected cell
- **Ctrl+C/V** copy/paste selected event

### Exporting

The `export` command bundles your game with the Monokrom engine so it can run directly without the editor.

- **Native**: `export mygame` creates a self-contained executable. Run it with `./mygame`.
- **WASM**: `export mygame` downloads `mygame.html`. Serve it alongside the runtime files.
