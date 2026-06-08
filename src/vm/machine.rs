use crate::compiler::bytecode::*;
use crate::config::{
    MAP_HEIGHT, MAP_REGION_START, MAP_WIDTH, SFX_COUNT, SPRITE_COUNT, SPRITE_REGION_START,
    SPRITE_SIZE,
};
use crate::render::font::FONT_DATA;

const STACK_LIMIT: usize = 256;
const CALL_STACK_LIMIT: usize = 64;
const MEMORY_SIZE: usize = 22528;
const FB_WIDTH: usize = 160;
const FB_HEIGHT: usize = 144;
const FB_SIZE: usize = FB_WIDTH * FB_HEIGHT;
const CYCLE_LIMIT: u32 = 1_000_000;
const FP_SCALE: f32 = 128.0;

#[derive(Debug)]
pub enum VmError {
    StackOverflow,
    StackUnderflow,
    DivisionByZero,
    OutOfBounds(String),
    NoEntryPoint,
    CallStackOverflow,
    CallStackUnderflow,
    UnknownOpcode(u8),
    CycleLimit,
}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StackOverflow => write!(f, "stack overflow"),
            Self::StackUnderflow => write!(f, "stack underflow"),
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::OutOfBounds(msg) => write!(f, "out of bounds: {msg}"),
            Self::NoEntryPoint => write!(f, "no main() function"),
            Self::CallStackOverflow => write!(f, "call stack overflow"),
            Self::CallStackUnderflow => write!(f, "call stack underflow"),
            Self::UnknownOpcode(op) => write!(f, "unknown opcode 0x{op:02X}"),
            Self::CycleLimit => write!(f, "cycle limit exceeded"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum VmResult {
    Continue,
    Flip,
    Halted,
}

struct CallFrame {
    return_pc: usize,
    locals_base: usize,
}

struct FuncEntry {
    code_offset: usize,
    n_locals: u16,
}

pub struct Vm {
    code: Vec<u8>,
    pc: usize,
    stack: Vec<i16>,
    locals: Vec<i16>,
    call_stack: Vec<CallFrame>,
    pub memory: [u8; MEMORY_SIZE],
    pub framebuffer: [u8; FB_SIZE],
    pub buttons: u8,
    pub prev_buttons: u8,
    string_pool: Vec<String>,
    functions: Vec<FuncEntry>,
    pub halted: bool,
    pub trace_output: Vec<String>,
    pub sfx_queue: Vec<u8>,
    start_time: f64,
    pub current_time: f64,
    rng_state: u32,
}

impl Vm {
    pub fn new(bc: &Bytecode, time: f64) -> Result<Self, VmError> {
        let entry = bc.entry_point.ok_or(VmError::NoEntryPoint)?;

        let functions: Vec<FuncEntry> = bc
            .functions
            .iter()
            .map(|f| FuncEntry {
                code_offset: f.code_offset,
                n_locals: f.n_locals,
            })
            .collect();

        // Find main's function entry to get n_locals
        let main_func = bc
            .functions
            .iter()
            .find(|f| f.code_offset == entry)
            .ok_or(VmError::NoEntryPoint)?;

        let n_locals = main_func.n_locals as usize;
        let locals = vec![0i16; n_locals];

        let mut vm = Self {
            code: bc.code.clone(),
            pc: entry,
            stack: Vec::with_capacity(STACK_LIMIT),
            locals,
            call_stack: Vec::with_capacity(CALL_STACK_LIMIT),
            memory: [0; MEMORY_SIZE],
            framebuffer: [0; FB_SIZE],
            buttons: 0,
            prev_buttons: 0,
            string_pool: bc.string_pool.clone(),
            functions,
            halted: false,
            trace_output: Vec::new(),
            sfx_queue: Vec::new(),
            start_time: time,
            current_time: time,
            rng_state: (time * 1_000_000.0) as u32 | 1,
        };

        // Push initial call frame for main (return_pc = usize::MAX means halt on return)
        vm.call_stack.push(CallFrame {
            return_pc: usize::MAX,
            locals_base: 0,
        });

        Ok(vm)
    }

    fn push(&mut self, v: i16) -> Result<(), VmError> {
        if self.stack.len() >= STACK_LIMIT {
            return Err(VmError::StackOverflow);
        }
        self.stack.push(v);
        Ok(())
    }

    fn pop(&mut self) -> Result<i16, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn read_u8(&mut self) -> u8 {
        let v = self.code[self.pc];
        self.pc += 1;
        v
    }

    fn read_i8(&mut self) -> i8 {
        self.read_u8() as i8
    }

    fn read_i16(&mut self) -> i16 {
        let lo = self.code[self.pc];
        let hi = self.code[self.pc + 1];
        self.pc += 2;
        i16::from_le_bytes([lo, hi])
    }

    fn read_u16(&mut self) -> u16 {
        let lo = self.code[self.pc];
        let hi = self.code[self.pc + 1];
        self.pc += 2;
        u16::from_le_bytes([lo, hi])
    }

    fn locals_base(&self) -> usize {
        self.call_stack.last().map(|f| f.locals_base).unwrap_or(0)
    }

    fn load_local(&self, slot: u16) -> i16 {
        self.locals[self.locals_base() + slot as usize]
    }

    fn store_local(&mut self, slot: u16, val: i16) {
        let base = self.locals_base();
        self.locals[base + slot as usize] = val;
    }

    pub fn step(&mut self) -> Result<VmResult, VmError> {
        if self.halted {
            return Ok(VmResult::Halted);
        }

        macro_rules! binop {
            ($op:expr) => {{
                let b = self.pop()?;
                let a = self.pop()?;
                self.push($op(a, b))?;
            }};
        }

        let op = self.read_u8();
        match op {
            OP_PUSH0 => self.push(0)?,
            OP_PUSH1 => self.push(1)?,
            OP_PUSH_I8 => {
                let v = self.read_i8() as i16;
                self.push(v)?;
            }
            OP_PUSH_I16 => {
                let v = self.read_i16();
                self.push(v)?;
            }
            OP_POP => {
                self.pop()?;
            }
            OP_LOAD_LOCAL => {
                let slot = self.read_u16();
                let v = self.load_local(slot);
                self.push(v)?;
            }
            OP_STORE_LOCAL => {
                let slot = self.read_u16();
                let v = self.pop()?;
                self.store_local(slot, v);
            }
            OP_ADD => binop!(i16::wrapping_add),
            OP_SUB => binop!(i16::wrapping_sub),
            OP_MUL => binop!(i16::wrapping_mul),
            OP_DIV => {
                let b = self.pop()?;
                if b == 0 {
                    return Err(VmError::DivisionByZero);
                }
                let a = self.pop()?;
                self.push(a.wrapping_div(b))?;
            }
            OP_MOD => {
                let b = self.pop()?;
                if b == 0 {
                    return Err(VmError::DivisionByZero);
                }
                let a = self.pop()?;
                self.push(a.wrapping_rem(b))?;
            }
            OP_NEG => {
                let a = self.pop()?;
                self.push(a.wrapping_neg())?;
            }
            OP_FMUL => binop!(|a: i16, b| ((a as i32 * b as i32) >> 7) as i16),
            OP_FDIV => {
                let b = self.pop()?;
                if b == 0 {
                    return Err(VmError::DivisionByZero);
                }
                let a = self.pop()?;
                self.push((((a as i32) << 7) / b as i32) as i16)?;
            }
            OP_EQ => binop!(|a: i16, b| if a == b { 1 } else { 0 }),
            OP_NEQ => binop!(|a: i16, b| if a != b { 1 } else { 0 }),
            OP_LT => binop!(|a: i16, b| if a < b { 1 } else { 0 }),
            OP_GT => binop!(|a: i16, b| if a > b { 1 } else { 0 }),
            OP_LEQ => binop!(|a: i16, b| if a <= b { 1 } else { 0 }),
            OP_GEQ => binop!(|a: i16, b| if a >= b { 1 } else { 0 }),
            OP_AND => binop!(|a: i16, b| if a != 0 && b != 0 { 1 } else { 0 }),
            OP_OR => binop!(|a: i16, b| if a != 0 || b != 0 { 1 } else { 0 }),
            OP_NOT => {
                let a = self.pop()?;
                self.push(if a == 0 { 1 } else { 0 })?;
            }
            OP_BAND => binop!(|a: i16, b| a & b),
            OP_BOR => binop!(|a: i16, b| a | b),
            OP_BXOR => binop!(|a: i16, b| a ^ b),
            OP_BNOT => {
                let a = self.pop()?;
                self.push(!a)?;
            }
            OP_SHL => binop!(|a: i16, b| a.wrapping_shl(b as u32)),
            OP_SHR => binop!(|a: i16, b| a.wrapping_shr(b as u32)),
            OP_LOAD1 => {
                let addr = self.pop()? as u16 as usize;
                if addr >= MEMORY_SIZE {
                    return Err(VmError::OutOfBounds(format!("memory read at {addr}")));
                }
                self.push(self.memory[addr] as i16)?;
            }
            OP_LOAD2 => {
                let addr = self.pop()? as u16 as usize;
                if addr + 1 >= MEMORY_SIZE {
                    return Err(VmError::OutOfBounds(format!("memory read at {addr}")));
                }
                let v = i16::from_le_bytes([self.memory[addr], self.memory[addr + 1]]);
                self.push(v)?;
            }
            OP_STORE1 => {
                let val = self.pop()?;
                let addr = self.pop()? as u16 as usize;
                if addr >= MEMORY_SIZE {
                    return Err(VmError::OutOfBounds(format!("memory write at {addr}")));
                }
                self.memory[addr] = val as u8;
            }
            OP_STORE2 => {
                let val = self.pop()?;
                let addr = self.pop()? as u16 as usize;
                if addr + 1 >= MEMORY_SIZE {
                    return Err(VmError::OutOfBounds(format!("memory write at {addr}")));
                }
                let bytes = val.to_le_bytes();
                self.memory[addr] = bytes[0];
                self.memory[addr + 1] = bytes[1];
            }
            OP_JUMP => {
                let offset = self.read_i16();
                self.pc = (self.pc as isize + offset as isize) as usize;
            }
            OP_JUMP_IF_FALSE => {
                let offset = self.read_i16();
                let cond = self.pop()?;
                if cond == 0 {
                    self.pc = (self.pc as isize + offset as isize) as usize;
                }
            }
            OP_CALL => {
                let func_idx = self.read_u16() as usize;
                let argc = self.read_u8();
                if func_idx >= self.functions.len() {
                    return Err(VmError::OutOfBounds(format!("function index {func_idx}")));
                }
                if self.call_stack.len() >= CALL_STACK_LIMIT {
                    return Err(VmError::CallStackOverflow);
                }

                let n_locals = self.functions[func_idx].n_locals;
                let code_offset = self.functions[func_idx].code_offset;
                let new_base = self.locals.len();

                // Allocate locals for the callee
                self.locals.resize(new_base + n_locals as usize, 0);

                // Pop args from stack and copy to first N local slots
                for i in (0..argc as usize).rev() {
                    let v = self.pop()?;
                    self.locals[new_base + i] = v;
                }

                self.call_stack.push(CallFrame {
                    return_pc: self.pc,
                    locals_base: new_base,
                });

                self.pc = code_offset;
            }
            OP_RET => {
                let frame = self.call_stack.pop().ok_or(VmError::CallStackUnderflow)?;
                if frame.return_pc == usize::MAX {
                    self.halted = true;
                    return Ok(VmResult::Halted);
                }
                // Truncate locals back to caller's frame
                self.locals.truncate(frame.locals_base);
                self.pc = frame.return_pc;
            }
            OP_HALT => {
                self.halted = true;
                return Ok(VmResult::Halted);
            }
            // -- Intrinsics --
            OP_PSET => {
                let col = self.pop()?;
                let y = self.pop()?;
                let x = self.pop()?;
                self.fb_pset(x as i32, y as i32, col as u8);
            }
            OP_PGET => {
                let y = self.pop()?;
                let x = self.pop()?;
                let c = self.fb_pget(x as i32, y as i32);
                self.push(c as i16)?;
            }
            OP_CLS => {
                let col = self.pop()? as u8;
                self.framebuffer.fill(col & 3);
            }
            OP_LINE => {
                let col = self.pop()?;
                let y1 = self.pop()?;
                let x1 = self.pop()?;
                let y0 = self.pop()?;
                let x0 = self.pop()?;
                self.fb_line(x0 as i32, y0 as i32, x1 as i32, y1 as i32, col as u8);
            }
            OP_RECT => {
                let col = self.pop()?;
                let h = self.pop()?;
                let w = self.pop()?;
                let y = self.pop()?;
                let x = self.pop()?;
                self.fb_rect(x as i32, y as i32, w as i32, h as i32, col as u8);
            }
            OP_CIRC => {
                let col = self.pop()?;
                let r = self.pop()?;
                let y = self.pop()?;
                let x = self.pop()?;
                self.fb_circ(x as i32, y as i32, r as i32, col as u8);
            }
            OP_SPR => {
                let y = self.pop()?;
                let x = self.pop()?;
                let n = self.pop()?;
                self.fb_spr(n as u16, x as i32, y as i32);
            }
            OP_PRINTS => {
                let col = self.pop()?;
                let y = self.pop()?;
                let x = self.pop()?;
                let str_idx = self.pop()? as u16 as usize;
                if str_idx < self.string_pool.len() {
                    let s = self.string_pool[str_idx].clone();
                    self.fb_print(&s, x as i32, y as i32, col as u8);
                }
            }
            OP_PRINTI => {
                let col = self.pop()?;
                let y = self.pop()?;
                let x = self.pop()?;
                let val = self.pop()?;
                self.fb_print(&format!("{val}"), x as i32, y as i32, col as u8);
            }
            OP_BTN => {
                let n = self.pop()? as u8;
                let pressed = (self.buttons >> (n & 7)) & 1;
                self.push(pressed as i16)?;
            }
            OP_BTNP => {
                let n = self.pop()? as u8;
                let bit = n & 7;
                let cur = (self.buttons >> bit) & 1;
                let prev = (self.prev_buttons >> bit) & 1;
                self.push(if cur != 0 && prev == 0 { 1 } else { 0 })?;
            }
            OP_SFX => {
                let n = self.pop()?;
                if n >= 0 && (n as usize) < SFX_COUNT {
                    self.sfx_queue.push(n as u8);
                }
            }
            OP_MUSIC => {
                self.pop()?; // stub
            }
            OP_PEEK => {
                let addr = self.pop()? as u16 as usize;
                if addr >= MEMORY_SIZE {
                    return Err(VmError::OutOfBounds(format!("peek at {addr}")));
                }
                self.push(self.memory[addr] as i16)?;
            }
            OP_POKE => {
                let val = self.pop()?;
                let addr = self.pop()? as u16 as usize;
                if addr >= MEMORY_SIZE {
                    return Err(VmError::OutOfBounds(format!("poke at {addr}")));
                }
                self.memory[addr] = val as u8;
            }
            OP_SIN => {
                let x = self.pop()?;
                let rad = (x as f32) / FP_SCALE;
                self.push((rad.sin() * FP_SCALE) as i16)?;
            }
            OP_COS => {
                let x = self.pop()?;
                let rad = (x as f32) / FP_SCALE;
                self.push((rad.cos() * FP_SCALE) as i16)?;
            }
            OP_SQRT => {
                let x = self.pop()?;
                let result = if x <= 0 {
                    0
                } else {
                    ((x as f32 / FP_SCALE).sqrt() * FP_SCALE) as i16
                };
                self.push(result)?;
            }
            OP_ABS => {
                let x = self.pop()?;
                self.push(x.wrapping_abs())?;
            }
            OP_MIN => binop!(i16::min),
            OP_MAX => binop!(i16::max),
            OP_TRACEI => {
                let val = self.pop()?;
                self.trace_output.push(format!("{val}"));
            }
            OP_TRACES => {
                let str_idx = self.pop()? as u16 as usize;
                if str_idx < self.string_pool.len() {
                    self.trace_output.push(self.string_pool[str_idx].clone());
                }
            }
            OP_TIME => {
                let secs = (self.current_time - self.start_time) as i16;
                self.push(secs)?;
            }
            OP_RND => {
                let n = self.pop()?;
                if n <= 0 {
                    self.push(0)?;
                } else {
                    // xorshift32
                    self.rng_state ^= self.rng_state << 13;
                    self.rng_state ^= self.rng_state >> 17;
                    self.rng_state ^= self.rng_state << 5;
                    self.push((self.rng_state % n as u32) as i16)?;
                }
            }
            OP_EXP => {
                let x = self.pop()?;
                self.push(((x as f32 / FP_SCALE).exp() * FP_SCALE) as i16)?;
            }
            OP_LOG => {
                let x = self.pop()?;
                let result = if x <= 0 {
                    0
                } else {
                    ((x as f32 / FP_SCALE).ln() * FP_SCALE) as i16
                };
                self.push(result)?;
            }
            OP_POW => {
                let exp = self.pop()?;
                let base = self.pop()?;
                self.push(
                    ((base as f32 / FP_SCALE).powf(exp as f32 / FP_SCALE) * FP_SCALE) as i16,
                )?;
            }
            OP_ATAN2 => {
                let x = self.pop()?;
                let y = self.pop()?;
                self.push(((y as f32 / FP_SCALE).atan2(x as f32 / FP_SCALE) * FP_SCALE) as i16)?;
            }
            OP_FTOI => {
                let x = self.pop()?;
                self.push(x >> 7)?;
            }
            OP_ITOF => {
                let x = self.pop()?;
                self.push(x << 7)?;
            }
            OP_TRACEF => {
                let val = self.pop()?;
                self.trace_output.push(format_fixed(val));
            }
            OP_PRINTF => {
                let col = self.pop()?;
                let y = self.pop()?;
                let x = self.pop()?;
                let val = self.pop()?;
                let s = format_fixed(val);
                self.fb_print(&s, x as i32, y as i32, col as u8);
            }
            OP_FLIP => {
                self.prev_buttons = self.buttons;
                return Ok(VmResult::Flip);
            }
            OP_MGET => {
                let y = self.pop()?;
                let x = self.pop()?;
                let xu = x as u16 as usize;
                let yu = y as u16 as usize;
                if xu < MAP_WIDTH && yu < MAP_HEIGHT {
                    self.push(self.memory[MAP_REGION_START + yu * MAP_WIDTH + xu] as i16)?;
                } else {
                    self.push(0)?;
                }
            }
            OP_MSET => {
                let tile = self.pop()?;
                let y = self.pop()?;
                let x = self.pop()?;
                let xu = x as u16 as usize;
                let yu = y as u16 as usize;
                if xu < MAP_WIDTH && yu < MAP_HEIGHT {
                    self.memory[MAP_REGION_START + yu * MAP_WIDTH + xu] = tile as u8;
                }
            }
            OP_MAP => {
                let h = self.pop()?;
                let w = self.pop()?;
                let dy = self.pop()?;
                let dx = self.pop()?;
                let sy = self.pop()?;
                let sx = self.pop()?;
                self.fb_map(
                    sx as i32, sy as i32, dx as i32, dy as i32, w as i32, h as i32,
                );
            }
            _ => return Err(VmError::UnknownOpcode(op)),
        }
        Ok(VmResult::Continue)
    }

    pub fn run_until_flip(&mut self) -> Result<VmResult, VmError> {
        for _ in 0..CYCLE_LIMIT {
            match self.step()? {
                VmResult::Continue => {}
                other => return Ok(other),
            }
        }
        Err(VmError::CycleLimit)
    }

    // -- Framebuffer helpers --

    fn fb_pset(&mut self, x: i32, y: i32, col: u8) {
        if x >= 0 && x < FB_WIDTH as i32 && y >= 0 && y < FB_HEIGHT as i32 {
            self.framebuffer[y as usize * FB_WIDTH + x as usize] = col & 3;
        }
    }

    fn fb_pget(&self, x: i32, y: i32) -> u8 {
        if x >= 0 && x < FB_WIDTH as i32 && y >= 0 && y < FB_HEIGHT as i32 {
            self.framebuffer[y as usize * FB_WIDTH + x as usize]
        } else {
            0
        }
    }

    fn fb_line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, col: u8) {
        // Bresenham's line algorithm
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.fb_pset(x0, y0, col);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn fb_rect(&mut self, x: i32, y: i32, w: i32, h: i32, col: u8) {
        for py in y..y + h {
            for px in x..x + w {
                self.fb_pset(px, py, col);
            }
        }
    }

    fn fb_circ(&mut self, cx: i32, cy: i32, r: i32, col: u8) {
        // Midpoint circle algorithm (filled)
        let mut x = r;
        let mut y = 0;
        let mut err = 1 - r;

        while x >= y {
            // Fill horizontal spans for all 8 octants
            self.fb_hline(cx - x, cx + x, cy + y, col);
            self.fb_hline(cx - x, cx + x, cy - y, col);
            self.fb_hline(cx - y, cx + y, cy + x, col);
            self.fb_hline(cx - y, cx + y, cy - x, col);

            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    fn fb_hline(&mut self, x0: i32, x1: i32, y: i32, col: u8) {
        for px in x0..=x1 {
            self.fb_pset(px, y, col);
        }
    }

    fn fb_spr(&mut self, n: u16, x: i32, y: i32) {
        if n as usize >= SPRITE_COUNT {
            return;
        }
        let base = SPRITE_REGION_START + n as usize * SPRITE_SIZE;
        for row in 0..8i32 {
            let b0 = self.memory[base + row as usize * 2];
            let b1 = self.memory[base + row as usize * 2 + 1];
            for col in 0..4i32 {
                let c0 = (b0 >> (6 - col * 2)) & 3;
                if c0 != 0 {
                    self.fb_pset(x + col, y + row, c0);
                }
            }
            for col in 0..4i32 {
                let c1 = (b1 >> (6 - col * 2)) & 3;
                if c1 != 0 {
                    self.fb_pset(x + 4 + col, y + row, c1);
                }
            }
        }
    }

    fn fb_map(&mut self, sx: i32, sy: i32, dx: i32, dy: i32, w: i32, h: i32) {
        for ty in 0..h {
            for tx in 0..w {
                let mx = sx + tx;
                let my = sy + ty;
                if mx >= 0 && mx < MAP_WIDTH as i32 && my >= 0 && my < MAP_HEIGHT as i32 {
                    let tile =
                        self.memory[MAP_REGION_START + my as usize * MAP_WIDTH + mx as usize];
                    self.fb_spr(tile as u16, dx + tx * 8, dy + ty * 8);
                }
            }
        }
    }

    fn fb_print(&mut self, text: &str, mut x: i32, y: i32, color: u8) {
        for ch in text.chars() {
            let code = ch as u32;
            if (32..=127).contains(&code) {
                let idx = (code - 32) as usize;
                for row in 0..6 {
                    let byte = FONT_DATA[idx * 6 + row];
                    for col in 0..4 {
                        if (byte >> (7 - col)) & 1 != 0 {
                            self.fb_pset(x + col, y + row as i32, color);
                        }
                    }
                }
            }
            x += 4;
        }
    }
}

fn format_fixed(val: i16) -> String {
    let abs = (val as i32).abs();
    let int_part = abs >> 7;
    let frac_part = (abs & 0x7F) * 100 / 128;
    if val < 0 {
        format!("-{int_part}.{frac_part:02}")
    } else {
        format!("{int_part}.{frac_part:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::compile;

    /// Build a minimal Bytecode with one function (main) from raw bytes.
    fn make_bc(code: Vec<u8>, n_locals: u16) -> Bytecode {
        Bytecode {
            entry_point: Some(0),
            functions: vec![FuncInfo {
                code_offset: 0,
                n_params: 0,
                n_locals,
            }],
            string_pool: Vec::new(),
            code,
        }
    }

    #[test]
    fn halt_immediately() {
        let bc = make_bc(vec![OP_HALT], 0);
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        assert_eq!(vm.step().unwrap(), VmResult::Halted);
        assert!(vm.halted);
    }

    #[test]
    fn push_and_store() {
        // Push 42, store to slot 0, halt
        let bc = make_bc(vec![OP_PUSH_I8, 42, OP_STORE_LOCAL, 0, 0, OP_HALT], 1);
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 42);
    }

    #[test]
    fn arithmetic() {
        // Push 10, store 0, push 3, store 1, load 0, load 1, add, store 2, halt
        let bc = make_bc(
            vec![
                OP_PUSH_I8,
                10,
                OP_STORE_LOCAL,
                0,
                0,
                OP_PUSH_I8,
                3,
                OP_STORE_LOCAL,
                1,
                0,
                OP_LOAD_LOCAL,
                0,
                0,
                OP_LOAD_LOCAL,
                1,
                0,
                OP_ADD,
                OP_STORE_LOCAL,
                2,
                0,
                OP_HALT,
            ],
            3,
        );
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[2], 13);
    }

    #[test]
    fn division_by_zero() {
        let bc = make_bc(vec![OP_PUSH1, OP_PUSH0, OP_DIV], 0);
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        assert!(matches!(vm.run_until_flip(), Err(VmError::DivisionByZero)));
    }

    #[test]
    fn cls_fills_framebuffer() {
        // cls(2): push 2, OP_CLS, halt
        let bc = make_bc(vec![OP_PUSH_I8, 2, OP_CLS, OP_HALT], 0);
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert!(vm.framebuffer.iter().all(|&p| p == 2));
    }

    #[test]
    fn flip_yields() {
        let bc = make_bc(vec![OP_FLIP, OP_HALT], 0);
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        assert_eq!(vm.step().unwrap(), VmResult::Flip);
        assert!(!vm.halted);
        assert_eq!(vm.step().unwrap(), VmResult::Halted);
    }

    #[test]
    fn pset_pget() {
        // pset(5, 3, 3), pget(5, 3) -> store slot 0, halt
        let bc = make_bc(
            vec![
                OP_PUSH_I8,
                5,
                OP_PUSH_I8,
                3,
                OP_PUSH_I8,
                3,
                OP_PSET,
                OP_PUSH_I8,
                5,
                OP_PUSH_I8,
                3,
                OP_PGET,
                OP_STORE_LOCAL,
                0,
                0,
                OP_HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 3);
        assert_eq!(vm.framebuffer[3 * FB_WIDTH + 5], 3);
    }

    #[test]
    fn jump_if_false() {
        // Push 0 (false), jump_if_false +N, push_i8 99 (skipped), store_local 0, halt
        // On false: skip push_i8(99) [2 bytes] + store_local(0) [3 bytes] = 5 bytes
        let bc = make_bc(
            vec![
                OP_PUSH0,
                OP_JUMP_IF_FALSE,
                5,
                0, // offset +5 (skip next 5 bytes)
                OP_PUSH_I8,
                99,
                OP_STORE_LOCAL,
                0,
                0,
                OP_HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        // Slot 0 should still be 0 (the push_i8 99 was skipped)
        assert_eq!(vm.locals[0], 0);
    }

    #[test]
    fn ret_from_main_halts() {
        let bc = make_bc(vec![OP_RET], 0);
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        assert_eq!(vm.step().unwrap(), VmResult::Halted);
    }

    #[test]
    fn btn_reads_buttons() {
        // btn(4): push 4, OP_BTN, store 0, halt
        let bc = make_bc(
            vec![OP_PUSH_I8, 4, OP_BTN, OP_STORE_LOCAL, 0, 0, OP_HALT],
            1,
        );
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.buttons = 0b0001_0000; // bit 4 set
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 1);
    }

    #[test]
    fn peek_poke() {
        // poke(100, 42), peek(100) -> store 0, halt
        let bc = make_bc(
            vec![
                OP_PUSH_I8,
                100,
                OP_PUSH_I8,
                42,
                OP_POKE,
                OP_PUSH_I8,
                100,
                OP_PEEK,
                OP_STORE_LOCAL,
                0,
                0,
                OP_HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 42);
        assert_eq!(vm.memory[100], 42);
    }

    #[test]
    fn memory_store_load() {
        // Store2 500 at addr 200, Load2 from addr 200 -> slot 0
        // push addr 200, push val 500, OP_STORE2
        // push addr 200, OP_LOAD2, store slot 0, halt
        let bc = make_bc(
            vec![
                OP_PUSH_I16,
                200u8,
                0, // addr=200
                OP_PUSH_I16,
                0xF4u8,
                0x01, // val=500
                OP_STORE2,
                OP_PUSH_I16,
                200u8,
                0,
                OP_LOAD2,
                OP_STORE_LOCAL,
                0,
                0,
                OP_HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 500);
    }

    #[test]
    fn function_call() {
        // Two functions: add(a, b) at offset 0, main at offset N
        // add: load 0, load 1, OP_ADD, OP_RET  (returns a+b on stack)
        // main: push 10, push 20, OP_CALL #0 (2 args), store 0, OP_HALT
        let add_code = vec![OP_LOAD_LOCAL, 0, 0, OP_LOAD_LOCAL, 1, 0, OP_ADD, OP_RET];
        let add_len = add_code.len();
        let main_offset = add_len;

        let mut code = add_code;
        code.extend_from_slice(&[
            OP_PUSH_I8,
            10,
            OP_PUSH_I8,
            20,
            OP_CALL,
            0,
            0, // func_idx=0
            2, // argc=2
            OP_STORE_LOCAL,
            0,
            0,
            OP_HALT,
        ]);

        let bc = Bytecode {
            entry_point: Some(main_offset),
            functions: vec![
                FuncInfo {
                    code_offset: 0,
                    n_params: 2,
                    n_locals: 2,
                },
                FuncInfo {
                    code_offset: main_offset,
                    n_params: 0,
                    n_locals: 1,
                },
            ],
            string_pool: Vec::new(),
            code,
        };

        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        // main's locals start at index 0, slot 0 should have 30
        assert_eq!(vm.locals[0], 30);
    }

    #[test]
    fn nested_while_compiled() {
        // Compile and run a nested while loop to verify outer variable increments
        let src = "fn main()\n  x: int = 0\n  while x < 3\n    y: int = 0\n    while y < 3\n      pset(x, y, 3)\n      y = y + 1\n    end\n    x = x + 1\n  end\n  flip()\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        let result = vm.run_until_flip().unwrap();
        assert_eq!(result, VmResult::Flip);
        // Should have 9 white pixels: (0,0)..(2,2)
        for x in 0..3 {
            for y in 0..3 {
                assert_eq!(
                    vm.framebuffer[y * FB_WIDTH + x],
                    3,
                    "expected pixel at ({x},{y})"
                );
            }
        }
        // Pixel at (3,0) should OP_NOT be set
        assert_eq!(vm.framebuffer[3], 0);
    }

    #[test]
    fn fixed_mul_no_overflow() {
        // cos(0.01) * 1.00 should be ~0.99, not -1.00
        let src = "fn main()\n  x = cos(0.01)\n  y = x * 1.00\n  tracef(y)\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.trace_output.len(), 1);
        assert_eq!(vm.trace_output[0], "0.99");
    }

    #[test]
    fn fmul_opcode() {
        // 1.5 * 2.0 = 3.0 (192 * 256 >> 7 = 384)
        let bc = make_bc(
            vec![
                OP_PUSH_I16,
                0xC0,
                0x00,
                OP_PUSH_I16,
                0x00,
                0x01,
                OP_FMUL,
                OP_STORE_LOCAL,
                0,
                0,
                OP_HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 384); // 3.0 in 9.7
    }

    #[test]
    fn bxor_opcode() {
        let bc = make_bc(
            vec![
                OP_PUSH_I8,
                0b1100,
                OP_PUSH_I8,
                0b1010,
                OP_BXOR,
                OP_STORE_LOCAL,
                0,
                0,
                OP_HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 0b0110);
    }

    #[test]
    fn fdiv_opcode() {
        // 3.0 / 1.5 = 2.0 (384 << 7 / 192 = 256)
        let bc = make_bc(
            vec![
                OP_PUSH_I16,
                0x80,
                0x01,
                OP_PUSH_I16,
                0xC0,
                0x00,
                OP_FDIV,
                OP_STORE_LOCAL,
                0,
                0,
                OP_HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 256); // 2.0 in 9.7
    }

    #[test]
    fn struct_return_field_access() {
        let src = "fn main()\n  printi(g().x, 55, 70, 3)\nend\n\nstruct F\n  x: int\nend\n\nfn g(): F\n  f: F\n  return f\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
    }

    #[test]
    fn compound_return_value_assignment() {
        let src = "fn new_list(): array[10] of int\n  a: array[10] of int\n  a[0] = 12\n  return a\nend\n\nfn main()\n  a = new_list()\n  tracei(a[0])\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.trace_output[0], "12");
    }

    #[test]
    fn spr_renders_pixels() {
        // Manually poke sprite 0 data and call spr(0, 10, 20)
        let src =
            "fn main()\n  poke(12288, 0xFF)\n  poke(12289, 0x00)\n  spr(0, 10, 20)\n  flip()\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        let result = vm.run_until_flip().unwrap();
        assert_eq!(result, VmResult::Flip);
        // First byte 0xFF = four pixels of color 3 (columns 0-3, row 0)
        assert_eq!(vm.framebuffer[20 * FB_WIDTH + 10], 3); // (10,20) = color 3
        assert_eq!(vm.framebuffer[20 * FB_WIDTH + 11], 3); // (11,20) = color 3
        assert_eq!(vm.framebuffer[20 * FB_WIDTH + 12], 3); // (12,20) = color 3
        assert_eq!(vm.framebuffer[20 * FB_WIDTH + 13], 3); // (13,20) = color 3
                                                           // Second byte 0x00 = four transparent pixels (columns 4-7)
        assert_eq!(vm.framebuffer[20 * FB_WIDTH + 14], 0); // (14,20) = transparent
    }

    #[test]
    fn spr_transparency() {
        // Color 0 should not overwrite existing pixels
        let src = "fn main()\n  cls(0)\n  pset(10, 20, 2)\n  poke(12288, 0x00)\n  spr(0, 10, 20)\n  flip()\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        // pset drew color 2, sprite has color 0, so should not overwrite
        assert_eq!(vm.framebuffer[20 * FB_WIDTH + 10], 2);
    }

    #[test]
    fn ref_scalar_write_through() {
        // fn inc(x: ref int) writes through to modify caller's local
        let src = "fn inc(x: ref int)\n  x = x + 1\nend\n\nfn main()\n  a = 10\n  inc(ref a)\n  tracei(a)\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.trace_output[0], "11");
    }

    #[test]
    fn ref_struct_field_mutation() {
        let src = "\
struct Vec2
  x: int
  y: int
end

fn add(a: ref Vec2, b: ref Vec2)
  a.x = a.x + b.x
  a.y = a.y + b.y
end

fn main()
  a: Vec2
  a.x = 3
  a.y = 4
  b: Vec2
  b.x = 10
  b.y = 20
  add(ref a, ref b)
  tracei(a.x)
  tracei(a.y)
end";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.trace_output[0], "13");
        assert_eq!(vm.trace_output[1], "24");
    }

    #[test]
    fn ref_forwarding() {
        // Forward a ref param to another function
        let src = "\
fn set_val(x: ref int)
  x = 99
end

fn wrapper(x: ref int)
  set_val(x)
end

fn main()
  v = 0
  wrapper(ref v)
  tracei(v)
end";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.trace_output[0], "99");
    }

    #[test]
    fn ref_local_binding() {
        // p = ref v creates a local ref that can modify v
        let src = "\
struct Vec2
  x: int
  y: int
end

v: Vec2

fn main()
  v.x = 5
  p = ref v
  p.x = 42
  tracei(v.x)
end";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.trace_output[0], "42");
    }

    #[test]
    fn compound_param_copy_semantics() {
        // Without ref, struct params are copied, so callee can't mutate caller
        let src = "\
struct Vec2
  x: int
  y: int
end

fn try_modify(v: Vec2)
  v.x = 999
end

fn main()
  v: Vec2
  v.x = 5
  try_modify(v)
  tracei(v.x)
end";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.trace_output[0], "5");
    }

    #[test]
    fn ref_swap() {
        let src = "\
fn swap(a: ref int, b: ref int)
  tmp = a
  a = b
  b = tmp
end

fn main()
  x = 10
  y = 20
  swap(ref x, ref y)
  tracei(x)
  tracei(y)
end";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.trace_output[0], "20");
        assert_eq!(vm.trace_output[1], "10");
    }

    #[test]
    fn nested_for_in_wildcard_indices() {
        let src = "\
n: int
outer: array[3] of int
inner: array[3] of int
fn main()
  for _, x in outer
    for _, y in inner
      n = n + 1
    end
  end
  tracei(n)
end";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.trace_output[0], "9");
    }
}
