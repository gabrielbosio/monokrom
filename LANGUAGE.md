# Monokrom Language Reference

Monokrom uses a statically-typed language with type inference. Programs compile through `Source -> AST -> HIR -> LIR (SSA) -> Bytecode`. Blocks are delimited with `end`.

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
| `fixed` | 8.8 fixed-point. Compiler sugar over `int`, compiles to pure integer operations |
| `bool` | `true` or `false` |
| `str` | Immutable string |
| `void` | For functions that return nothing |

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
| 3 | `==` `!=` `<` `>` `<=` `>=` | Comparison |
| 4 | `+` `-` | Addition, subtraction |
| 5 | `*` `/` `%` | Multiplication, division, modulo |
| 6 | `-` `not` | Unary negation, logical NOT |
| 7 | `()` `[]` `.` | Call, index, field access |

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
enemies[0].x = 10
enemies[0].alive = true
```

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
| `printn(val, x, y, col)` | Print number |

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
| `peek(addr): int` | Read memory |
| `poke(addr, val)` | Write memory |

### Math

| Function | Description |
|----------|-------------|
| `sin(x): fixed` | Sine |
| `cos(x): fixed` | Cosine |
| `sqrt(x): fixed` | Square root |
| `abs(x): int` | Absolute value |
| `min(a, b): int` | Minimum |
| `max(a, b): int` | Maximum |

### System

| Function | Description |
|----------|-------------|
| `flip()` | End frame, present screen |
| `time(): int` | Seconds since program start |
| `rnd(max): int` | Random integer in [0, max) |

### Debug

| Function | Description |
|----------|-------------|
| `tracen(val)` | Print number to terminal |
| `traces(s)` | Print string to terminal |

### Audio (deferred)

| Function | Description |
|----------|-------------|
| `sfx(n)` | Play sound effect |
| `music(n)` | Play music |

## Game Loop

Programs run through a `main()` function. Call `flip()` at the end of each frame to present the screen and yield control:

```
fn main()
    while true
        cls(0)
        // update and draw
        flip()
    end
end
```

Without `flip()`, the program runs to completion and exits.

## Limits

- **Bytecode**: 32KB max
- **Runtime memory**: 16KB flat (static allocation, no GC)
- **Sprites**: 8x8 pixels
