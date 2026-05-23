use std::collections::HashMap;

use crate::compiler::ast::{BinOp, UnaryOp};
use crate::compiler::lir::{LirFunc, LirInst, Value};

use super::{apply_replacements, resolve};

pub fn simplify(func: &mut LirFunc) {
    let mut consts: HashMap<Value, i16> = HashMap::new();
    let mut bools: HashMap<Value, bool> = HashMap::new();
    let mut replace: HashMap<Value, Value> = HashMap::new();

    for block in &mut func.blocks {
        for (val, inst) in &mut block.insts {
            let val = *val;
            match inst.clone() {
                LirInst::Const(n) => {
                    consts.insert(val, n);
                }
                LirInst::ConstBool(b) => {
                    bools.insert(val, b);
                }
                LirInst::BinOp {
                    op,
                    lhs,
                    rhs,
                    is_fixed,
                } => {
                    let lhs = resolve(&replace, lhs);
                    let rhs = resolve(&replace, rhs);

                    let l_const = consts.get(&lhs).copied();
                    let r_const = consts.get(&rhs).copied();
                    let l_bool = bools.get(&lhs).copied();
                    let r_bool = bools.get(&rhs).copied();

                    if let (Some(l), Some(r)) = (l_const, r_const) {
                        if let Some(result) = eval_binop(op, l, r, is_fixed) {
                            if is_comparison(op) || is_logical(op) {
                                let b = result != 0;
                                *inst = LirInst::ConstBool(b);
                                bools.insert(val, b);
                            } else {
                                *inst = LirInst::Const(result);
                                consts.insert(val, result);
                            }
                            continue;
                        }
                    }

                    if is_logical(op) {
                        if let (Some(l), Some(r)) = (l_bool, r_bool) {
                            let result = match op {
                                BinOp::And => l && r,
                                BinOp::Or => l || r,
                                _ => unreachable!(),
                            };
                            *inst = LirInst::ConstBool(result);
                            bools.insert(val, result);
                            continue;
                        }
                    }

                    if let Some(new) =
                        algebraic_simplify(op, lhs, rhs, is_fixed, &consts, &bools, inst)
                    {
                        match new {
                            Simplified::Replace(target) => {
                                replace.insert(val, target);
                                if let Some(&n) = consts.get(&target) {
                                    consts.insert(val, n);
                                }
                                if let Some(&b) = bools.get(&target) {
                                    bools.insert(val, b);
                                }
                            }
                            Simplified::Const(n) => {
                                consts.insert(val, n);
                            }
                            Simplified::ConstBool(b) => {
                                bools.insert(val, b);
                            }
                        }
                    }
                }
                LirInst::UnaryOp { op, val: operand } => {
                    let operand = resolve(&replace, operand);
                    match op {
                        UnaryOp::Neg => {
                            if let Some(&n) = consts.get(&operand) {
                                *inst = LirInst::Const(n.wrapping_neg());
                                consts.insert(val, n.wrapping_neg());
                            }
                        }
                        UnaryOp::Not => {
                            if let Some(&b) = bools.get(&operand) {
                                *inst = LirInst::ConstBool(!b);
                                bools.insert(val, !b);
                            }
                        }
                        UnaryOp::BitNot => {
                            if let Some(&n) = consts.get(&operand) {
                                *inst = LirInst::Const(!n);
                                consts.insert(val, !n);
                            }
                        }
                    }
                }
                LirInst::Phi(entries) => {
                    let resolved: Vec<Value> = entries
                        .iter()
                        .map(|(_, v)| resolve(&replace, *v))
                        .filter(|v| *v != val)
                        .collect();

                    if !resolved.is_empty() && resolved.iter().all(|v| *v == resolved[0]) {
                        let target = resolved[0];
                        replace.insert(val, target);
                        if let Some(&n) = consts.get(&target) {
                            consts.insert(val, n);
                        }
                        if let Some(&b) = bools.get(&target) {
                            bools.insert(val, b);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    apply_replacements(func, &replace);
}

fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Leq | BinOp::Geq
    )
}

fn is_logical(op: BinOp) -> bool {
    matches!(op, BinOp::And | BinOp::Or)
}

fn eval_binop(op: BinOp, l: i16, r: i16, is_fixed: bool) -> Option<i16> {
    Some(match op {
        BinOp::Add => l.wrapping_add(r),
        BinOp::Sub => l.wrapping_sub(r),
        BinOp::Mul => {
            if is_fixed {
                ((l as i32 * r as i32) >> 7) as i16
            } else {
                l.wrapping_mul(r)
            }
        }
        BinOp::Div => {
            if r == 0 {
                return None;
            }
            if is_fixed {
                (((l as i32) << 7) / r as i32) as i16
            } else {
                l.wrapping_div(r)
            }
        }
        BinOp::Mod => {
            if r == 0 {
                return None;
            }
            l.wrapping_rem(r)
        }
        BinOp::Eq => (l == r) as i16,
        BinOp::Neq => (l != r) as i16,
        BinOp::Lt => (l < r) as i16,
        BinOp::Gt => (l > r) as i16,
        BinOp::Leq => (l <= r) as i16,
        BinOp::Geq => (l >= r) as i16,
        BinOp::And => ((l != 0) && (r != 0)) as i16,
        BinOp::Or => ((l != 0) || (r != 0)) as i16,
        BinOp::BitAnd => l & r,
        BinOp::BitOr => l | r,
        BinOp::Shl => l.wrapping_shl(r as u32),
        BinOp::Shr => l.wrapping_shr(r as u32),
    })
}

enum Simplified {
    Replace(Value),
    Const(i16),
    ConstBool(bool),
}

fn algebraic_simplify(
    op: BinOp,
    lhs: Value,
    rhs: Value,
    is_fixed: bool,
    consts: &HashMap<Value, i16>,
    bools: &HashMap<Value, bool>,
    inst: &mut LirInst,
) -> Option<Simplified> {
    let l_const = consts.get(&lhs).copied();
    let r_const = consts.get(&rhs).copied();
    let l_bool = bools.get(&lhs).copied();
    let r_bool = bools.get(&rhs).copied();

    let one = if is_fixed { 128 } else { 1 };

    match op {
        // x + 0, 0 + x -> x
        BinOp::Add => {
            if r_const == Some(0) {
                return Some(Simplified::Replace(lhs));
            }
            if l_const == Some(0) {
                return Some(Simplified::Replace(rhs));
            }
        }
        // x - 0 -> x
        BinOp::Sub if r_const == Some(0) => {
            return Some(Simplified::Replace(lhs));
        }
        // x * 1, 1 * x -> x. x * 0, 0 * x -> 0
        BinOp::Mul => {
            if r_const == Some(one) {
                return Some(Simplified::Replace(lhs));
            }
            if l_const == Some(one) {
                return Some(Simplified::Replace(rhs));
            }
            if r_const == Some(0) {
                *inst = LirInst::Const(0);
                return Some(Simplified::Const(0));
            }
            if l_const == Some(0) {
                *inst = LirInst::Const(0);
                return Some(Simplified::Const(0));
            }
        }
        // x / 1 -> x
        BinOp::Div if r_const == Some(one) => {
            return Some(Simplified::Replace(lhs));
        }
        // x and true, true and x -> x. x and false, false and x -> false
        BinOp::And => {
            if r_bool == Some(true) {
                return Some(Simplified::Replace(lhs));
            }
            if l_bool == Some(true) {
                return Some(Simplified::Replace(rhs));
            }
            if r_bool == Some(false) {
                *inst = LirInst::ConstBool(false);
                return Some(Simplified::ConstBool(false));
            }
            if l_bool == Some(false) {
                *inst = LirInst::ConstBool(false);
                return Some(Simplified::ConstBool(false));
            }
        }
        // x or false, false or x -> x. x or true, true or x -> true
        BinOp::Or => {
            if r_bool == Some(false) {
                return Some(Simplified::Replace(lhs));
            }
            if l_bool == Some(false) {
                return Some(Simplified::Replace(rhs));
            }
            if r_bool == Some(true) {
                *inst = LirInst::ConstBool(true);
                return Some(Simplified::ConstBool(true));
            }
            if l_bool == Some(true) {
                *inst = LirInst::ConstBool(true);
                return Some(Simplified::ConstBool(true));
            }
        }
        _ => {}
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::compiler::ast::BinOp;
    use crate::compiler::lir::*;
    use crate::compiler::test_helpers::*;

    #[test]
    fn fold_add() {
        let m = optimized_lir("fn f(): int\n  return 3 + 5\nend");
        let f = find_func(&m, "f");
        assert!(has_inst(f, |i| matches!(i, LirInst::Const(8))));
        assert!(!has_inst(f, |i| matches!(i, LirInst::BinOp { .. })));
    }

    #[test]
    fn fold_comparison() {
        let m = optimized_lir("fn f(): bool\n  return 3 < 5\nend");
        let f = find_func(&m, "f");
        assert!(has_inst(f, |i| matches!(i, LirInst::ConstBool(true))));
        assert!(!has_inst(f, |i| matches!(i, LirInst::BinOp { .. })));
    }

    #[test]
    fn fold_neg() {
        let m = optimized_lir("fn f(): int\n  return -5\nend");
        let f = find_func(&m, "f");
        assert!(has_inst(f, |i| matches!(i, LirInst::Const(-5))));
    }

    #[test]
    fn fold_not() {
        let m = optimized_lir("fn f(): bool\n  return not true\nend");
        let f = find_func(&m, "f");
        assert!(has_inst(f, |i| matches!(i, LirInst::ConstBool(false))));
    }

    #[test]
    fn fold_chain() {
        let m = optimized_lir("fn f(): int\n  return 1 + 2 + 3\nend");
        let f = find_func(&m, "f");
        assert!(has_inst(f, |i| matches!(i, LirInst::Const(6))));
        assert!(!has_inst(f, |i| matches!(i, LirInst::BinOp { .. })));
    }

    #[test]
    fn identity_mul_one() {
        let m = optimized_lir("fn f(x: int): int\n  return x * 1\nend");
        let f = find_func(&m, "f");
        assert!(!has_inst(f, |i| matches!(
            i,
            LirInst::BinOp { op: BinOp::Mul, .. }
        )));
    }

    #[test]
    fn identity_add_zero() {
        let m = optimized_lir("fn f(x: int): int\n  return x + 0\nend");
        let f = find_func(&m, "f");
        assert!(!has_inst(f, |i| matches!(
            i,
            LirInst::BinOp { op: BinOp::Add, .. }
        )));
    }

    #[test]
    fn mul_zero() {
        let m = optimized_lir("fn f(x: int): int\n  return x * 0\nend");
        let f = find_func(&m, "f");
        assert!(has_inst(f, |i| matches!(i, LirInst::Const(0))));
    }

    #[test]
    fn div_by_zero_unchanged() {
        let m = optimized_lir("fn f(): int\n  return 1 / 0\nend");
        let f = find_func(&m, "f");
        assert!(has_inst(f, |i| matches!(i, LirInst::BinOp { .. })));
    }
}
