# Monokrom Language Reference

Monokrom uses a statically-typed language with type inference. The language was designed and implemented specifically for the technical limitations of the console. All the design decisions were made taking into account that programs that run in the console are the only use case of the language.
To run a Monokrom program, the source code gets compiled into bytecode so that the console's VM interprets each instruction at runtime.

## Example

```
fn main()
  prints("Hello, world!", 55, 70, 3)
  flip()
end
```

## Comments

```
// line comment

/* block
   comment */
```

## Types

- `int`: 16-bit signed integer. Range goes from -32768 to 32767.
- `fixed`: 8.8 fixed-point. Range goes from -128.0 to 127.99609375. Precision is 0.00390625. Compiler sugar over `int`.
- `bool`: `true` or `false`.
- `str`: Immutable string.
- `void`: For functions that return nothing.

A literal that does not fit its type is a compile error. At runtime, arithmetic wraps around on overflow.

Integer literals support decimal, hexadecimal, binary, and octal:

```
x = 255      // decimal
x = 0xFF     // hexadecimal (0x or 0X prefix)
x = 0b11111111 // binary (0b or 0B prefix)
x = 0o377    // octal (0o or 0O prefix)
```

## Variables

Type is inferred from the right-hand side:

```
x = 5        // int
y = 3.75     // fixed
alive = true // bool
name = "hi"  // str
```

Explicit type annotation:

```
x: int = 5
y: fixed = 3.75
```

Declaration without initialization must specify type:

```
x: int
```

Declaring a variable without initializing means declaring a variable and initializing it to its zero value. See [Zero initialization](#zero-initialization) for the zero value of each type.

Top-level variables are globals. Variables inside functions are locals. Variables declared inside `if`, `while`, or `for` blocks are scoped to that block.

Variables are mutable:

```
x: int = 5
x = 3
```

Re-declaration of a variable in the same scope is not allowed:

```
x: int = 5
x: int = 3 // Compile error
```

Re-declaring a variable in an inner scope shadows the name of the outer variable:

```
x: int = 5

fn main()
  y = x // y is 5
  x: int = 3
  z = x // z is 3
end
```

## Operators

Precedence from highest to lowest:

1. `()` `[]` `.`: Call, index, field access
2. `-` `not` `~`: Unary negation, logical NOT, bitwise NOT
3. `*` `/` `%`: Multiplication, division, modulo
4. `+` `-`: Addition, subtraction
5. `<<` `>>`: Shift left, arithmetic shift right (sign-extending)
6. `==` `!=` `<` `>` `<=` `>=`: Comparison
7. `&`: Bitwise AND
8. `^`: Bitwise XOR
9. `|`: Bitwise OR
10. `and`: Logical AND
11. `or`: Logical OR

## Control Flow

### If

```
if condition
  // ...
else if other_condition
  // ...
else
  // ...
end
```

### While

```
while condition
  // ...
end
```

`break` exits the loop. `continue` skips to the next iteration.

### For Loops

#### Range (exclusive)

```
for i in 0..10
  // i goes 0, 1, 2, ..., 9
end
```

#### Range (inclusive)

```
for i in 0..=10
  // i goes 0, 1, 2, ..., 10
end
```

`break` exits the loop. `continue` skips to the next iteration.

The loop variable can be shadowed by a declaration in the loop body. It does not affect the loop execution:

```
for i in 0..10
  x = i // x takes the value of the loop variable
  i: int = 5
  y = i // y is 5
  // The loop still runs 10 times
end
```

#### Array iteration

Both index and element variables are required. Variables with name `_` are discarded:

```
for i, e in enemies
  // i is the index, e is the element
end

for _, e in enemies
  // discard index
end
```

By default, iterating an array will copy each element so modifying the element does not affect the array:

```
for _, e in enemies
  e.x = e.x + 1  // modifies the copy, enemies[i] is unchanged
end
```

Prefixing the element with `ref` binds it as `ref T` and allows modifying the array inside the loop block:

```
for _, ref e in enemies
  e.x = e.x + 1  // modifies enemies[i].x
end
```

## Functions

```
fn add(a: int, b: int): int
  return a + b
end
```

Omit the return type for void functions:

```
fn greet(name: str)
  prints(name, 0, 0, 3)
end
```

All functions are global. No methods, no closures. Function overloading is not supported.

## Structs

Compiler sugar over raw memory. Fields have fixed offsets, no runtime type metadata. No methods.

```
struct Enemy
  x: int
  y: int
  hp: int
  alive: bool
end
```

Field access with dot notation:

```
e: Enemy
e.x = 10
e.alive = true
```

Assigning a struct copies all fields (value semantics):

```
a: Enemy
a.x = 10
b = a
b.x = 20
// a.x is still 10
```

## Arrays

Fixed-size, homogeneous:

```
enemies: array[40] of Enemy
scores: array[10] of int
```

Indexed with brackets:

```
enemies[0].x = 10
scores[i] = 100
```

## References

The `ref` keyword creates a mutable reference to a variable, field, or array element. References allow functions to modify the caller's data.

### Ref parameters

Without `ref`, all parameters are passed by value, so scalars are copied and structs and arrays are copied on entry. With `ref`, the callee receives an address and can modify the original:

```
fn inc(x: ref int)
  x = x + 1
end

fn main()
  a = 10
  inc(ref a)
  // a is now 11
end
```

The `ref` keyword is required at both the declaration and the call site.

### Ref with structs

```
struct Vec2
  x: int
  y: int
end

fn add(a: ref Vec2, b: ref Vec2)
  a.x = a.x + b.x
  a.y = a.y + b.y
end
```

Field access and assignment work through refs transparently, with no special syntax needed.

### Local ref bindings

You can bind a ref to a local variable for convenience:

```
p = ref game.player
p.x = p.x + dx
p.y = p.y + dy
```

### Auto-deref

Refs auto-dereference wherever a value is expected: operands, function and intrinsic arguments, array indexes, `if` and `while` conditions, `for` range bounds, and the array iterated by a `for-in`. `x + 1` works whether `x` is `int` or `ref int`:

```
fn double(x: ref int)
  x = x * 2   // x auto-derefs on read, writes through on assign
end
```

### Forwarding

A `ref` parameter can be passed directly to another `ref` parameter without repeating `ref`:

```
fn set_zero(x: ref int)
  x = 0
end

fn clear(x: ref int)
  set_zero(x)  // forwards the ref
end
```

### Rules

- Refs are scoped: only in function parameters and local bindings
- All refs are mutable
- Cannot store refs in struct fields or arrays
- Cannot return refs from functions
- No nested `ref ref T`

## Zero initialization

- `int`: `0`
- `fixed`: `0.0`
- `bool`: `false`
- `str`: `""`
- `struct`: each field gets zero-initialized
- `array`: each element gets zero-initialized
- `ref`: cannot be zero-initialized

## Intrinsics

Built-in functions that compile to single opcodes.

### Graphics

- `cls(col: int)`: Clear screen with color.
- `pset(x: int, y: int, col: int)`: Set pixel.
- `pget(x: int, y: int): int`: Get pixel color.
- `line(x0: int, y0: int, x1: int, y1: int, col: int)`: Draw line.
- `rect(x: int, y: int, w: int, h: int, col: int)`: Draw rectangle.
- `circ(x: int, y: int, r: int, col: int)`: Draw circle.
- `spr(n: int, x: int, y: int, flip_x: bool, flip_y: bool)`: Draw sprite. Colors marked transparent by `palt` are skipped.
- `map(sx: int, sy: int, dx: int, dy: int, w: int, h: int)`: Draw map region. Tiles at (sx,sy) to screen at (dx,dy), wxh tiles. Tiles are drawn as sprites, so `palt` applies.
- `palt(mask: int)`: Set which colors `spr` and `map` skip. Bit `i` set means color `i` is transparent. Defaults to 1, so color 0 is transparent and the other three are drawn. `palt(0)` makes sprites fully opaque.
- `prints(s: str, x: int, y: int, col: int)`: Print string.
- `printi(val: int, x: int, y: int, col: int)`: Print integer.
- `printf(val: fixed, x: int, y: int, col: int)`: Print fixed-point as decimal (e.g. "1.25").

### Input

- `btn(n: int): bool`: Button held down
- `btnp(n: int): bool`: Button just pressed

Button mapping:

- 0: Left
- 1: Right
- 2: Up
- 3: Down
- 4: Z
- 5: X
- 6: Enter
- 7: Right Shift

### Memory

- `peek(addr: int): int`: Read byte from memory
- `poke(addr: int, val: int)`: Write byte to memory
- `mget(x: int, y: int): int`: Get sprite index of the map tile at position
- `mset(x: int, y: int, tile: int)`: Set sprite index of the map tile at position

Memory layout:

| Address | Size | Contents |
|---------|------|----------|
| `0x0000` | 12KB | Program data (globals, locals, storage for call frames) |
| `0x3000` | 4KB | Sprite data (256 x 8 cols x 8 rows of 2-bit pixels) |
| `0x4000` | 4KB | Map data (128 x 32 tiles, 1 byte per tile) |
| `0x5000` | 256 bytes | Wave tables (16 x 32 4-bit samples) |
| `0x5100` | 256 bytes | Sfx lengths (256 x 1 byte) |
| `0x5200` | 11736 bytes | Sfx frames data (978 x 12 bytes) |
| `0x7FD8` | 8 bytes | Padding |
| `0x7FE0` | 16 bytes | live wave data |
| `0x7FF0` | 1 byte | Master volume |
| `0x7FF1` | 1 byte | Channel panning |
| `0x7FF2` | 14 bytes | Reserved |

Peek and poke operate on single bytes. Since `int` is 16-bit (2 bytes, little-endian), reading a full int requires two peeks:

```
val = peek(addr) + peek(addr + 1) * 256
```

### Math

- `sin(x: fixed): fixed`: Sine.
- `cos(x: fixed): fixed`: Cosine.
- `sqrt(x: fixed): fixed`: Square root.
- `abs(x: int): int`: Absolute value.
- `min(a: int, b: int): int`: Minimum.
- `max(a: int, b: int): int`: Maximum.
- `exp(x: fixed): fixed`: Euler's number raised to the power `x`. Clamps to the `fixed` range instead of wrapping.
- `log(x: fixed): fixed`: Natural logarithm (ln). Returns `0` for `x <= 0`.
- `pow(x: fixed, y: fixed): fixed`: `x` raised to the power `y`. Clamps to the `fixed` range instead of wrapping.
- `atan2(y: fixed, x: fixed): fixed`: Two-argument arctangent (radians).

### Conversion

- `ftoi(x: fixed): int`: Fixed to integer, rounding down.
- `itof(x: int): fixed`: Integer to fixed-point. The integer must fit the `fixed` range or the result wraps.

### Constants

| Name | Value | Description |
|------|-------|-------------|
| `PI` | `3.140625` | The mathematical constant pi, approximated to `fixed` precision |
| `E` | `2.71875` | Euler's number, approximated to `fixed` precision |
| `INT_MIN` | `-32768` | Minimum `int` value |
| `INT_MAX` | `32767` | Maximum `int` value |
| `FIXED_MIN` | `-128.0` | Minimum `fixed` value |
| `FIXED_MAX` | `127.99609375` | Maximum `fixed` value |

Constants are inlined at compile time.

### System

- `flip()`: End frame, present screen
- `time(): int`: Seconds since program start
- `rnd(max: int): int`: Random integer in [0, max)

### Debug

- `tracei(val: int)`: Print integer to terminal
- `traces(s: str)`: Print string to terminal
- `tracef(val: fixed)`: Print fixed-point as decimal to terminal

### Audio

- `sfx(n: int, frame: int)`: Play frame `frame` of sfx `n`. `n < 0` stops all channels.

## Game Loop

Programs run through a `main()` function. `flip()` presents the screen and yields control until the next frame. A typical game loop:

```
fn main()
  while true
    cls(0)
    // update and draw
    flip()
  end
end
```

When `main()` returns, the program exits. You can omit the game loop for programs that are one-shot scripts that only print output.
