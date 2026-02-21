use crate::compiler::ast::{BinOp, UnaryOp};
use crate::compiler::hir::Intrinsic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Value(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub enum LirInst {
    Const(i16),
    ConstBool(bool),
    BinOp { op: BinOp, lhs: Value, rhs: Value },
    UnaryOp { op: UnaryOp, val: Value },
    Call { name: String, args: Vec<Value> },
    Intrinsic { op: Intrinsic, args: Vec<Value> },
    GlobalAddr(u16),
    Load { addr: Value, size: u8 },
    Store { addr: Value, val: Value, size: u8 },
    Phi(Vec<(BlockId, Value)>),
}

impl LirInst {
    pub fn replace_values(&mut self, mut f: impl FnMut(Value) -> Value) {
        match self {
            LirInst::Const(_) | LirInst::ConstBool(_) | LirInst::GlobalAddr(_) => {}
            LirInst::BinOp { lhs, rhs, .. } => {
                *lhs = f(*lhs);
                *rhs = f(*rhs);
            }
            LirInst::UnaryOp { val, .. } => *val = f(*val),
            LirInst::Call { args, .. } | LirInst::Intrinsic { args, .. } => {
                for a in args {
                    *a = f(*a);
                }
            }
            LirInst::Load { addr, .. } => *addr = f(*addr),
            LirInst::Store { addr, val, .. } => {
                *addr = f(*addr);
                *val = f(*val);
            }
            LirInst::Phi(operands) => {
                for (_, v) in operands {
                    *v = f(*v);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        cond: Value,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return(Option<Value>),
}

impl Terminator {
    pub fn replace_values(&mut self, mut f: impl FnMut(Value) -> Value) {
        match self {
            Terminator::Jump(_) => {}
            Terminator::Branch { cond, .. } => *cond = f(*cond),
            Terminator::Return(Some(v)) => *v = f(*v),
            Terminator::Return(None) => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub id: BlockId,
    pub insts: Vec<(Value, LirInst)>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LirFunc {
    pub name: String,
    pub params: Vec<Value>,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Debug, PartialEq)]
pub struct LirModule {
    pub functions: Vec<LirFunc>,
    pub string_pool: Vec<String>,
    pub globals_size: u16,
}
