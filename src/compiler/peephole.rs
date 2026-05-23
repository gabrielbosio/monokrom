use std::collections::HashMap;

use crate::compiler::bytecode::*;

pub fn optimize(bc: &mut Bytecode) {
    if bc.functions.is_empty() {
        return;
    }

    // Collect removal ranges across all functions
    let mut remove = vec![false; bc.code.len()];
    let mut any_removed = false;

    for fi in 0..bc.functions.len() {
        let start = bc.functions[fi].code_offset;
        let end = func_end(bc, fi);
        let insts = parse_insts(&bc.code, start, end);

        // Count OP_LOAD_LOCAL per slot within this function
        let mut load_count: HashMap<u16, u32> = HashMap::new();
        for &(off, op, _) in &insts {
            if op == OP_LOAD_LOCAL {
                let slot = u16::from_le_bytes([bc.code[off + 1], bc.code[off + 2]]);
                *load_count.entry(slot).or_default() += 1;
            }
        }

        // Find removable adjacent OP_STORE_LOCAL N / OP_LOAD_LOCAL N pairs
        for i in 0..insts.len().saturating_sub(1) {
            let (off_a, op_a, sz_a) = insts[i];
            let (off_b, op_b, sz_b) = insts[i + 1];
            if op_a == OP_STORE_LOCAL && op_b == OP_LOAD_LOCAL {
                let slot_a = u16::from_le_bytes([bc.code[off_a + 1], bc.code[off_a + 2]]);
                let slot_b = u16::from_le_bytes([bc.code[off_b + 1], bc.code[off_b + 2]]);
                if slot_a == slot_b && load_count.get(&slot_a).copied().unwrap_or(0) == 1 {
                    remove[off_a..off_a + sz_a].fill(true);
                    remove[off_b..off_b + sz_b].fill(true);
                    any_removed = true;
                }
            }
        }
    }

    if !any_removed {
        return;
    }

    // Build old-to-new position map
    let old_to_new = build_offset_map(&remove);

    // Build new code, fixing jumps
    let mut new_code = Vec::with_capacity(bc.code.len());
    let mut pc = 0;
    while pc < bc.code.len() {
        let op = bc.code[pc];
        let sz = inst_size(op);
        if remove[pc] {
            pc += sz;
            continue;
        }
        match op {
            OP_JUMP | OP_JUMP_IF_FALSE => {
                new_code.push(op);
                let rel = i16::from_le_bytes([bc.code[pc + 1], bc.code[pc + 2]]);
                let old_target = (pc + 3).wrapping_add(rel as usize);
                let new_pc = old_to_new[pc];
                let new_target = old_to_new[old_target];
                let new_rel = new_target as isize - (new_pc + 3) as isize;
                let bytes = (new_rel as i16).to_le_bytes();
                new_code.push(bytes[0]);
                new_code.push(bytes[1]);
            }
            _ => {
                for j in 0..sz {
                    new_code.push(bc.code[pc + j]);
                }
            }
        }
        pc += sz;
    }

    bc.code = new_code;

    // Update all function offsets first
    for fi in 0..bc.functions.len() {
        let old_offset = bc.functions[fi].code_offset;
        bc.functions[fi].code_offset = old_to_new[old_offset];
    }

    // Recount n_locals with corrected offsets
    for fi in 0..bc.functions.len() {
        let start = bc.functions[fi].code_offset;
        let end = func_end(bc, fi);
        let mut max_slot: Option<u16> = None;
        let insts = parse_insts(&bc.code, start, end);
        for &(off, op, _) in &insts {
            if op == OP_STORE_LOCAL || op == OP_LOAD_LOCAL {
                let slot = u16::from_le_bytes([bc.code[off + 1], bc.code[off + 2]]);
                max_slot = Some(max_slot.map_or(slot, |m: u16| m.max(slot)));
            }
        }
        let n_params = bc.functions[fi].n_params as u16;
        bc.functions[fi].n_locals = match max_slot {
            Some(s) => (s + 1).max(n_params),
            None => n_params,
        };
    }

    // Update entry point
    if let Some(ep) = bc.entry_point {
        bc.entry_point = Some(old_to_new[ep]);
    }
}

fn func_end(bc: &Bytecode, fi: usize) -> usize {
    if fi + 1 < bc.functions.len() {
        bc.functions[fi + 1].code_offset
    } else {
        bc.code.len()
    }
}

fn parse_insts(code: &[u8], start: usize, end: usize) -> Vec<(usize, u8, usize)> {
    let mut insts = Vec::new();
    let mut pc = start;
    while pc < end {
        let op = code[pc];
        let sz = inst_size(op);
        insts.push((pc, op, sz));
        pc += sz;
    }
    insts
}

fn build_offset_map(remove: &[bool]) -> Vec<usize> {
    let mut map = Vec::with_capacity(remove.len() + 1);
    let mut removed = 0usize;
    for &r in remove {
        map.push(removed);
        if r {
            removed += 1;
        }
    }
    // One extra entry for positions == code.len() (jump targets can point past the last instruction)
    map.push(removed);
    // Convert: new_pos = old_pos - cumulative_removed_before
    for (i, m) in map.iter_mut().enumerate() {
        *m = i - *m;
    }
    map
}

#[cfg(test)]
mod tests {
    use crate::compiler::bytecode::*;
    use crate::compiler::compile;
    use crate::vm::{Vm, VmResult};

    #[test]
    fn store_load_eliminated() {
        let bc = compile("fn f(): int\n  return 42\nend").unwrap();
        let insts = super::parse_insts(&bc.code, 0, bc.code.len());
        for &(_, op, _) in &insts {
            assert!(op != OP_STORE_LOCAL, "OP_STORE_LOCAL should be eliminated");
            assert!(op != OP_LOAD_LOCAL, "OP_LOAD_LOCAL should be eliminated");
        }
    }

    #[test]
    fn multi_use_preserved() {
        // x = a + 1 produces a value used twice in x + x, so its OP_STORE_LOCAL must survive
        let src = "fn f(a: int): int\n  x: int = a + 1\n  return x + x\nend";
        let bc = compile(src).unwrap();
        let start = bc.functions[0].code_offset;
        let end = if bc.functions.len() > 1 {
            bc.functions[1].code_offset
        } else {
            bc.code.len()
        };
        let insts = super::parse_insts(&bc.code, start, end);
        let has_store = insts.iter().any(|&(_, op, _)| op == OP_STORE_LOCAL);
        assert!(
            has_store,
            "OP_STORE_LOCAL should be preserved for multi-use"
        );
    }

    #[test]
    fn jump_offsets_adjusted() {
        let src = "fn main()\n  x: int = 0\n  while x < 10\n    x = x + 1\n  end\n  flip()\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        let result = vm.run_until_flip().unwrap();
        assert_eq!(result, VmResult::Flip);
    }

    #[test]
    fn if_else_runs_correctly() {
        let src = "fn main()\n  x: int = 5\n  if x > 3\n    pset(0, 0, 3)\n  else\n    pset(0, 0, 1)\n  end\n  flip()\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        let result = vm.run_until_flip().unwrap();
        assert_eq!(result, VmResult::Flip);
        assert_eq!(vm.framebuffer[0], 3);
    }

    #[test]
    fn nested_while_compiled() {
        let src = "fn main()\n  x: int = 0\n  while x < 3\n    y: int = 0\n    while y < 3\n      pset(x, y, 3)\n      y = y + 1\n    end\n    x = x + 1\n  end\n  flip()\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        let result = vm.run_until_flip().unwrap();
        assert_eq!(result, VmResult::Flip);
        for x in 0..3 {
            for y in 0..3 {
                assert_eq!(vm.framebuffer[y * 160 + x], 3, "pixel at ({x},{y})");
            }
        }
    }

    #[test]
    fn function_call_after_peephole() {
        let src = "fn add(a: int, b: int): int\n  return a + b\nend\nfn main()\n  r: int = add(10, 20)\n  pset(r, 0, 3)\n  flip()\nend";
        let bc = compile(src).unwrap();
        let mut vm = Vm::new(&bc, 0.0).unwrap();
        let result = vm.run_until_flip().unwrap();
        assert_eq!(result, VmResult::Flip);
        assert_eq!(vm.framebuffer[30], 3);
    }

    #[test]
    fn bytecode_smaller_after_peephole() {
        // Compile with peephole (default) and compare to unoptimized
        let src = "fn main()\n  x: int = 0\n  while x < 10\n    pset(x, 0, 3)\n    x = x + 1\n  end\n  flip()\nend";
        let bc = compile(src).unwrap();
        // Without peephole this would have many OP_STORE_LOCAL/OP_LOAD_LOCAL pairs
        // Just verify it's reasonably small
        assert!(
            bc.code.len() < 100,
            "bytecode should be compact: {} bytes",
            bc.code.len()
        );
    }

    #[test]
    fn bouncing_ball_size() {
        let src = "fn main()\n  x = 80\n  y = 72\n  dx = 1\n  dy = 1\n  while true\n    cls(0)\n    if x > 160\n      dx = -1\n    else if x < 0\n      dx = 1\n    end\n    if y > 144\n      dy = -1\n    else if y < 0\n      dy = 1\n    end\n    y = y + dy\n    x = x + dx\n    circ(x, y, 5, 3)\n    flip()\n  end\nend";
        let bc = compile(src).unwrap();
        assert!(
            bc.code.len() <= 300,
            "bouncing ball should be <= 300 bytes, got {}",
            bc.code.len()
        );
    }
}
