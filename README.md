# Monokrom

Monokrom is a fantasy console where its imposed technical limitations make it ressemble a game framework for retro monochrome handhelds. Its interface is heavily inspired by PICO-8 and TIC-80. The main difference with the reference consoles is that the code limit is imposed to the bytecode size instead of the source code size or token count.

## Specs

- **Screen:** 160x144 pixels (40x24 chars with 4x6 font). Actual size will depend on the window size but the ratio is always preserved.
- **Palette:** 4-color monochrome (black, dark gray, light gray, white)
- **Bytecode limit:** 32KB
- **Runtime memory:** 20KB flat
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

Monokrom boots into a terminal. Press **Escape** to cycle through the following screens in order:

- Terminal
- Code editor
- Sprite editor
- Map editor
- Sfx editor

**Shift+Escape** cycles in reverse.

**Ctrl+Enter** runs the current file from any editor mode or the terminal. **Escape** while running stops the program and returns to whichever mode launched it. Any error triggered during program execution will switch to the terminal and show the corresponding error message, regardless of the editor set before starting the program.

This section shows how to navigate through each screen. Depending on the screen, navigation may consist of running commands by writing them and/or triggering them using specific hotkeys.

> *NOTE:* The docs show that some hotkeys involve pressing the Ctrl key. Due to context-specific restrictions, some hotkeys are mapped differently depending where Monokrom is running:
>
> - On macOS, word operations (i.e. Ctrl+Arrow) use Option instead of Ctrl.
> - On WASM, all Ctrl hotkeys use Alt instead.

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
- **Ctrl+Enter** run program
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

- **Arrow keys** move cursor on the 8x8 pixel grid
- **Space** paint pixel (hold while moving to draw continuously)
- **1/2/3/4** select color (1=black, 2=dark, 3=light, 4=white)
- **C** eyedropper (pick color under cursor)
- **Alt+Arrow** navigate the sprite sheet
- **Ctrl+Z** undo, **Ctrl+Y** redo
- **Ctrl+C/V** copy/paste sprite
- **Ctrl+S** save
- **F** flip horizontal, **Shift+F** flip vertical
- **Shift+Arrow** shift sprite contents
- **Delete** clear sprite

### Map editor

- **Arrow keys** move cursor on the 128×32 tile grid
- **Shift+Arrow** move cursor by 8 tiles
- **Space** paint tile (hold while moving to paint continuously)
- **Delete/Backspace** clear tile (place tile 0)
- **C** eyedropper (pick tile under cursor)
- **Alt+Arrow** navigate the tile picker (picker scrolls one column at a time)
- **Tab** (hold) peek a 20×18 tile region as it will appear at runtime
- **Ctrl+Z** undo, **Ctrl+Y** redo
- **Ctrl+S** save

### Sfx editor

This editor has two panes which you set focus on one of them: The header and the grid. When entering this editor for the first time, the focus is set on the grid. There are hotkeys that are exclusive for each pane and that work for both.

#### Global

- **Tab** toggle header / grid focus
- **Space** play selected SFX, **Shift+Space** stop all sounds
- **Ctrl+S** save
- **Ctrl+Z** undo, **Ctrl+Y** — redo
- **Ctrl+Shift+C/V** copy/paste whole SFX

#### Grid

- **Ctrl+C/V** copy/paste cell
- **Left/Right** move cell cursor
- **Up/Down** change row (Pitch, Timbre, Volume, Effect)
- **Alt+Up/Down** change current field by 1
- **Shift+Up/Shift+Down** change current field on Pitch row by an octave
- **0..7** set current field to that digit
- **Delete/Backspace** clear current cell

#### Header

- **Left/Right** previous / next field (Sfx, Channel, Speed, LoopStart, LoopEnd)
- **Up/Down** change value by 1
- **Shift+Up/Down** change value by 10

### Exporting

The `export` command bundles your game with the Monokrom engine so it can run directly without the editor.

- **Native**: `export mygame` creates a self-contained executable. Run it with `./mygame`.
- **WASM**: `export mygame` downloads `mygame.html`. Serve it alongside the runtime files.
