pub mod cse;
pub mod dce;
pub mod simplify;

use std::collections::HashMap;

use crate::compiler::lir::{LirFunc, LirInst, LirModule, Terminator, Value};

pub fn optimize(module: &mut LirModule) {
    for func in &mut module.functions {
        simplify::simplify(func);
        cse::eliminate(func);
        dce::eliminate(func);
    }
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

fn has_side_effects(inst: &LirInst) -> bool {
    matches!(
        inst,
        LirInst::Store { .. } | LirInst::Call { .. } | LirInst::Intrinsic { .. }
    )
}

fn resolve(map: &HashMap<Value, Value>, mut v: Value) -> Value {
    while let Some(&next) = map.get(&v) {
        v = next;
    }
    v
}

fn apply_replacements(func: &mut LirFunc, map: &HashMap<Value, Value>) {
    if map.is_empty() {
        return;
    }
    for block in &mut func.blocks {
        for (_, inst) in &mut block.insts {
            inst.replace_values(|v| resolve(map, v));
        }
        block.terminator.replace_values(|v| resolve(map, v));
    }
}
