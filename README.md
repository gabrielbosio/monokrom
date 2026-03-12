# Monokrom

Fantasy console inspired by retro monochrome handhelds. Like PICO-8/TIC-80 but memory limits are based on bytecode size, not source character count.

Built in Rust with Macroquad.

## Specs

- Screen: 160x144 pixels (40x24 chars with 4x6 font, scaled 4x)
- Palette: 4-color monochrome (black, dark gray, light gray, white)
- Bytecode limit: 32KB
- Runtime memory: 16KB flat
- Sprites: 256 sprites, 8x8 pixels, 2bpp (4 colors), memory-mapped at 0x3000

## Language

Monokrom programs are written in a statically-typed language with type inference. The compiler pipeline is:

```
Source -> AST -> HIR -> LIR (SSA) -> Bytecode
```

Optimization passes: constant folding, constant propagation, dead code elimination, CSE, strength reduction, function inlining, peephole store-load elimination.

See [LANGUAGE.md](LANGUAGE.md) for the full language reference.

## Building

Requires Rust (stable).

```bash
cargo run                          # debug
cargo run --release                # release
```

Debug stores files in `./fs/`, release in `~/.monokrom/`.

## Usage

Monokrom boots into a terminal. Press Escape to toggle between terminal and editor.

### Terminal commands

- `run`: compile and run the current file
- `stat`: show bytecode size vs 32KB limit
- `open <file>` / `save <file>`, file operations (use quotes for spaces: `open "my file"`)
- `ls`, list files
- `cp <src> <dst>`, copy file
- `rm <file>`, remove file
- `example`, list built-in examples
- `example <name>`, load an example into the editor (e.g. `example tictactoe`)
- `export <name>`: export game as a standalone binary

### Exporting

The `export` command bundles your game with the Monokrom engine so it can run directly without the editor.

- **Native**: `export mygame` creates a self-contained executable in the current directory. Run it with `./mygame`.
- **WASM**: `export mygame` downloads `mygame.html`. Serve it alongside the runtime files (`gl.js`, `sapp_jsutils.js`, `quad-storage.js`, `monokrom.js`, `monokrom.wasm`).

### Editor hotkeys

On macOS, word operations use Option instead of Ctrl (Ctrl+Arrow is intercepted by the OS). On WASM, all Ctrl shortcuts use Alt instead (browsers intercept Ctrl combos).

- **Ctrl+S** save, **Ctrl+O** open, **Ctrl+N** new file
- **Ctrl+Enter** run program
- **Ctrl+Z** undo, **Ctrl+Y** redo
- **Ctrl+X/C/V** cut, copy, paste
- **Ctrl+A** select all
- **Ctrl+F** find, **Ctrl+R** replace, **Ctrl+L** go to line
- **Ctrl+Left/Right** word movement, **Ctrl+Backspace** word delete (macOS: Option instead of Ctrl)
- **Ctrl+Up/Down** scroll viewport one line (macOS: Option instead of Ctrl)
- **Shift+arrows** selection, **Shift+Home/End** select to line start/end
- **Ctrl+Shift+Left/Right** select by word (macOS: Option+Shift)
- **PageUp/PageDown** move cursor by page
- **Tab/Shift+Tab** indent/dedent (block indent when multi-line selected)
- **Escape** cycle between terminal, editor, and sprite editor

### Sprite editor

Press Escape from the code editor to enter the sprite editor.

- **Arrow keys** move cursor on the 8x8 pixel grid
- **Space** paint pixel (hold while moving to draw continuously)
- **1/2/3/4** select color (1=black, 2=dark, 3=light, 4=white)
- **C** eyedropper (pick color under cursor)
- **Tab/Shift+Tab** next/previous sprite
- **Ctrl+Z** undo, **Ctrl+Y** redo
- **Ctrl+C/V** copy/paste sprite
- **Ctrl+S** save
- **F** flip horizontal, **Shift+F** flip vertical
- **Shift+Arrow** shift sprite contents
- **Delete** clear sprite
- **Escape** return to terminal

Sprite data is saved alongside source code in `.mkr` files and included in exports.
