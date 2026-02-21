use std::collections::HashMap;

use crate::compiler::lir::LirFunc;

use super::{has_side_effects, inst_operands, terminator_operands};

pub fn eliminate(func: &mut LirFunc) {
    loop {
        let mut uses: HashMap<u32, usize> = HashMap::new();

        // Count uses across all instructions and terminators
        for block in &func.blocks {
            for (_, inst) in &block.insts {
                for op in inst_operands(inst) {
                    *uses.entry(op.0).or_default() += 1;
                }
            }
            for op in terminator_operands(&block.terminator) {
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
    use crate::compiler::opt::optimize;
    use crate::compiler::{lower, parse};

    fn optimized_lir(src: &str) -> LirModule {
        let ast = parse(src).unwrap();
        let hir = lower(&ast).unwrap();
        let mut module = crate::compiler::hir_to_lir::lower_to_lir(&hir);
        optimize(&mut module);
        module
    }

    fn find_func<'a>(m: &'a LirModule, name: &str) -> &'a LirFunc {
        m.functions.iter().find(|f| f.name == name).unwrap()
    }

    fn has_inst(f: &LirFunc, pred: impl Fn(&LirInst) -> bool) -> bool {
        f.blocks
            .iter()
            .any(|b| b.insts.iter().any(|(_, inst)| pred(inst)))
    }

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
