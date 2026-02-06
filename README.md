# Monokrom

Fantasy console inspired by retro monochrome handhelds. Like Pico-8/TIC-80 but memory limits are based on bytecode size, not source character count.

Built in Rust with Macroquad. Currently in Version 1 (text editor).

## Specs

- Screen: 256x240 pixels (32x30 tiles of 8x8, scaled 3x)
- Palette: Black (0), White (1), Gray (2)

## Building

Requires Rust (stable).

```bash
cargo run                          # debug
cargo run --release                # release
```

Debug stores files in `./fs/`, release in `~/.monokrom/`.

