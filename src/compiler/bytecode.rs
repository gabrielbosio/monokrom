use crate::compiler::hir::Intrinsic;

// Opcode constants
pub const PUSH0: u8 = 0x00;
pub const PUSH1: u8 = 0x01;
pub const PUSH_I8: u8 = 0x02;
pub const PUSH_I16: u8 = 0x03;
pub const POP: u8 = 0x04;
pub const LOAD_LOCAL: u8 = 0x05;
pub const STORE_LOCAL: u8 = 0x06;

pub const ADD: u8 = 0x10;
pub const SUB: u8 = 0x11;
pub const MUL: u8 = 0x12;
pub const DIV: u8 = 0x13;
pub const MOD: u8 = 0x14;
pub const NEG: u8 = 0x15;

pub const EQ: u8 = 0x16;
pub const NEQ: u8 = 0x17;
pub const LT: u8 = 0x18;
pub const GT: u8 = 0x19;
pub const LEQ: u8 = 0x1A;
pub const GEQ: u8 = 0x1B;

pub const AND: u8 = 0x1C;
pub const OR: u8 = 0x1D;
pub const NOT: u8 = 0x1E;

pub const BAND: u8 = 0x64;
pub const BOR: u8 = 0x65;
pub const BNOT: u8 = 0x66;
pub const SHL: u8 = 0x67;
pub const SHR: u8 = 0x68;

pub const LOAD1: u8 = 0x20;
pub const LOAD2: u8 = 0x21;
pub const STORE1: u8 = 0x22;
pub const STORE2: u8 = 0x23;

pub const JUMP: u8 = 0x30;
pub const JUMP_IF_FALSE: u8 = 0x31;
pub const CALL: u8 = 0x32;
pub const RET: u8 = 0x33;
pub const HALT: u8 = 0x34;

pub const OP_PSET: u8 = 0x40;
pub const OP_PGET: u8 = 0x41;
pub const OP_CLS: u8 = 0x42;
pub const OP_LINE: u8 = 0x43;
pub const OP_RECT: u8 = 0x44;
pub const OP_CIRC: u8 = 0x45;
pub const OP_SPR: u8 = 0x46;
pub const OP_PRINTS: u8 = 0x47;
pub const OP_BTN: u8 = 0x48;
pub const OP_BTNP: u8 = 0x49;
pub const OP_SFX: u8 = 0x4A;
pub const OP_MUSIC: u8 = 0x4B;
pub const OP_PEEK: u8 = 0x4C;
pub const OP_POKE: u8 = 0x4D;
pub const OP_SIN: u8 = 0x4E;
pub const OP_COS: u8 = 0x4F;
pub const OP_SQRT: u8 = 0x50;
pub const OP_ABS: u8 = 0x51;
pub const OP_MIN: u8 = 0x52;
pub const OP_MAX: u8 = 0x53;
pub const OP_FLIP: u8 = 0x54;
pub const OP_PRINTI: u8 = 0x55;
pub const OP_TRACEI: u8 = 0x56;
pub const OP_TRACES: u8 = 0x57;
pub const OP_TIME: u8 = 0x58;
pub const OP_RND: u8 = 0x59;
pub const OP_EXP: u8 = 0x5A;
pub const OP_LOG: u8 = 0x5B;
pub const OP_POW: u8 = 0x5C;
pub const OP_ATAN2: u8 = 0x5D;
pub const OP_FTOI: u8 = 0x5E;
pub const OP_ITOF: u8 = 0x5F;
pub const OP_TRACEF: u8 = 0x60;
pub const OP_PRINTF: u8 = 0x61;
pub const FMUL: u8 = 0x62;
pub const FDIV: u8 = 0x63;

pub fn inst_size(op: u8) -> usize {
    match op {
        PUSH_I8 => 2,
        PUSH_I16 | LOAD_LOCAL | STORE_LOCAL | JUMP | JUMP_IF_FALSE => 3,
        CALL => 4,
        _ => 1,
    }
}

pub fn intrinsic_opcode(op: Intrinsic) -> u8 {
    match op {
        Intrinsic::Pset => OP_PSET,
        Intrinsic::Pget => OP_PGET,
        Intrinsic::Cls => OP_CLS,
        Intrinsic::Line => OP_LINE,
        Intrinsic::Rect => OP_RECT,
        Intrinsic::Circ => OP_CIRC,
        Intrinsic::Spr => OP_SPR,
        Intrinsic::Prints => OP_PRINTS,
        Intrinsic::Printi => OP_PRINTI,
        Intrinsic::Btn => OP_BTN,
        Intrinsic::Btnp => OP_BTNP,
        Intrinsic::Sfx => OP_SFX,
        Intrinsic::Music => OP_MUSIC,
        Intrinsic::Peek => OP_PEEK,
        Intrinsic::Poke => OP_POKE,
        Intrinsic::Sin => OP_SIN,
        Intrinsic::Cos => OP_COS,
        Intrinsic::Sqrt => OP_SQRT,
        Intrinsic::Abs => OP_ABS,
        Intrinsic::Min => OP_MIN,
        Intrinsic::Max => OP_MAX,
        Intrinsic::Flip => OP_FLIP,
        Intrinsic::Tracei => OP_TRACEI,
        Intrinsic::Traces => OP_TRACES,
        Intrinsic::Time => OP_TIME,
        Intrinsic::Rnd => OP_RND,
        Intrinsic::Exp => OP_EXP,
        Intrinsic::Log => OP_LOG,
        Intrinsic::Pow => OP_POW,
        Intrinsic::Atan2 => OP_ATAN2,
        Intrinsic::Ftoi => OP_FTOI,
        Intrinsic::Itof => OP_ITOF,
        Intrinsic::Tracef => OP_TRACEF,
        Intrinsic::Printf => OP_PRINTF,
    }
}

pub fn intrinsic_has_return_value(op: Intrinsic) -> bool {
    matches!(
        op,
        Intrinsic::Pget
            | Intrinsic::Btn
            | Intrinsic::Btnp
            | Intrinsic::Peek
            | Intrinsic::Sin
            | Intrinsic::Cos
            | Intrinsic::Sqrt
            | Intrinsic::Abs
            | Intrinsic::Min
            | Intrinsic::Max
            | Intrinsic::Time
            | Intrinsic::Rnd
            | Intrinsic::Exp
            | Intrinsic::Log
            | Intrinsic::Pow
            | Intrinsic::Atan2
            | Intrinsic::Ftoi
            | Intrinsic::Itof
    )
}

#[cfg(test)]
fn op_name(b: u8) -> Option<&'static str> {
    match b {
        PUSH0 => Some("Push0"),
        PUSH1 => Some("Push1"),
        PUSH_I8 => Some("PushI8"),
        PUSH_I16 => Some("PushI16"),
        POP => Some("Pop"),
        LOAD_LOCAL => Some("LoadLocal"),
        STORE_LOCAL => Some("StoreLocal"),
        ADD => Some("Add"),
        SUB => Some("Sub"),
        MUL => Some("Mul"),
        DIV => Some("Div"),
        MOD => Some("Mod"),
        NEG => Some("Neg"),
        EQ => Some("Eq"),
        NEQ => Some("Neq"),
        LT => Some("Lt"),
        GT => Some("Gt"),
        LEQ => Some("Leq"),
        GEQ => Some("Geq"),
        AND => Some("And"),
        OR => Some("Or"),
        NOT => Some("Not"),
        LOAD1 => Some("Load1"),
        LOAD2 => Some("Load2"),
        STORE1 => Some("Store1"),
        STORE2 => Some("Store2"),
        JUMP => Some("Jump"),
        JUMP_IF_FALSE => Some("JumpIfFalse"),
        CALL => Some("Call"),
        RET => Some("Ret"),
        HALT => Some("Halt"),
        OP_PSET => Some("Pset"),
        OP_PGET => Some("Pget"),
        OP_CLS => Some("Cls"),
        OP_LINE => Some("Line"),
        OP_RECT => Some("Rect"),
        OP_CIRC => Some("Circ"),
        OP_SPR => Some("Spr"),
        OP_PRINTS => Some("Prints"),
        OP_PRINTI => Some("Printi"),
        OP_BTN => Some("Btn"),
        OP_BTNP => Some("Btnp"),
        OP_SFX => Some("Sfx"),
        OP_MUSIC => Some("Music"),
        OP_PEEK => Some("Peek"),
        OP_POKE => Some("Poke"),
        OP_SIN => Some("Sin"),
        OP_COS => Some("Cos"),
        OP_SQRT => Some("Sqrt"),
        OP_ABS => Some("Abs"),
        OP_MIN => Some("Min"),
        OP_MAX => Some("Max"),
        OP_FLIP => Some("Flip"),
        OP_TRACEI => Some("Tracei"),
        OP_TRACES => Some("Traces"),
        OP_TIME => Some("Time"),
        OP_RND => Some("Rnd"),
        OP_EXP => Some("Exp"),
        OP_LOG => Some("Log"),
        OP_POW => Some("Pow"),
        OP_ATAN2 => Some("Atan2"),
        OP_FTOI => Some("Ftoi"),
        OP_ITOF => Some("Itof"),
        OP_TRACEF => Some("Tracef"),
        OP_PRINTF => Some("Printf"),
        FMUL => Some("Fmul"),
        FDIV => Some("Fdiv"),
        BAND => Some("Band"),
        BOR => Some("Bor"),
        BNOT => Some("Bnot"),
        SHL => Some("Shl"),
        SHR => Some("Shr"),
        _ => None,
    }
}

#[cfg(test)]
const ALL_OPCODES: [u8; 72] = [
    PUSH0,
    PUSH1,
    PUSH_I8,
    PUSH_I16,
    POP,
    LOAD_LOCAL,
    STORE_LOCAL,
    ADD,
    SUB,
    MUL,
    DIV,
    MOD,
    NEG,
    EQ,
    NEQ,
    LT,
    GT,
    LEQ,
    GEQ,
    AND,
    OR,
    NOT,
    LOAD1,
    LOAD2,
    STORE1,
    STORE2,
    JUMP,
    JUMP_IF_FALSE,
    CALL,
    RET,
    HALT,
    OP_PSET,
    OP_PGET,
    OP_CLS,
    OP_LINE,
    OP_RECT,
    OP_CIRC,
    OP_SPR,
    OP_PRINTS,
    OP_PRINTI,
    OP_BTN,
    OP_BTNP,
    OP_SFX,
    OP_MUSIC,
    OP_PEEK,
    OP_POKE,
    OP_SIN,
    OP_COS,
    OP_SQRT,
    OP_ABS,
    OP_MIN,
    OP_MAX,
    OP_FLIP,
    OP_TRACEI,
    OP_TRACES,
    OP_TIME,
    OP_RND,
    OP_EXP,
    OP_LOG,
    OP_POW,
    OP_ATAN2,
    OP_FTOI,
    OP_ITOF,
    OP_TRACEF,
    OP_PRINTF,
    FMUL,
    FDIV,
    BAND,
    BOR,
    BNOT,
    SHL,
    SHR,
];

#[derive(Debug)]
pub struct FuncInfo {
    pub code_offset: usize,
    pub n_params: u8,
    pub n_locals: u16,
}

#[derive(Debug)]
pub struct Bytecode {
    pub code: Vec<u8>,
    pub string_pool: Vec<String>,
    pub functions: Vec<FuncInfo>,
    pub entry_point: Option<usize>,
}

impl Bytecode {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            string_pool: Vec::new(),
            functions: Vec::new(),
            entry_point: None,
        }
    }

    pub fn emit_op(&mut self, op: u8) {
        self.code.push(op);
    }

    pub fn emit_i8(&mut self, v: i8) {
        self.code.push(v as u8);
    }

    pub fn emit_u8(&mut self, v: u8) {
        self.code.push(v);
    }

    pub fn emit_i16(&mut self, v: i16) {
        let bytes = v.to_le_bytes();
        self.code.push(bytes[0]);
        self.code.push(bytes[1]);
    }

    pub fn emit_u16(&mut self, v: u16) {
        let bytes = v.to_le_bytes();
        self.code.push(bytes[0]);
        self.code.push(bytes[1]);
    }

    pub fn patch_i16(&mut self, offset: usize, v: i16) {
        let bytes = v.to_le_bytes();
        self.code[offset] = bytes[0];
        self.code[offset + 1] = bytes[1];
    }

    pub fn pos(&self) -> usize {
        self.code.len()
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Code
        buf.extend(&(self.code.len() as u32).to_le_bytes());
        buf.extend(&self.code);
        // String pool
        buf.extend(&(self.string_pool.len() as u32).to_le_bytes());
        for s in &self.string_pool {
            let bytes = s.as_bytes();
            buf.extend(&(bytes.len() as u32).to_le_bytes());
            buf.extend(bytes);
        }
        // Functions
        buf.extend(&(self.functions.len() as u32).to_le_bytes());
        for f in &self.functions {
            buf.extend(&(f.code_offset as u32).to_le_bytes());
            buf.push(f.n_params);
            buf.extend(&f.n_locals.to_le_bytes());
        }
        // Entry point
        match self.entry_point {
            Some(ep) => {
                buf.push(1);
                buf.extend(&(ep as u32).to_le_bytes());
            }
            None => buf.push(0),
        }
        buf
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, String> {
        let mut pos = 0;
        let read_u32 = |pos: &mut usize| -> Result<u32, String> {
            if *pos + 4 > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let v = u32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap());
            *pos += 4;
            Ok(v)
        };
        // Code
        let code_len = read_u32(&mut pos)? as usize;
        if pos + code_len > data.len() {
            return Err("unexpected end of data".to_string());
        }
        let code = data[pos..pos + code_len].to_vec();
        pos += code_len;
        // String pool
        let str_count = read_u32(&mut pos)? as usize;
        let mut string_pool = Vec::with_capacity(str_count);
        for _ in 0..str_count {
            let slen = read_u32(&mut pos)? as usize;
            if pos + slen > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let s = std::str::from_utf8(&data[pos..pos + slen])
                .map_err(|e| format!("invalid utf-8: {e}"))?
                .to_string();
            pos += slen;
            string_pool.push(s);
        }
        // Functions
        let func_count = read_u32(&mut pos)? as usize;
        let mut functions = Vec::with_capacity(func_count);
        for _ in 0..func_count {
            let code_offset = read_u32(&mut pos)? as usize;
            if pos >= data.len() {
                return Err("unexpected end of data".to_string());
            }
            let n_params = data[pos];
            pos += 1;
            if pos + 2 > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let n_locals = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap());
            pos += 2;
            functions.push(FuncInfo {
                code_offset,
                n_params,
                n_locals,
            });
        }
        // Entry point
        if pos >= data.len() {
            return Err("unexpected end of data".to_string());
        }
        let has_entry = data[pos];
        pos += 1;
        let entry_point = if has_entry == 1 {
            Some(read_u32(&mut pos)? as usize)
        } else {
            None
        };
        Ok(Bytecode {
            code,
            string_pool,
            functions,
            entry_point,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_roundtrip() {
        for &opcode in &ALL_OPCODES {
            assert!(
                op_name(opcode).is_some(),
                "opcode 0x{opcode:02X} has no name"
            );
        }
    }

    #[test]
    fn serialize_roundtrip_empty() {
        let bc = Bytecode::new();
        let data = bc.serialize();
        let bc2 = Bytecode::deserialize(&data).unwrap();
        assert!(bc2.code.is_empty());
        assert!(bc2.string_pool.is_empty());
        assert!(bc2.functions.is_empty());
        assert_eq!(bc2.entry_point, None);
    }

    #[test]
    fn serialize_roundtrip_full() {
        let bc = Bytecode {
            code: vec![PUSH_I8, 42, HALT],
            string_pool: vec!["hello".to_string(), "world".to_string()],
            functions: vec![
                FuncInfo {
                    code_offset: 0,
                    n_params: 2,
                    n_locals: 5,
                },
                FuncInfo {
                    code_offset: 10,
                    n_params: 0,
                    n_locals: 300,
                },
            ],
            entry_point: Some(0),
        };
        let data = bc.serialize();
        let bc2 = Bytecode::deserialize(&data).unwrap();
        assert_eq!(bc2.code, bc.code);
        assert_eq!(bc2.string_pool, bc.string_pool);
        assert_eq!(bc2.functions.len(), 2);
        assert_eq!(bc2.functions[0].code_offset, 0);
        assert_eq!(bc2.functions[0].n_params, 2);
        assert_eq!(bc2.functions[0].n_locals, 5);
        assert_eq!(bc2.functions[1].code_offset, 10);
        assert_eq!(bc2.functions[1].n_locals, 300);
        assert_eq!(bc2.entry_point, Some(0));
    }

    #[test]
    fn deserialize_truncated() {
        assert!(Bytecode::deserialize(&[]).is_err());
        assert!(Bytecode::deserialize(&[0, 0]).is_err());
    }

    #[test]
    fn no_duplicate_opcodes() {
        let mut seen = std::collections::HashSet::new();
        for &opcode in &ALL_OPCODES {
            assert!(seen.insert(opcode), "duplicate opcode 0x{opcode:02X}");
        }
    }
}
