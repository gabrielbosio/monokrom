use crate::compiler::bytecode::*;
use crate::render::font::FONT_DATA;

const STACK_LIMIT: usize = 256;
const CALL_STACK_LIMIT: usize = 64;
const MEMORY_SIZE: usize = 16384;
const FB_WIDTH: usize = 160;
const FB_HEIGHT: usize = 144;
const FB_SIZE: usize = FB_WIDTH * FB_HEIGHT;
const CYCLE_LIMIT: u32 = 100_000;

#[derive(Debug)]
pub enum VmError {
    StackOverflow,
    StackUnderflow,
    DivisionByZero,
    OutOfBounds(String),
    NoEntryPoint,
    CallStackOverflow,
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
    n_locals: u8,
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
}

impl Vm {
    pub fn new(bc: &Bytecode) -> Result<Self, VmError> {
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

    fn load_local(&self, slot: u8) -> i16 {
        self.locals[self.locals_base() + slot as usize]
    }

    fn store_local(&mut self, slot: u8, val: i16) {
        let base = self.locals_base();
        self.locals[base + slot as usize] = val;
    }

    pub fn step(&mut self) -> Result<VmResult, VmError> {
        if self.halted {
            return Ok(VmResult::Halted);
        }

        let op = self.read_u8();
        match op {
            PUSH0 => self.push(0)?,
            PUSH1 => self.push(1)?,
            PUSH_I8 => {
                let v = self.read_i8() as i16;
                self.push(v)?;
            }
            PUSH_I16 => {
                let v = self.read_i16();
                self.push(v)?;
            }
            POP => {
                self.pop()?;
            }
            LOAD_LOCAL => {
                let slot = self.read_u8();
                let v = self.load_local(slot);
                self.push(v)?;
            }
            STORE_LOCAL => {
                let slot = self.read_u8();
                let v = self.pop()?;
                self.store_local(slot, v);
            }
            ADD => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a.wrapping_add(b))?;
            }
            SUB => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a.wrapping_sub(b))?;
            }
            MUL => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a.wrapping_mul(b))?;
            }
            DIV => {
                let b = self.pop()?;
                let a = self.pop()?;
                if b == 0 {
                    return Err(VmError::DivisionByZero);
                }
                self.push(a.wrapping_div(b))?;
            }
            MOD => {
                let b = self.pop()?;
                let a = self.pop()?;
                if b == 0 {
                    return Err(VmError::DivisionByZero);
                }
                self.push(a.wrapping_rem(b))?;
            }
            NEG => {
                let a = self.pop()?;
                self.push(a.wrapping_neg())?;
            }
            EQ => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a == b { 1 } else { 0 })?;
            }
            NEQ => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a != b { 1 } else { 0 })?;
            }
            LT => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a < b { 1 } else { 0 })?;
            }
            GT => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a > b { 1 } else { 0 })?;
            }
            LEQ => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a <= b { 1 } else { 0 })?;
            }
            GEQ => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a >= b { 1 } else { 0 })?;
            }
            AND => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a != 0 && b != 0 { 1 } else { 0 })?;
            }
            OR => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(if a != 0 || b != 0 { 1 } else { 0 })?;
            }
            NOT => {
                let a = self.pop()?;
                self.push(if a == 0 { 1 } else { 0 })?;
            }
            LOAD1 => {
                let addr = self.pop()? as u16 as usize;
                if addr >= MEMORY_SIZE {
                    return Err(VmError::OutOfBounds(format!("memory read at {addr}")));
                }
                self.push(self.memory[addr] as i16)?;
            }
            LOAD2 => {
                let addr = self.pop()? as u16 as usize;
                if addr + 1 >= MEMORY_SIZE {
                    return Err(VmError::OutOfBounds(format!("memory read at {addr}")));
                }
                let v = i16::from_le_bytes([self.memory[addr], self.memory[addr + 1]]);
                self.push(v)?;
            }
            STORE1 => {
                let val = self.pop()?;
                let addr = self.pop()? as u16 as usize;
                if addr >= MEMORY_SIZE {
                    return Err(VmError::OutOfBounds(format!("memory write at {addr}")));
                }
                self.memory[addr] = val as u8;
            }
            STORE2 => {
                let val = self.pop()?;
                let addr = self.pop()? as u16 as usize;
                if addr + 1 >= MEMORY_SIZE {
                    return Err(VmError::OutOfBounds(format!("memory write at {addr}")));
                }
                let bytes = val.to_le_bytes();
                self.memory[addr] = bytes[0];
                self.memory[addr + 1] = bytes[1];
            }
            JUMP => {
                let offset = self.read_i16();
                self.pc = (self.pc as isize + offset as isize) as usize;
            }
            JUMP_IF_FALSE => {
                let offset = self.read_i16();
                let cond = self.pop()?;
                if cond == 0 {
                    self.pc = (self.pc as isize + offset as isize) as usize;
                }
            }
            CALL => {
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
            RET => {
                let frame = self.call_stack.pop().unwrap();
                if frame.return_pc == usize::MAX {
                    self.halted = true;
                    return Ok(VmResult::Halted);
                }
                // Truncate locals back to caller's frame
                self.locals.truncate(frame.locals_base);
                self.pc = frame.return_pc;
            }
            HALT => {
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
                // stub: pop 3 args, no-op
                self.pop()?;
                self.pop()?;
                self.pop()?;
            }
            OP_PRINT => {
                let y = self.pop()?;
                let x = self.pop()?;
                let str_idx = self.pop()? as u16 as usize;
                if str_idx < self.string_pool.len() {
                    let s = self.string_pool[str_idx].clone();
                    self.fb_print(&s, x as i32, y as i32);
                }
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
                self.pop()?; // stub
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
                let rad = (x as f32) / 256.0;
                let result = (rad.sin() * 256.0) as i16;
                self.push(result)?;
            }
            OP_COS => {
                let x = self.pop()?;
                let rad = (x as f32) / 256.0;
                let result = (rad.cos() * 256.0) as i16;
                self.push(result)?;
            }
            OP_SQRT => {
                let x = self.pop()?;
                let result = if x <= 0 {
                    0
                } else {
                    ((x as f32).sqrt() * 256.0) as i16
                };
                self.push(result)?;
            }
            OP_ABS => {
                let x = self.pop()?;
                self.push(x.wrapping_abs())?;
            }
            OP_MIN => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a.min(b))?;
            }
            OP_MAX => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(a.max(b))?;
            }
            OP_FLIP => {
                self.prev_buttons = self.buttons;
                return Ok(VmResult::Flip);
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

    fn fb_print(&mut self, text: &str, mut x: i32, y: i32) {
        for ch in text.chars() {
            let code = ch as u32;
            if (32..=127).contains(&code) {
                let idx = (code - 32) as usize;
                for row in 0..6 {
                    let byte = FONT_DATA[idx * 6 + row];
                    for col in 0..4 {
                        if (byte >> (7 - col)) & 1 != 0 {
                            self.fb_pset(x + col, y + row as i32, 3);
                        }
                    }
                }
            }
            x += 4;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal Bytecode with one function (main) from raw bytes.
    fn make_bc(code: Vec<u8>, n_locals: u8) -> Bytecode {
        Bytecode {
            entry_point: Some(0),
            functions: vec![FuncInfo {
                name: "main".into(),
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
        let bc = make_bc(vec![HALT], 0);
        let mut vm = Vm::new(&bc).unwrap();
        assert_eq!(vm.step().unwrap(), VmResult::Halted);
        assert!(vm.halted);
    }

    #[test]
    fn push_and_store() {
        // Push 42, store to slot 0, halt
        let bc = make_bc(vec![PUSH_I8, 42, STORE_LOCAL, 0, HALT], 1);
        let mut vm = Vm::new(&bc).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 42);
    }

    #[test]
    fn arithmetic() {
        // Push 10, store 0, push 3, store 1, load 0, load 1, add, store 2, halt
        let bc = make_bc(
            vec![
                PUSH_I8,
                10,
                STORE_LOCAL,
                0,
                PUSH_I8,
                3,
                STORE_LOCAL,
                1,
                LOAD_LOCAL,
                0,
                LOAD_LOCAL,
                1,
                ADD,
                STORE_LOCAL,
                2,
                HALT,
            ],
            3,
        );
        let mut vm = Vm::new(&bc).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[2], 13);
    }

    #[test]
    fn division_by_zero() {
        let bc = make_bc(vec![PUSH1, PUSH0, DIV], 0);
        let mut vm = Vm::new(&bc).unwrap();
        assert!(matches!(vm.run_until_flip(), Err(VmError::DivisionByZero)));
    }

    #[test]
    fn cls_fills_framebuffer() {
        // cls(2): push 2, OP_CLS, halt
        let bc = make_bc(vec![PUSH_I8, 2, OP_CLS, HALT], 0);
        let mut vm = Vm::new(&bc).unwrap();
        vm.run_until_flip().unwrap();
        assert!(vm.framebuffer.iter().all(|&p| p == 2));
    }

    #[test]
    fn flip_yields() {
        let bc = make_bc(vec![OP_FLIP, HALT], 0);
        let mut vm = Vm::new(&bc).unwrap();
        assert_eq!(vm.step().unwrap(), VmResult::Flip);
        assert!(!vm.halted);
        assert_eq!(vm.step().unwrap(), VmResult::Halted);
    }

    #[test]
    fn pset_pget() {
        // pset(5, 3, 3), pget(5, 3) -> store slot 0, halt
        let bc = make_bc(
            vec![
                PUSH_I8,
                5,
                PUSH_I8,
                3,
                PUSH_I8,
                3,
                OP_PSET,
                PUSH_I8,
                5,
                PUSH_I8,
                3,
                OP_PGET,
                STORE_LOCAL,
                0,
                HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 3);
        assert_eq!(vm.framebuffer[3 * FB_WIDTH + 5], 3);
    }

    #[test]
    fn jump_if_false() {
        // Push 0 (false), jump_if_false +3, push_i8 99 (skipped), store_local 0, halt
        // On false: skip the push_i8 99 + store_local 0 (4 bytes)
        // After JUMP_IF_FALSE + 2-byte offset, pc is at the push_i8 instruction
        // We want to skip push_i8(99) [2 bytes] + store_local(0) [2 bytes] = 4 bytes
        let bc = make_bc(
            vec![
                PUSH0,
                JUMP_IF_FALSE,
                4,
                0, // offset +4 (skip next 4 bytes)
                PUSH_I8,
                99,
                STORE_LOCAL,
                0,
                HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc).unwrap();
        vm.run_until_flip().unwrap();
        // Slot 0 should still be 0 (the push_i8 99 was skipped)
        assert_eq!(vm.locals[0], 0);
    }

    #[test]
    fn ret_from_main_halts() {
        let bc = make_bc(vec![RET], 0);
        let mut vm = Vm::new(&bc).unwrap();
        assert_eq!(vm.step().unwrap(), VmResult::Halted);
    }

    #[test]
    fn btn_reads_buttons() {
        // btn(4): push 4, OP_BTN, store 0, halt
        let bc = make_bc(vec![PUSH_I8, 4, OP_BTN, STORE_LOCAL, 0, HALT], 1);
        let mut vm = Vm::new(&bc).unwrap();
        vm.buttons = 0b0001_0000; // bit 4 set
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 1);
    }

    #[test]
    fn peek_poke() {
        // poke(100, 42), peek(100) -> store 0, halt
        let bc = make_bc(
            vec![
                PUSH_I8,
                100,
                PUSH_I8,
                42,
                OP_POKE,
                PUSH_I8,
                100,
                OP_PEEK,
                STORE_LOCAL,
                0,
                HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 42);
        assert_eq!(vm.memory[100], 42);
    }

    #[test]
    fn memory_store_load() {
        // Store2 500 at addr 200, Load2 from addr 200 -> slot 0
        // push addr 200, push val 500, STORE2
        // push addr 200, LOAD2, store slot 0, halt
        let bc = make_bc(
            vec![
                PUSH_I16,
                200u8,
                0, // addr=200
                PUSH_I16,
                0xF4u8,
                0x01, // val=500
                STORE2,
                PUSH_I16,
                200u8,
                0,
                LOAD2,
                STORE_LOCAL,
                0,
                HALT,
            ],
            1,
        );
        let mut vm = Vm::new(&bc).unwrap();
        vm.run_until_flip().unwrap();
        assert_eq!(vm.locals[0], 500);
    }

    #[test]
    fn function_call() {
        // Two functions: add(a, b) at offset 0, main at offset N
        // add: load 0, load 1, ADD, RET  (returns a+b on stack)
        // main: push 10, push 20, CALL #0 (2 args), store 0, HALT
        let add_code = vec![LOAD_LOCAL, 0, LOAD_LOCAL, 1, ADD, RET];
        let add_len = add_code.len();
        let main_offset = add_len;

        let mut code = add_code;
        code.extend_from_slice(&[
            PUSH_I8,
            10,
            PUSH_I8,
            20,
            CALL,
            0,
            0, // func_idx=0
            2, // argc=2
            STORE_LOCAL,
            0,
            HALT,
        ]);

        let bc = Bytecode {
            entry_point: Some(main_offset),
            functions: vec![
                FuncInfo {
                    name: "add".into(),
                    code_offset: 0,
                    n_params: 2,
                    n_locals: 2,
                },
                FuncInfo {
                    name: "main".into(),
                    code_offset: main_offset,
                    n_params: 0,
                    n_locals: 1,
                },
            ],
            string_pool: Vec::new(),
            code,
        };

        let mut vm = Vm::new(&bc).unwrap();
        vm.run_until_flip().unwrap();
        // main's locals start at index 0, slot 0 should have 30
        assert_eq!(vm.locals[0], 30);
    }
}
