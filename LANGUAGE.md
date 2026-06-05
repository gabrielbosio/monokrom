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
- `fixed`: 9.7 fixed-point. Range goes from -256.0 to 255.9921875. Precision is 0.0078125. Compiler sugar over `int`.
- `bool`: `true` or `false`.
- `str`: Immutable string.
- `void`: For functions that return nothing.

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

Top-level variables are globals. Variables inside functions are locals. Variables declared inside `if`, `while`, or `for` blocks are scoped to that block.

## Operators

Precedence from highest to lowest:

1. `()` `[]` `.`: Call, index, field access
2. `-` `not` `~`: Unary negation, logical NOT, bitwise NOT
3. `*` `/` `%`: Multiplication, division, modulo
4. `+` `-`: Addition, subtraction
5. `<<` `>>`: Shift left, arithmetic shift right (sign-extending)
6. `==` `!=` `<` `>` `<=` `>=`: Comparison
7. `&`: Bitwise AND
8. `|`: Bitwise OR
9. `and`: Logical AND
10. `or`: Logical OR

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

Refs auto-dereference in expressions. `x + 1` works whether `x` is `int` or `ref int`:

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

## Intrinsics

Built-in functions that compile to single opcodes.

### Graphics

- `cls(col)`: Clear screen with color
- `pset(x, y, col)`: Set pixel.
- `pget(x, y): int`: Get pixel color.
- `line(x0, y0, x1, y1, col)`: Draw line.
- `rect(x, y, w, h, col)`: Draw rectangle.
- `circ(x, y, r, col)`: Draw circle
- `spr(n, x, y)`: Draw sprite.
- `map(sx, sy, dx, dy, w, h)`: Draw map region. Tiles at (sx,sy) to screen at (dx,dy), w×h tiles.
- `prints(s, x, y, col)`: Print string.
- `printi(val, x, y, col)`: Print integer.
- `printf(val, x, y, col)`: Print fixed-point as decimal (e.g. "1.25").

### Input

- `btn(n): bool`: Button held down
- `btnp(n): bool`: Button just pressed

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

- `peek(addr): int`: Read byte from memory
- `poke(addr, val)`: Write byte to memory
- `mget(x, y): int`: Get sprite index of the map tile at position
- `mset(x, y, tile)`: Set sprite index of the map tile at position

Memory layout:

| Address | Size | Contents |
|---------|------|----------|
| `0x0000` | 12KB | General purpose (globals, arrays) |
| `0x3000` | 4KB | Sprite data (256 × 16 bytes) |
| `0x4000` | 4KB | Map data (128 × 32 tiles) |

Peek and poke operate on single bytes. Since `int` is 16-bit (2 bytes, little-endian), reading a full int requires two peeks:

```
val = peek(addr) + peek(addr + 1) * 256
```

### Math

- `sin(x): fixed`: Sine.
- `cos(x): fixed`: Cosine.
- `sqrt(x): fixed`: Square root.
- `abs(x): int`: Absolute value.
- `min(a, b): int`: Minimum.
- `max(a, b): int`: Maximum.
- `exp(x): fixed`: Euler's number raised to the power `x`.
- `log(x): fixed`: Natural logarithm (ln). Returns `0` for `x <= 0`.
- `pow(x, y): fixed`: `x` raised to the power `y`.
- `atan2(y, x): fixed`: Two-argument arctangent (radians).

### Conversion

- `ftoi(x): int`: Fixed to integer (truncates fractional part)
- `itof(x): fixed`: Integer to fixed-point

### Constants

| Name | Value | Description |
|------|-------|-------------|
| `PI` | `3.140625` | The mathematical constant pi, approximated to `fixed` precision |
| `E` | `2.71875` | Euler's number, approximated to `fixed` precision |
| `INT_MIN` | `-32768` | Minimum `int` value |
| `INT_MAX` | `32767` | Maximum `int` value |

Constants are inlined at compile time.

### System

- `flip()`: End frame, present screen
- `time(): int`: Seconds since program start
- `rnd(max): int`: Random integer in [0, max)

### Debug

- `tracei(val)`: Print integer to terminal
- `traces(s)`: Print string to terminal
- `tracef(val)`: Print fixed-point as decimal to terminal

### Audio (deferred)

- `sfx(n)`: Play sound effect
- `music(n)`: Play music

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
