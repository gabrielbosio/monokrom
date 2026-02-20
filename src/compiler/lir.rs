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
