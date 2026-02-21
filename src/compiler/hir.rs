use crate::compiler::ast::{BinOp, UnaryOp};

#[derive(Debug, Clone, PartialEq)]
pub enum HirType {
    Int,
    Fixed,
    Bool,
    Str,
    Void,
    Array(Box<HirType>, i16),
    Struct(String),
}

impl std::fmt::Display for HirType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int => write!(f, "int"),
            Self::Fixed => write!(f, "fixed"),
            Self::Bool => write!(f, "bool"),
            Self::Str => write!(f, "str"),
            Self::Void => write!(f, "void"),
            Self::Array(elem, size) => write!(f, "array[{size}] of {elem}"),
            Self::Struct(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intrinsic {
    Pset,
    Pget,
    Cls,
    Line,
    Rect,
    Circ,
    Spr,
    Prints,
    Printn,
    Btn,
    Btnp,
    Sfx,
    Music,
    Peek,
    Poke,
    Sin,
    Cos,
    Sqrt,
    Abs,
    Min,
    Max,
    Flip,
    Tracen,
    Traces,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirExpr {
    pub kind: HirExprKind,
    pub ty: HirType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HirExprKind {
    IntLit(i16),
    FixedLit(i16),
    BoolLit(bool),
    StrLit(u16),
    Var(String),
    Global(String),
    BinOp {
        op: BinOp,
        lhs: Box<HirExpr>,
        rhs: Box<HirExpr>,
    },
    UnaryOp {
        op: UnaryOp,
        expr: Box<HirExpr>,
    },
    Call {
        name: String,
        args: Vec<HirExpr>,
    },
    Intrinsic {
        op: Intrinsic,
        args: Vec<HirExpr>,
    },
    Index {
        expr: Box<HirExpr>,
        index: Box<HirExpr>,
    },
    FieldAccess {
        expr: Box<HirExpr>,
        field: String,
        offset: u16,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum HirStmt {
    VarDecl {
        name: String,
        ty: HirType,
        value: Option<HirExpr>,
    },
    Assign {
        target: HirExpr,
        value: HirExpr,
    },
    If {
        cond: HirExpr,
        body: Vec<HirStmt>,
        else_ifs: Vec<(HirExpr, Vec<HirStmt>)>,
        else_body: Vec<HirStmt>,
    },
    While {
        cond: HirExpr,
        body: Vec<HirStmt>,
    },
    ForIn {
        index: String,
        elem: String,
        iter: HirExpr,
        body: Vec<HirStmt>,
    },
    ForRange {
        var: String,
        start: HirExpr,
        end: HirExpr,
        inclusive: bool,
        body: Vec<HirStmt>,
    },
    Return(Option<HirExpr>),
    Expression(HirExpr),
    Break,
    Continue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirStruct {
    pub name: String,
    pub fields: Vec<HirStructField>,
    pub size: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirStructField {
    pub name: String,
    pub ty: HirType,
    pub offset: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirGlobal {
    pub name: String,
    pub ty: HirType,
    pub address: u16,
    pub init: Option<HirExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirFunc {
    pub name: String,
    pub params: Vec<HirParam>,
    pub ret_type: HirType,
    pub locals: Vec<(String, HirType)>,
    pub body: Vec<HirStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirParam {
    pub name: String,
    pub ty: HirType,
}

#[derive(Debug)]
pub struct HirModule {
    pub structs: Vec<HirStruct>,
    pub globals: Vec<HirGlobal>,
    pub functions: Vec<HirFunc>,
    pub global_init: Vec<HirStmt>,
    pub string_pool: Vec<String>,
}
