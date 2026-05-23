pub mod cse;
pub mod dce;
pub mod simplify;

use std::collections::HashMap;

use crate::compiler::lir::{LirFunc, LirInst, LirModule, Value};

pub fn optimize(module: &mut LirModule) {
    for func in &mut module.functions {
        simplify::simplify(func);
        cse::eliminate(func);
        dce::eliminate(func);
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
