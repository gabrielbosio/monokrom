use std::collections::{HashMap, HashSet};

use crate::compiler::ast::{BinOp, UnaryOp};
use crate::compiler::bytecode::*;
use crate::compiler::error::CompileError;
use crate::compiler::lir::*;

const BYTECODE_LIMIT: usize = 32 * 1024;

pub fn generate(module: &LirModule) -> Result<Bytecode, CompileError> {
    let mut bc = Bytecode::new();
    bc.string_pool = module.string_pool.clone();

    let func_indices: HashMap<&str, u16> = module
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.as_str(), i as u16))
        .collect();

    let void_funcs: HashSet<&str> = module
        .functions
        .iter()
        .filter(|f| {
            f.blocks
                .iter()
                .flat_map(|b| match &b.terminator {
                    Terminator::Return(v) => Some(v),
                    _ => None,
                })
                .all(|v| v.is_none())
        })
        .map(|f| f.name.as_str())
        .collect();

    for func in &module.functions {
        let code_offset = bc.pos();
        let n_params = func.params.len() as u8;
        let n_locals = count_locals(func);

        emit_function(&mut bc, func, &func_indices, &void_funcs)?;

        bc.functions.push(FuncInfo {
            code_offset,
            n_params,
            n_locals,
        });

        if func.name == "main" {
            bc.entry_point = Some(code_offset);
        }
    }

    if bc.code.len() > BYTECODE_LIMIT {
        return Err(CompileError::new("bytecode exceeds 32KB limit"));
    }

    Ok(bc)
}

fn count_locals(func: &LirFunc) -> u16 {
    let mut max_val: u32 = 0;
    let mut found = false;
    for block in &func.blocks {
        for (val, _) in &block.insts {
            if val.0 >= max_val {
                max_val = val.0;
                found = true;
            }
        }
    }
    for p in &func.params {
        if p.0 >= max_val {
            max_val = p.0;
            found = true;
        }
    }
    if found {
        (max_val + 1) as u16
    } else {
        0
    }
}

fn count_uses(func: &LirFunc) -> HashMap<Value, u32> {
    let mut uses: HashMap<Value, u32> = HashMap::new();
    for block in &func.blocks {
        for (_, inst) in &block.insts {
            for v in inst_operands(inst) {
                *uses.entry(v).or_default() += 1;
            }
        }
        for v in terminator_operands(&block.terminator) {
            *uses.entry(v).or_default() += 1;
        }
    }
    uses
}

fn inst_operands(inst: &LirInst) -> Vec<Value> {
    match inst {
        LirInst::Const(_) | LirInst::ConstBool(_) | LirInst::GlobalAddr(_) => vec![],
        LirInst::BinOp { lhs, rhs, .. } => vec![*lhs, *rhs],
        LirInst::UnaryOp { val, .. } => vec![*val],
        LirInst::Call { args, .. } | LirInst::Intrinsic { args, .. } => args.clone(),
        LirInst::Load { addr, .. } => vec![*addr],
        LirInst::Store { addr, val, .. } => vec![*addr, *val],
        LirInst::Phi(entries) => entries.iter().map(|(_, v)| *v).collect(),
    }
}

fn terminator_operands(term: &Terminator) -> Vec<Value> {
    match term {
        Terminator::Jump(_) => vec![],
        Terminator::Branch { cond, .. } => vec![*cond],
        Terminator::Return(Some(v)) => vec![*v],
        Terminator::Return(None) => vec![],
    }
}

fn emit_store_or_pop(bc: &mut Bytecode, val: Value, used: bool) {
    if used {
        bc.emit_op(OP_STORE_LOCAL);
        bc.emit_u16(val.0 as u16);
    } else {
        bc.emit_op(OP_POP);
    }
}

fn emit_const(bc: &mut Bytecode, n: i16) {
    match n {
        0 => bc.emit_op(OP_PUSH0),
        1 => bc.emit_op(OP_PUSH1),
        -128..=127 => {
            bc.emit_op(OP_PUSH_I8);
            bc.emit_i8(n as i8);
        }
        _ => {
            bc.emit_op(OP_PUSH_I16);
            bc.emit_i16(n);
        }
    }
}

fn binop_opcode(op: BinOp) -> u8 {
    match op {
        BinOp::Add => OP_ADD,
        BinOp::Sub => OP_SUB,
        BinOp::Mul => OP_MUL,
        BinOp::Div => OP_DIV,
        BinOp::Mod => OP_MOD,
        BinOp::Eq => OP_EQ,
        BinOp::Neq => OP_NEQ,
        BinOp::Lt => OP_LT,
        BinOp::Gt => OP_GT,
        BinOp::Leq => OP_LEQ,
        BinOp::Geq => OP_GEQ,
        BinOp::And => OP_AND,
        BinOp::Or => OP_OR,
        BinOp::BitAnd => OP_BAND,
        BinOp::BitOr => OP_BOR,
        BinOp::Shl => OP_SHL,
        BinOp::Shr => OP_SHR,
    }
}

fn unaryop_opcode(op: UnaryOp) -> u8 {
    match op {
        UnaryOp::Neg => OP_NEG,
        UnaryOp::Not => OP_NOT,
        UnaryOp::BitNot => OP_BNOT,
    }
}

struct JumpPatch {
    patch_offset: usize,
    target: BlockId,
}

fn emit_function(
    bc: &mut Bytecode,
    func: &LirFunc,
    func_indices: &HashMap<&str, u16>,
    void_funcs: &HashSet<&str>,
) -> Result<(), CompileError> {
    let uses = count_uses(func);

    // Pass 1: emit with placeholder jumps
    let mut block_offsets: HashMap<BlockId, usize> = HashMap::new();
    let mut patches: Vec<JumpPatch> = Vec::new();

    for block in &func.blocks {
        block_offsets.insert(block.id, bc.pos());

        for (val, inst) in &block.insts {
            if matches!(inst, LirInst::Phi(_)) {
                continue;
            }
            emit_instruction(bc, *val, inst, &uses, func_indices, void_funcs)?;
        }

        emit_terminator(bc, block.id, &block.terminator, func, &mut patches);
    }

    // Pass 2: patch jumps
    for patch in &patches {
        let target_offset = block_offsets[&patch.target];
        let rel = target_offset as isize - (patch.patch_offset as isize + 2);
        bc.patch_i16(patch.patch_offset, rel as i16);
    }

    Ok(())
}

fn emit_instruction(
    bc: &mut Bytecode,
    val: Value,
    inst: &LirInst,
    uses: &HashMap<Value, u32>,
    func_indices: &HashMap<&str, u16>,
    void_funcs: &HashSet<&str>,
) -> Result<(), CompileError> {
    let used = uses.get(&val).copied().unwrap_or(0) > 0;

    match inst {
        LirInst::Const(n) => {
            emit_const(bc, *n);
            emit_store_or_pop(bc, val, used);
        }
        LirInst::ConstBool(b) => {
            if *b {
                bc.emit_op(OP_PUSH1);
            } else {
                bc.emit_op(OP_PUSH0);
            }
            emit_store_or_pop(bc, val, used);
        }
        LirInst::GlobalAddr(addr) => {
            emit_const(bc, *addr as i16);
            emit_store_or_pop(bc, val, used);
        }
        LirInst::BinOp {
            op,
            lhs,
            rhs,
            is_fixed,
        } => {
            bc.emit_op(OP_LOAD_LOCAL);
            bc.emit_u16(lhs.0 as u16);
            bc.emit_op(OP_LOAD_LOCAL);
            bc.emit_u16(rhs.0 as u16);
            if *is_fixed {
                match op {
                    BinOp::Mul => bc.emit_op(OP_FMUL),
                    BinOp::Div => bc.emit_op(OP_FDIV),
                    _ => bc.emit_op(binop_opcode(*op)),
                }
            } else {
                bc.emit_op(binop_opcode(*op));
            }
            emit_store_or_pop(bc, val, used);
        }
        LirInst::UnaryOp { op, val: operand } => {
            bc.emit_op(OP_LOAD_LOCAL);
            bc.emit_u16(operand.0 as u16);
            bc.emit_op(unaryop_opcode(*op));
            emit_store_or_pop(bc, val, used);
        }
        LirInst::Call { name, args } => {
            for arg in args {
                bc.emit_op(OP_LOAD_LOCAL);
                bc.emit_u16(arg.0 as u16);
            }
            let func_idx = func_indices
                .get(name.as_str())
                .ok_or_else(|| CompileError::new(format!("undefined function: {name}")))?;
            bc.emit_op(OP_CALL);
            bc.emit_u16(*func_idx);
            bc.emit_u8(args.len() as u8);
            if used {
                bc.emit_op(OP_STORE_LOCAL);
                bc.emit_u16(val.0 as u16);
            } else if !void_funcs.contains(name.as_str()) {
                bc.emit_op(OP_POP);
            }
        }
        LirInst::Intrinsic { op, args } => {
            for arg in args {
                bc.emit_op(OP_LOAD_LOCAL);
                bc.emit_u16(arg.0 as u16);
            }
            bc.emit_op(intrinsic_opcode(*op));
            if intrinsic_has_return_value(*op) {
                emit_store_or_pop(bc, val, used);
            }
        }
        LirInst::Load { addr, size } => {
            bc.emit_op(OP_LOAD_LOCAL);
            bc.emit_u16(addr.0 as u16);
            match size {
                1 => bc.emit_op(OP_LOAD1),
                _ => bc.emit_op(OP_LOAD2),
            }
            emit_store_or_pop(bc, val, used);
        }
        LirInst::Store {
            addr,
            val: store_val,
            size,
        } => {
            bc.emit_op(OP_LOAD_LOCAL);
            bc.emit_u16(addr.0 as u16);
            bc.emit_op(OP_LOAD_LOCAL);
            bc.emit_u16(store_val.0 as u16);
            match size {
                1 => bc.emit_op(OP_STORE1),
                _ => bc.emit_op(OP_STORE2),
            }
        }
        LirInst::Phi(_) => {}
    }

    Ok(())
}

fn emit_phi_copies(bc: &mut Bytecode, from_block: BlockId, to_block: BlockId, func: &LirFunc) {
    let Some(target) = func.blocks.iter().find(|b| b.id == to_block) else {
        return;
    };
    for (phi_val, inst) in &target.insts {
        if let LirInst::Phi(entries) = inst {
            for (block_id, source_val) in entries {
                if *block_id == from_block {
                    bc.emit_op(OP_LOAD_LOCAL);
                    bc.emit_u16(source_val.0 as u16);
                    bc.emit_op(OP_STORE_LOCAL);
                    bc.emit_u16(phi_val.0 as u16);
                    break;
                }
            }
        }
    }
}

fn emit_terminator(
    bc: &mut Bytecode,
    current_block: BlockId,
    term: &Terminator,
    func: &LirFunc,
    patches: &mut Vec<JumpPatch>,
) {
    match term {
        Terminator::Jump(target) => {
            emit_phi_copies(bc, current_block, *target, func);
            bc.emit_op(OP_JUMP);
            let patch_offset = bc.pos();
            bc.emit_i16(0);
            patches.push(JumpPatch {
                patch_offset,
                target: *target,
            });
        }
        Terminator::Branch {
            cond,
            then_block,
            else_block,
        } => {
            bc.emit_op(OP_LOAD_LOCAL);
            bc.emit_u16(cond.0 as u16);
            bc.emit_op(OP_JUMP_IF_FALSE);
            let else_patch_offset = bc.pos();
            bc.emit_i16(0); // placeholder for else copies

            // Then path: phi copies + jump to then_block
            emit_phi_copies(bc, current_block, *then_block, func);
            bc.emit_op(OP_JUMP);
            let then_patch_offset = bc.pos();
            bc.emit_i16(0);
            patches.push(JumpPatch {
                patch_offset: then_patch_offset,
                target: *then_block,
            });

            // Else path: patch JumpIfFalse to land here
            let else_copies_pos = bc.pos();
            let rel = else_copies_pos as isize - (else_patch_offset as isize + 2);
            bc.patch_i16(else_patch_offset, rel as i16);

            emit_phi_copies(bc, current_block, *else_block, func);
            bc.emit_op(OP_JUMP);
            let else_jump_patch = bc.pos();
            bc.emit_i16(0);
            patches.push(JumpPatch {
                patch_offset: else_jump_patch,
                target: *else_block,
            });
        }
        Terminator::Return(None) => {
            bc.emit_op(OP_RET);
        }
        Terminator::Return(Some(val)) => {
            bc.emit_op(OP_LOAD_LOCAL);
            bc.emit_u16(val.0 as u16);
            bc.emit_op(OP_RET);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::opt::optimize;
    use crate::compiler::{lower, lower_lir, parse};

    fn compile(src: &str) -> Bytecode {
        let ast = parse(src).unwrap();
        let hir = lower(&ast).unwrap();
        let mut lir = lower_lir(&hir);
        optimize(&mut lir);
        generate(&lir).unwrap()
    }

    fn has_opcode(code: &[u8], op: u8) -> bool {
        code.contains(&op)
    }

    #[test]
    fn empty_function() {
        let bc = compile("fn f()\nend");
        assert!(!bc.functions.is_empty());
        assert!(has_opcode(&bc.code, OP_RET));
    }

    #[test]
    fn constant_return() {
        let bc = compile("fn f(): int\n  return 42\nend");
        assert!(has_opcode(&bc.code, OP_PUSH_I8));
        assert!(has_opcode(&bc.code, OP_RET));
    }

    #[test]
    fn arithmetic_folded() {
        let bc = compile("fn f(): int\n  return 3 + 5\nend");
        assert!(has_opcode(&bc.code, OP_PUSH_I8));
        assert!(has_opcode(&bc.code, OP_RET));
    }

    #[test]
    fn intrinsic_cls() {
        let bc = compile("fn f()\n  cls(0)\nend");
        assert!(has_opcode(&bc.code, OP_PUSH0));
        assert!(has_opcode(&bc.code, OP_CLS));
    }

    #[test]
    fn function_call() {
        let bc = compile(
            "fn add(a: int, b: int): int\n  return a + b\nend\nfn f(): int\n  return add(1, 2)\nend",
        );
        assert!(has_opcode(&bc.code, OP_CALL));
    }

    #[test]
    fn if_branch() {
        let bc = compile("fn f(x: bool)\n  if x\n    cls(0)\n  end\nend");
        assert!(has_opcode(&bc.code, OP_JUMP_IF_FALSE));
    }

    #[test]
    fn while_loop() {
        let bc = compile("fn f()\n  x: int = 0\n  while x < 10\n    x = x + 1\n  end\nend");
        assert!(has_opcode(&bc.code, OP_JUMP));
    }

    #[test]
    fn global_store_load() {
        let bc = compile("g: int\nfn f()\n  g = 5\n  cls(g)\nend");
        assert!(has_opcode(&bc.code, OP_STORE2));
        assert!(has_opcode(&bc.code, OP_LOAD2));
    }

    #[test]
    fn entry_point_main() {
        let bc = compile("fn main()\nend");
        assert!(bc.entry_point.is_some());
    }

    #[test]
    fn entry_point_no_main() {
        let bc = compile("fn f()\nend");
        assert!(bc.entry_point.is_none());
    }

    #[test]
    fn void_call_no_pop() {
        let bc = compile(
            "buf: array[128] of int\nfn mutate(idx: int)\n  buf[idx] = 0\nend\nfn main()\n  mutate(1)\nend",
        );
        // Find the OP_CALL in main's code and verify no OP_POP follows it
        let main_info = bc.functions.last().unwrap();
        let code = &bc.code[main_info.code_offset..];
        let call_pos = code.windows(1).position(|w| w[0] == OP_CALL).unwrap();
        // OP_CALL is followed by u16 func_idx + u8 n_args = 3 bytes
        let after_call = call_pos + 1 + 3;
        assert_ne!(code[after_call], OP_POP, "void call should not emit OP_POP");
    }
}
