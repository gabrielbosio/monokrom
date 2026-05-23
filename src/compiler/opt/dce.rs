use std::collections::HashMap;

use crate::compiler::lir::LirFunc;

use super::has_side_effects;

pub fn eliminate(func: &mut LirFunc) {
    loop {
        let mut uses: HashMap<u32, usize> = HashMap::new();

        // Count uses across all instructions and terminators
        for block in &func.blocks {
            for (_, inst) in &block.insts {
                for op in inst.operands() {
                    *uses.entry(op.0).or_default() += 1;
                }
            }
            for op in block.terminator.operands() {
                *uses.entry(op.0).or_default() += 1;
            }
        }

        let mut changed = false;
        for block in &mut func.blocks {
            block.insts.retain(|(val, inst)| {
                let used = uses.get(&val.0).copied().unwrap_or(0) > 0;
                if !used && !has_side_effects(inst) {
                    changed = true;
                    return false;
                }
                true
            });
        }

        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::lir::*;
    use crate::compiler::test_helpers::*;

    #[test]
    fn remove_unused() {
        let m = optimized_lir("fn f()\n  x = 1 + 2\nend");
        let f = find_func(&m, "f");
        assert!(!has_inst(f, |i| matches!(i, LirInst::Const(_))));
        assert!(!has_inst(f, |i| matches!(i, LirInst::BinOp { .. })));
    }

    #[test]
    fn keep_side_effects() {
        let m = optimized_lir("fn f()\n  cls(0)\nend");
        let f = find_func(&m, "f");
        assert!(has_inst(f, |i| matches!(i, LirInst::Intrinsic { .. })));
    }

    #[test]
    fn keep_used() {
        let m = optimized_lir("fn f(): int\n  return 1 + 2\nend");
        let f = find_func(&m, "f");
        // After constant folding, 1+2 becomes Const(3), which is used by return
        assert!(has_inst(f, |i| matches!(i, LirInst::Const(3))));
    }
}
