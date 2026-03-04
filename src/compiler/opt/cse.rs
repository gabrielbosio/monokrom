use std::collections::HashMap;

use crate::compiler::ast::{BinOp, UnaryOp};
use crate::compiler::lir::{LirFunc, LirInst, Value};

use super::apply_replacements;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CseKey {
    Const(i16),
    ConstBool(bool),
    BinOp {
        op: BinOp,
        lhs: Value,
        rhs: Value,
        is_fixed: bool,
    },
    UnaryOp {
        op: UnaryOp,
        val: Value,
    },
    GlobalAddr(u16),
    Load {
        addr: Value,
        size: u8,
    },
}

pub fn eliminate(func: &mut LirFunc) {
    let mut replace: HashMap<Value, Value> = HashMap::new();

    for block in &func.blocks {
        let mut seen: HashMap<CseKey, Value> = HashMap::new();

        for (val, inst) in &block.insts {
            // Skip side-effectful and phi
            if super::has_side_effects(inst) || matches!(inst, LirInst::Phi(_)) {
                // Stores invalidate load entries
                if matches!(inst, LirInst::Store { .. }) {
                    seen.retain(|k, _| !matches!(k, CseKey::Load { .. }));
                }
                continue;
            }

            let key = match inst {
                LirInst::Const(n) => CseKey::Const(*n),
                LirInst::ConstBool(b) => CseKey::ConstBool(*b),
                LirInst::BinOp {
                    op,
                    lhs,
                    rhs,
                    is_fixed,
                } => CseKey::BinOp {
                    op: *op,
                    lhs: *lhs,
                    rhs: *rhs,
                    is_fixed: *is_fixed,
                },
                LirInst::UnaryOp { op, val } => CseKey::UnaryOp { op: *op, val: *val },
                LirInst::GlobalAddr(addr) => CseKey::GlobalAddr(*addr),
                LirInst::Load { addr, size } => CseKey::Load {
                    addr: *addr,
                    size: *size,
                },
                _ => continue,
            };

            if let Some(&existing) = seen.get(&key) {
                replace.insert(*val, existing);
            } else {
                seen.insert(key, *val);
            }
        }
    }

    apply_replacements(func, &replace);
}

#[cfg(test)]
mod tests {
    use crate::compiler::lir::*;
    use crate::compiler::test_helpers::*;

    #[test]
    fn eliminate_duplicate() {
        let m = optimized_lir(
            "fn f(a: int, b: int): int\n  x = a + b\n  y = a + b\n  return x + y\nend",
        );
        let f = find_func(&m, "f");
        let add_count: usize = f
            .blocks
            .iter()
            .map(|b| {
                b.insts
                    .iter()
                    .filter(|(_, inst)| {
                        matches!(
                            inst,
                            LirInst::BinOp {
                                op: crate::compiler::ast::BinOp::Add,
                                ..
                            }
                        )
                    })
                    .count()
            })
            .sum();
        // Should have 2 adds: one for a+b (deduplicated), one for x+y
        assert_eq!(add_count, 2);
    }
}
