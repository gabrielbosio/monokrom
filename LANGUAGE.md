# Monokrom Language Reference

Monokrom uses a statically-typed language with type inference. Programs compile through `Source -> AST -> HIR -> LIR (SSA) -> Bytecode`. Blocks are delimited with `end`.

## Example

```
fn main()
  prints("Hello, world!", 55, 70, 3)
end
```

## Comments

```
// line comment

/* block
   comment */
```

## Types

| Type | Description |
|------|-------------|
| `int` | 16-bit signed integer (-32768 to 32767) |
| `fixed` | 9.7 fixed-point. Compiler sugar over `int` that compiles to pure integer operations |
| `bool` | `true` or `false` |
| `str` | Immutable string |
| `void` | For functions that return nothing |

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
y = 3.14     // fixed
alive = true // bool
name = "hi"  // str
```

Explicit type annotation:

```
x: int = 5
y: fixed = 3.14
```

Declaration without initialization (must specify type):

```
x: int
```

Top-level variables are globals. Variables inside functions are locals. Variables declared inside `if`, `while`, or `for` blocks are scoped to that block.

## Operators

Precedence from lowest to highest:

| Precedence | Operators | Description |
|-----------|-----------|-------------|
| 1 | `or` | Logical OR |
| 2 | `and` | Logical AND |
| 3 | `\|` | Bitwise OR |
| 4 | `&` | Bitwise AND |
| 5 | `==` `!=` `<` `>` `<=` `>=` | Comparison |
| 6 | `<<` `>>` | Bit shift left, right |
| 7 | `+` `-` | Addition, subtraction |
| 8 | `*` `/` `%` | Multiplication, division, modulo |
| 9 | `-` `not` `~` | Unary negation, logical NOT, bitwise NOT |
| 10 | `()` `[]` `.` | Call, index, field access |

Bitwise operators work on `int` only. `>>` is arithmetic (sign-extending).

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

## For Loops

### Range (exclusive)

```
for i in 0..10
    // i goes 0, 1, 2, ..., 9
end
```

### Range (inclusive)

```
for i in 0..=10
    // i goes 0, 1, 2, ..., 10
end
```

### Array iteration

Both index and element variables are required. Use `_` to discard either:

```
for i, e in enemies
    // i is the index, e is the element
end

for _, e in enemies
    // discard index
end
```

For scalar types (`int`, `fixed`, `bool`), the element is a copy. For structs, the element is a reference to the array entry, so modifications are written back to the array:

```
for _, e in enemies
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

All functions are global. No methods, no closures.

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

| Function | Description |
|----------|-------------|
| `cls(col)` | Clear screen with color |
| `pset(x, y, col)` | Set pixel |
| `pget(x, y): int` | Get pixel color |
| `line(x0, y0, x1, y1, col)` | Draw line |
| `rect(x, y, w, h, col)` | Draw rectangle |
| `circ(x, y, r, col)` | Draw circle |
| `spr(n, x, y)` | Draw sprite |
| `prints(s, x, y, col)` | Print string |
| `printi(val, x, y, col)` | Print integer |
| `printf(val, x, y, col)` | Print fixed-point as decimal (e.g. "1.50") |

### Input

| Function | Description |
|----------|-------------|
| `btn(n): bool` | Button held down |
| `btnp(n): bool` | Button just pressed |

Button mapping:

| n | Button |
|---|--------|
| 0 | Left |
| 1 | Right |
| 2 | Up |
| 3 | Down |
| 4 | Z |
| 5 | X |
| 6 | Enter |
| 7 | Right Shift |

### Memory

| Function | Description |
|----------|-------------|
| `peek(addr): int` | Read byte from memory |
| `poke(addr, val)` | Write byte to memory |

Peek and poke operate on single bytes. Since `int` is 16-bit (2 bytes, little-endian), reading a full int requires two peeks:

```
val = peek(addr) + peek(addr + 1) * 256
```

### Math

| Function | Description |
|----------|-------------|
| `sin(x): fixed` | Sine |
| `cos(x): fixed` | Cosine |
| `sqrt(x): fixed` | Square root |
| `abs(x): int` | Absolute value |
| `min(a, b): int` | Minimum |
| `max(a, b): int` | Maximum |
| `exp(x): fixed` | e^x |
| `log(x): fixed` | Natural logarithm (ln). Returns 0 for x <= 0 |
| `pow(x, y): fixed` | x raised to the power y |
| `atan2(y, x): fixed` | Two-argument arctangent (radians) |

### Conversion

| Function | Description |
|----------|-------------|
| `ftoi(x): int` | Fixed to integer (truncates fractional part) |
| `itof(x): fixed` | Integer to fixed-point |

### Constants

| Name | Value | Description |
|------|-------|-------------|
| `PI` | 3.14159265 | The mathematical constant pi |
| `E` | 2.71828182 | Euler's number |

Constants are inlined at compile time as fixed-point literals.

### System

| Function | Description |
|----------|-------------|
| `flip()` | End frame, present screen |
| `time(): int` | Seconds since program start |
| `rnd(max): int` | Random integer in [0, max) |

### Debug

| Function | Description |
|----------|-------------|
| `tracei(val)` | Print integer to terminal |
| `traces(s)` | Print string to terminal |
| `tracef(val)` | Print fixed-point as decimal to terminal |

### Audio (deferred)

| Function | Description |
|----------|-------------|
| `sfx(n)` | Play sound effect |
| `music(n)` | Play music |

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

Programs that don't call `flip()` run once and exit, which is useful for one-shot scripts that just print output.

## Limits

- **Bytecode**: 32KB max
- **Runtime memory**: 16KB flat (static allocation, no GC)
- **Sprites**: 8x8 pixels
