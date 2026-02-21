use std::collections::HashMap;

use crate::compiler::ast::{BinOp, UnaryOp};
use crate::compiler::bytecode::*;
use crate::compiler::error::CompileError;
use crate::compiler::lir::*;

pub fn generate(module: &LirModule) -> Result<Bytecode, CompileError> {
    let mut bc = Bytecode::new();
    bc.string_pool = module.string_pool.clone();

    let func_indices: HashMap<&str, u16> = module
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.as_str(), i as u16))
        .collect();

    for func in &module.functions {
        let code_offset = bc.pos();
        let n_params = func.params.len() as u8;
        let n_locals = count_locals(func);

        emit_function(&mut bc, func, &func_indices)?;

        bc.functions.push(FuncInfo {
            name: func.name.clone(),
            code_offset,
            n_params,
            n_locals,
        });

        if func.name == "main" {
            bc.entry_point = Some(code_offset);
        }
    }

    if bc.code.len() > 32768 {
        return Err(CompileError::new("bytecode exceeds 32KB limit"));
    }

    Ok(bc)
}

fn count_locals(func: &LirFunc) -> u8 {
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
        (max_val + 1).min(255) as u8
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

fn emit_const(bc: &mut Bytecode, n: i16) {
    match n {
        0 => bc.emit_op(PUSH0),
        1 => bc.emit_op(PUSH1),
        -128..=127 => {
            bc.emit_op(PUSH_I8);
            bc.emit_i8(n as i8);
        }
        _ => {
            bc.emit_op(PUSH_I16);
            bc.emit_i16(n);
        }
    }
}

fn binop_opcode(op: BinOp) -> u8 {
    match op {
        BinOp::Add => ADD,
        BinOp::Sub => SUB,
        BinOp::Mul => MUL,
        BinOp::Div => DIV,
        BinOp::Mod => MOD,
        BinOp::Eq => EQ,
        BinOp::Neq => NEQ,
        BinOp::Lt => LT,
        BinOp::Gt => GT,
        BinOp::Leq => LEQ,
        BinOp::Geq => GEQ,
        BinOp::And => AND,
        BinOp::Or => OR,
    }
}

fn unaryop_opcode(op: UnaryOp) -> u8 {
    match op {
        UnaryOp::Neg => NEG,
        UnaryOp::Not => NOT,
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
            emit_instruction(bc, *val, inst, &uses, func_indices)?;
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
) -> Result<(), CompileError> {
    let used = uses.get(&val).copied().unwrap_or(0) > 0;

    match inst {
        LirInst::Const(n) => {
            emit_const(bc, *n);
            if used {
                bc.emit_op(STORE_LOCAL);
                bc.emit_u8(val.0 as u8);
            } else {
                bc.emit_op(POP);
            }
        }
        LirInst::ConstBool(b) => {
            if *b {
                bc.emit_op(PUSH1);
            } else {
                bc.emit_op(PUSH0);
            }
            if used {
                bc.emit_op(STORE_LOCAL);
                bc.emit_u8(val.0 as u8);
            } else {
                bc.emit_op(POP);
            }
        }
        LirInst::GlobalAddr(addr) => {
            emit_const(bc, *addr as i16);
            if used {
                bc.emit_op(STORE_LOCAL);
                bc.emit_u8(val.0 as u8);
            } else {
                bc.emit_op(POP);
            }
        }
        LirInst::BinOp { op, lhs, rhs } => {
            bc.emit_op(LOAD_LOCAL);
            bc.emit_u8(lhs.0 as u8);
            bc.emit_op(LOAD_LOCAL);
            bc.emit_u8(rhs.0 as u8);
            bc.emit_op(binop_opcode(*op));
            if used {
                bc.emit_op(STORE_LOCAL);
                bc.emit_u8(val.0 as u8);
            } else {
                bc.emit_op(POP);
            }
        }
        LirInst::UnaryOp { op, val: operand } => {
            bc.emit_op(LOAD_LOCAL);
            bc.emit_u8(operand.0 as u8);
            bc.emit_op(unaryop_opcode(*op));
            if used {
                bc.emit_op(STORE_LOCAL);
                bc.emit_u8(val.0 as u8);
            } else {
                bc.emit_op(POP);
            }
        }
        LirInst::Call { name, args } => {
            for arg in args {
                bc.emit_op(LOAD_LOCAL);
                bc.emit_u8(arg.0 as u8);
            }
            let func_idx = func_indices
                .get(name.as_str())
                .ok_or_else(|| CompileError::new(format!("undefined function: {name}")))?;
            bc.emit_op(CALL);
            bc.emit_u16(*func_idx);
            bc.emit_u8(args.len() as u8);
            if used {
                bc.emit_op(STORE_LOCAL);
                bc.emit_u8(val.0 as u8);
            } else {
                bc.emit_op(POP);
            }
        }
        LirInst::Intrinsic { op, args } => {
            for arg in args {
                bc.emit_op(LOAD_LOCAL);
                bc.emit_u8(arg.0 as u8);
            }
            bc.emit_op(intrinsic_opcode(*op));
            if intrinsic_has_return_value(*op) {
                if used {
                    bc.emit_op(STORE_LOCAL);
                    bc.emit_u8(val.0 as u8);
                } else {
                    bc.emit_op(POP);
                }
            }
        }
        LirInst::Load { addr, size } => {
            bc.emit_op(LOAD_LOCAL);
            bc.emit_u8(addr.0 as u8);
            match size {
                1 => bc.emit_op(LOAD1),
                _ => bc.emit_op(LOAD2),
            }
            if used {
                bc.emit_op(STORE_LOCAL);
                bc.emit_u8(val.0 as u8);
            } else {
                bc.emit_op(POP);
            }
        }
        LirInst::Store {
            addr,
            val: store_val,
            size,
        } => {
            bc.emit_op(LOAD_LOCAL);
            bc.emit_u8(addr.0 as u8);
            bc.emit_op(LOAD_LOCAL);
            bc.emit_u8(store_val.0 as u8);
            match size {
                1 => bc.emit_op(STORE1),
                _ => bc.emit_op(STORE2),
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
                    bc.emit_op(LOAD_LOCAL);
                    bc.emit_u8(source_val.0 as u8);
                    bc.emit_op(STORE_LOCAL);
                    bc.emit_u8(phi_val.0 as u8);
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
            bc.emit_op(JUMP);
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
            bc.emit_op(LOAD_LOCAL);
            bc.emit_u8(cond.0 as u8);
            bc.emit_op(JUMP_IF_FALSE);
            let else_patch_offset = bc.pos();
            bc.emit_i16(0); // placeholder for else copies

            // Then path: phi copies + jump to then_block
            emit_phi_copies(bc, current_block, *then_block, func);
            bc.emit_op(JUMP);
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
            bc.emit_op(JUMP);
            let else_jump_patch = bc.pos();
            bc.emit_i16(0);
            patches.push(JumpPatch {
                patch_offset: else_jump_patch,
                target: *else_block,
            });
        }
        Terminator::Return(None) => {
            bc.emit_op(RET);
        }
        Terminator::Return(Some(val)) => {
            bc.emit_op(LOAD_LOCAL);
            bc.emit_u8(val.0 as u8);
            bc.emit_op(RET);
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
        assert_eq!(bc.functions[0].name, "f");
        assert!(has_opcode(&bc.code, RET));
    }

    #[test]
    fn constant_return() {
        let bc = compile("fn f(): int\n  return 42\nend");
        assert!(has_opcode(&bc.code, PUSH_I8));
        assert!(has_opcode(&bc.code, RET));
    }

    #[test]
    fn arithmetic_folded() {
        let bc = compile("fn f(): int\n  return 3 + 5\nend");
        assert!(has_opcode(&bc.code, PUSH_I8));
        assert!(has_opcode(&bc.code, RET));
    }

    #[test]
    fn intrinsic_cls() {
        let bc = compile("fn f()\n  cls(0)\nend");
        assert!(has_opcode(&bc.code, PUSH0));
        assert!(has_opcode(&bc.code, OP_CLS));
    }

    #[test]
    fn function_call() {
        let bc = compile(
            "fn add(a: int, b: int): int\n  return a + b\nend\nfn f(): int\n  return add(1, 2)\nend",
        );
        assert!(has_opcode(&bc.code, CALL));
    }

    #[test]
    fn if_branch() {
        let bc = compile("fn f(x: bool)\n  if x\n    cls(0)\n  end\nend");
        assert!(has_opcode(&bc.code, JUMP_IF_FALSE));
    }

    #[test]
    fn while_loop() {
        let bc = compile("fn f()\n  x: int = 0\n  while x < 10\n    x = x + 1\n  end\nend");
        assert!(has_opcode(&bc.code, JUMP));
    }

    #[test]
    fn global_store_load() {
        let bc = compile("g: int\nfn f()\n  g = 5\n  cls(g)\nend");
        assert!(has_opcode(&bc.code, STORE2));
        assert!(has_opcode(&bc.code, LOAD2));
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
}
