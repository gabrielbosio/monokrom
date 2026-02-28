use std::collections::{HashMap, HashSet};

use crate::compiler::ast::BinOp;
use crate::compiler::hir::{HirExpr, HirExprKind, HirFunc, HirModule, HirStmt, HirType};
use crate::compiler::lir::{BasicBlock, BlockId, LirFunc, LirInst, LirModule, Terminator, Value};

struct LowerCtx<'a> {
    module: &'a HirModule,
    next_val: u32,
    next_block: u32,
    blocks: Vec<BasicBlock>,
    current_block: BlockId,
    current_insts: Vec<(Value, LirInst)>,
    block_terminated: bool,

    // Braun et al. SSA construction
    current_def: HashMap<BlockId, HashMap<String, Value>>,
    predecessors: HashMap<BlockId, Vec<BlockId>>,
    sealed: HashSet<BlockId>,
    incomplete_phis: HashMap<BlockId, Vec<(String, Value)>>,

    // Track phi values for trivial-phi removal
    phi_values: HashMap<Value, (BlockId, String)>,

    // Value replacements from trivial phi removal
    replacements: HashMap<Value, Value>,

    // Loops
    loop_stack: Vec<(BlockId, BlockId)>,

    // Compound local addresses (static allocation)
    compound_local_addrs: HashMap<String, u16>,
    next_compound_addr: u16,
}

impl<'a> LowerCtx<'a> {
    fn new(module: &'a HirModule, globals_size: u16) -> Self {
        let entry = BlockId(0);
        let mut ctx = Self {
            module,
            next_val: 0,
            next_block: 1,
            blocks: Vec::new(),
            current_block: entry,
            current_insts: Vec::new(),
            block_terminated: false,
            current_def: HashMap::new(),
            predecessors: HashMap::new(),
            sealed: HashSet::new(),
            incomplete_phis: HashMap::new(),
            phi_values: HashMap::new(),
            replacements: HashMap::new(),
            loop_stack: Vec::new(),
            compound_local_addrs: HashMap::new(),
            next_compound_addr: globals_size,
        };
        ctx.current_def.insert(entry, HashMap::new());
        ctx.sealed.insert(entry);
        ctx
    }

    fn fresh_val(&mut self) -> Value {
        let v = Value(self.next_val);
        self.next_val += 1;
        v
    }

    fn fresh_block(&mut self) -> BlockId {
        let b = BlockId(self.next_block);
        self.next_block += 1;
        self.current_def.insert(b, HashMap::new());
        b
    }

    fn emit(&mut self, inst: LirInst) -> Value {
        let v = self.fresh_val();
        self.current_insts.push((v, inst));
        v
    }

    fn finish_block(&mut self, terminator: Terminator) {
        if self.block_terminated {
            return;
        }
        self.block_terminated = true;
        let id = self.current_block;
        let insts = std::mem::take(&mut self.current_insts);
        self.blocks.push(BasicBlock {
            id,
            insts,
            terminator,
        });
    }

    fn start_block(&mut self, id: BlockId) {
        self.current_block = id;
        self.current_insts = Vec::new();
        self.block_terminated = false;
    }

    fn add_predecessor(&mut self, block: BlockId, pred: BlockId) {
        self.predecessors.entry(block).or_default().push(pred);
    }

    // --- SSA construction (Braun et al. 2013, simplified) ---

    fn write_variable(&mut self, var: &str, block: BlockId, val: Value) {
        self.current_def
            .entry(block)
            .or_default()
            .insert(var.to_string(), val);
    }

    fn read_variable(&mut self, var: &str, block: BlockId) -> Value {
        if let Some(val) = self.current_def.get(&block).and_then(|m| m.get(var)) {
            return *val;
        }
        self.read_variable_recursive(var, block)
    }

    fn read_variable_recursive(&mut self, var: &str, block: BlockId) -> Value {
        let preds = self.predecessors.get(&block).cloned().unwrap_or_default();
        let val = if !self.sealed.contains(&block) {
            // Incomplete CFG, so place incomplete phi
            let phi_val = self.fresh_val();
            self.incomplete_phis
                .entry(block)
                .or_default()
                .push((var.to_string(), phi_val));
            self.phi_values.insert(phi_val, (block, var.to_string()));
            phi_val
        } else if preds.len() == 1 {
            self.read_variable(var, preds[0])
        } else {
            // Multiple predecessors, so insert phi
            let phi_val = self.fresh_val();
            self.write_variable(var, block, phi_val);
            self.phi_values.insert(phi_val, (block, var.to_string()));
            let operands: Vec<(BlockId, Value)> = preds
                .iter()
                .map(|&p| (p, self.read_variable(var, p)))
                .collect();
            let resolved = self.add_phi_to_block(block, phi_val, operands);
            // If the phi was trivial, replace all stale references to the dead
            // phi_val that were cached in intermediate blocks during operand
            // collection.
            if resolved != phi_val {
                for defs in self.current_def.values_mut() {
                    for v in defs.values_mut() {
                        if *v == phi_val {
                            *v = resolved;
                        }
                    }
                }
            }
            resolved
        };
        self.write_variable(var, block, val);
        val
    }

    fn add_phi_to_block(
        &mut self,
        block: BlockId,
        phi_val: Value,
        operands: Vec<(BlockId, Value)>,
    ) -> Value {
        // Check if trivial
        if let Some(trivial) = self.try_remove_trivial_phi(phi_val, &operands) {
            self.replacements.insert(phi_val, trivial);
            return trivial;
        }
        // Insert phi at the beginning of the block
        let inst = LirInst::Phi(operands);
        if let Some(b) = self.blocks.iter_mut().find(|b| b.id == block) {
            b.insts.insert(0, (phi_val, inst));
        } else if self.current_block == block {
            self.current_insts.insert(0, (phi_val, inst));
        }
        phi_val
    }

    fn try_remove_trivial_phi(
        &self,
        phi_val: Value,
        operands: &[(BlockId, Value)],
    ) -> Option<Value> {
        let mut same: Option<Value> = None;
        for &(_, val) in operands {
            if val == phi_val {
                continue; // self-reference
            }
            if let Some(s) = same {
                if s != val {
                    return None; // non-trivial: multiple distinct values
                }
            } else {
                same = Some(val);
            }
        }
        same
    }

    fn seal_block(&mut self, block: BlockId) {
        let incomplete = self.incomplete_phis.remove(&block).unwrap_or_default();
        for (var, phi_val) in incomplete {
            let preds = self.predecessors.get(&block).cloned().unwrap_or_default();
            let operands: Vec<(BlockId, Value)> = preds
                .iter()
                .map(|&p| (p, self.read_variable(&var, p)))
                .collect();
            let resolved = self.add_phi_to_block(block, phi_val, operands);
            if resolved != phi_val {
                self.write_variable(&var, block, resolved);
                // Replace stale references to the dead phi_val that were
                // cached in other blocks during operand collection or earlier
                // reads before this block was sealed.
                for defs in self.current_def.values_mut() {
                    for v in defs.values_mut() {
                        if *v == phi_val {
                            *v = resolved;
                        }
                    }
                }
            }
        }
        self.sealed.insert(block);
    }

    fn apply_replacements(&mut self) {
        if self.replacements.is_empty() {
            return;
        }
        let replacements = self.replacements.clone();
        let resolve = |v: Value| -> Value {
            let mut val = v;
            while let Some(&r) = replacements.get(&val) {
                val = r;
            }
            val
        };
        for block in &mut self.blocks {
            for (_, inst) in &mut block.insts {
                inst.replace_values(&resolve);
            }
            block.terminator.replace_values(&resolve);
        }
    }

    // --- Type helpers ---

    fn type_size(&self, ty: &HirType) -> u8 {
        match ty {
            HirType::Int | HirType::Fixed | HirType::Str => 2,
            HirType::Bool => 1,
            HirType::Void => 0,
            HirType::Array(elem, count) => (self.type_size(elem) as u16 * (*count as u16)) as u8,
            HirType::Struct(name) => self
                .module
                .structs
                .iter()
                .find(|s| s.name == *name)
                .map(|s| s.size as u8)
                .unwrap_or(0),
        }
    }

    fn type_size_u16(&self, ty: &HirType) -> u16 {
        match ty {
            HirType::Int | HirType::Fixed | HirType::Str => 2,
            HirType::Bool => 1,
            HirType::Void => 0,
            HirType::Array(elem, count) => self.type_size_u16(elem) * (*count as u16),
            HirType::Struct(name) => self
                .module
                .structs
                .iter()
                .find(|s| s.name == *name)
                .map(|s| s.size)
                .unwrap_or(0),
        }
    }

    fn is_scalar(&self, ty: &HirType) -> bool {
        matches!(
            ty,
            HirType::Int | HirType::Fixed | HirType::Bool | HirType::Str
        )
    }

    fn find_global_addr(&self, name: &str) -> u16 {
        self.module
            .globals
            .iter()
            .find(|g| g.name == name)
            .map(|g| g.address)
            .unwrap_or(0)
    }

    fn find_global_type(&self, name: &str) -> &HirType {
        self.module
            .globals
            .iter()
            .find(|g| g.name == name)
            .map(|g| &g.ty)
            .unwrap_or(&HirType::Void)
    }

    // --- Lowering expressions ---

    fn lower_expr(&mut self, expr: &HirExpr) -> Value {
        match &expr.kind {
            HirExprKind::IntLit(n) => self.emit(LirInst::Const(*n)),
            HirExprKind::FixedLit(n) => self.emit(LirInst::Const(*n)),
            HirExprKind::BoolLit(b) => self.emit(LirInst::ConstBool(*b)),
            HirExprKind::StrLit(idx) => self.emit(LirInst::Const(*idx as i16)),

            HirExprKind::Var(name) => {
                if self.is_scalar(&expr.ty) {
                    self.read_variable(name, self.current_block)
                } else if self.compound_local_addrs.contains_key(name) {
                    let addr = self.compound_local_addrs[name];
                    self.emit(LirInst::GlobalAddr(addr))
                } else {
                    // Dynamic compound address (e.g. for-in element variable)
                    self.read_variable(name, self.current_block)
                }
            }

            HirExprKind::Global(name) => {
                let addr = self.find_global_addr(name);
                let addr_val = self.emit(LirInst::GlobalAddr(addr));
                if self.is_scalar(&expr.ty) {
                    let size = self.type_size(&expr.ty);
                    self.emit(LirInst::Load {
                        addr: addr_val,
                        size,
                    })
                } else {
                    addr_val
                }
            }

            HirExprKind::BinOp { op, lhs, rhs } => {
                let l = self.lower_expr(lhs);
                let r = self.lower_expr(rhs);
                self.emit(LirInst::BinOp {
                    op: *op,
                    lhs: l,
                    rhs: r,
                    is_fixed: lhs.ty == HirType::Fixed,
                })
            }

            HirExprKind::UnaryOp { op, expr: inner } => {
                let v = self.lower_expr(inner);
                self.emit(LirInst::UnaryOp { op: *op, val: v })
            }

            HirExprKind::Call { name, args } => {
                let arg_vals: Vec<Value> = args.iter().map(|a| self.lower_expr(a)).collect();
                self.emit(LirInst::Call {
                    name: name.clone(),
                    args: arg_vals,
                })
            }

            HirExprKind::Intrinsic { op, args } => {
                let arg_vals: Vec<Value> = args.iter().map(|a| self.lower_expr(a)).collect();
                self.emit(LirInst::Intrinsic {
                    op: *op,
                    args: arg_vals,
                })
            }

            HirExprKind::Index { expr: base, index } => {
                let addr = self.lower_addr_index(base, index, &expr.ty);
                if self.is_scalar(&expr.ty) {
                    let size = self.type_size(&expr.ty);
                    self.emit(LirInst::Load { addr, size })
                } else {
                    addr
                }
            }

            HirExprKind::FieldAccess {
                expr: base, offset, ..
            } => {
                let base_addr = self.lower_addr_of(base);
                let off = self.emit(LirInst::Const(*offset as i16));
                let addr = self.emit(LirInst::BinOp {
                    op: BinOp::Add,
                    lhs: base_addr,
                    rhs: off,
                    is_fixed: false,
                });
                if self.is_scalar(&expr.ty) {
                    let size = self.type_size(&expr.ty);
                    self.emit(LirInst::Load { addr, size })
                } else {
                    addr
                }
            }
        }
    }

    /// Get the memory address of an expression (for lvalues and compound access)
    fn lower_addr_of(&mut self, expr: &HirExpr) -> Value {
        match &expr.kind {
            HirExprKind::Global(name) => {
                let addr = self.find_global_addr(name);
                self.emit(LirInst::GlobalAddr(addr))
            }
            HirExprKind::Var(name) => {
                if self.compound_local_addrs.contains_key(name) {
                    let addr = self.compound_local_addrs[name];
                    self.emit(LirInst::GlobalAddr(addr))
                } else {
                    self.read_variable(name, self.current_block)
                }
            }
            HirExprKind::Index { expr: base, index } => {
                self.lower_addr_index(base, index, &expr.ty)
            }
            HirExprKind::FieldAccess {
                expr: base, offset, ..
            } => {
                let base_addr = self.lower_addr_of(base);
                let off = self.emit(LirInst::Const(*offset as i16));
                self.emit(LirInst::BinOp {
                    op: BinOp::Add,
                    lhs: base_addr,
                    rhs: off,
                    is_fixed: false,
                })
            }
            _ => self.lower_expr(expr),
        }
    }

    fn lower_addr_index(&mut self, base: &HirExpr, index: &HirExpr, elem_ty: &HirType) -> Value {
        let base_addr = self.lower_addr_of(base);
        let idx = self.lower_expr(index);
        let elem_size = self.type_size_u16(elem_ty);
        let size_val = self.emit(LirInst::Const(elem_size as i16));
        let offset = self.emit(LirInst::BinOp {
            op: BinOp::Mul,
            lhs: idx,
            rhs: size_val,
            is_fixed: false,
        });
        self.emit(LirInst::BinOp {
            op: BinOp::Add,
            lhs: base_addr,
            rhs: offset,
            is_fixed: false,
        })
    }

    // --- Lowering statements ---

    fn lower_stmt(&mut self, stmt: &HirStmt) {
        if self.block_terminated {
            return;
        }
        match stmt {
            HirStmt::VarDecl { name, ty, value } => {
                if self.is_scalar(ty) {
                    let val = if let Some(v) = value {
                        self.lower_expr(v)
                    } else {
                        match ty {
                            HirType::Bool => self.emit(LirInst::ConstBool(false)),
                            _ => self.emit(LirInst::Const(0)),
                        }
                    };
                    let block = self.current_block;
                    self.write_variable(name, block, val);
                }
                // Compound types: if initialized from a function return, use its address
                if !self.is_scalar(ty) {
                    if let Some(v) = value {
                        let val = self.lower_expr(v);
                        let block = self.current_block;
                        self.compound_local_addrs.remove(name.as_str());
                        self.write_variable(name, block, val);
                    }
                }
            }

            HirStmt::Assign { target, value } => {
                match &target.kind {
                    // Scalar local
                    HirExprKind::Var(name) if self.is_scalar(&target.ty) => {
                        let val = self.lower_expr(value);
                        let block = self.current_block;
                        self.write_variable(name, block, val);
                    }
                    // Compound local reassigned from a function return
                    HirExprKind::Var(name) if !self.is_scalar(&target.ty) => {
                        let val = self.lower_expr(value);
                        let block = self.current_block;
                        self.compound_local_addrs.remove(name.as_str());
                        self.write_variable(name, block, val);
                    }
                    // Global or compound field
                    _ => {
                        let addr = self.lower_addr_of(target);
                        let val = self.lower_expr(value);
                        let size = self.type_size(&target.ty);
                        self.emit(LirInst::Store { addr, val, size });
                    }
                }
            }

            HirStmt::If {
                cond,
                body,
                else_ifs,
                else_body,
            } => {
                self.lower_if(cond, body, else_ifs, else_body);
            }

            HirStmt::While { cond, body } => {
                self.lower_while(cond, body);
            }

            HirStmt::ForRange {
                var,
                start,
                end,
                inclusive,
                body,
            } => {
                self.lower_for_range(var, start, end, *inclusive, body);
            }

            HirStmt::ForIn {
                index,
                elem,
                iter,
                body,
            } => {
                self.lower_for_in(index, elem, iter, body);
            }

            HirStmt::Return(expr) => {
                let val = expr.as_ref().map(|e| self.lower_expr(e));
                self.finish_block(Terminator::Return(val));
                // Dead block for any code after return
                let dead = self.fresh_block();
                self.sealed.insert(dead);
                self.start_block(dead);
            }

            HirStmt::Break => {
                if let Some(&(_, exit)) = self.loop_stack.last() {
                    let cur = self.current_block;
                    self.add_predecessor(exit, cur);
                    self.finish_block(Terminator::Jump(exit));
                    let dead = self.fresh_block();
                    self.sealed.insert(dead);
                    self.start_block(dead);
                }
            }

            HirStmt::Continue => {
                if let Some(&(header, _)) = self.loop_stack.last() {
                    let cur = self.current_block;
                    self.add_predecessor(header, cur);
                    self.finish_block(Terminator::Jump(header));
                    let dead = self.fresh_block();
                    self.sealed.insert(dead);
                    self.start_block(dead);
                }
            }

            HirStmt::Expression(expr) => {
                self.lower_expr(expr);
            }
        }
    }

    fn lower_if(
        &mut self,
        cond: &HirExpr,
        body: &[HirStmt],
        else_ifs: &[(HirExpr, Vec<HirStmt>)],
        else_body: &[HirStmt],
    ) {
        let then_block = self.fresh_block();
        let merge_block = self.fresh_block();

        // First else-if or else or merge
        let first_else = if !else_ifs.is_empty() || !else_body.is_empty() {
            self.fresh_block()
        } else {
            merge_block
        };

        let cond_val = self.lower_expr(cond);
        let cur = self.current_block;
        self.add_predecessor(then_block, cur);
        self.add_predecessor(first_else, cur);
        self.finish_block(Terminator::Branch {
            cond: cond_val,
            then_block,
            else_block: first_else,
        });

        // Then block
        self.sealed.insert(then_block);
        self.start_block(then_block);
        for s in body {
            self.lower_stmt(s);
        }
        if !self.block_terminated {
            let cur = self.current_block;
            self.add_predecessor(merge_block, cur);
            self.finish_block(Terminator::Jump(merge_block));
        }

        // Else-if chain
        let mut current_else = first_else;
        for (i, (ei_cond, ei_body)) in else_ifs.iter().enumerate() {
            if current_else == merge_block {
                break;
            }
            self.sealed.insert(current_else);
            self.start_block(current_else);

            let ei_then = self.fresh_block();
            let ei_else = if i + 1 < else_ifs.len() || !else_body.is_empty() {
                self.fresh_block()
            } else {
                merge_block
            };

            let ei_cond_val = self.lower_expr(ei_cond);
            let cur = self.current_block;
            self.add_predecessor(ei_then, cur);
            self.add_predecessor(ei_else, cur);
            self.finish_block(Terminator::Branch {
                cond: ei_cond_val,
                then_block: ei_then,
                else_block: ei_else,
            });

            self.sealed.insert(ei_then);
            self.start_block(ei_then);
            for s in ei_body {
                self.lower_stmt(s);
            }
            if !self.block_terminated {
                let cur = self.current_block;
                self.add_predecessor(merge_block, cur);
                self.finish_block(Terminator::Jump(merge_block));
            }

            current_else = ei_else;
        }

        // Else block
        if !else_body.is_empty() && current_else != merge_block {
            self.sealed.insert(current_else);
            self.start_block(current_else);
            for s in else_body {
                self.lower_stmt(s);
            }
            if !self.block_terminated {
                let cur = self.current_block;
                self.add_predecessor(merge_block, cur);
                self.finish_block(Terminator::Jump(merge_block));
            }
        }

        self.sealed.insert(merge_block);
        self.start_block(merge_block);
    }

    fn lower_while(&mut self, cond: &HirExpr, body: &[HirStmt]) {
        let header = self.fresh_block();
        let body_block = self.fresh_block();
        let exit = self.fresh_block();

        // Jump from current block to header
        let cur = self.current_block;
        self.add_predecessor(header, cur);
        self.finish_block(Terminator::Jump(header));

        // Header is unsealed (back-edge not yet known)
        self.start_block(header);
        let cond_val = self.lower_expr(cond);
        let cur = self.current_block;
        self.add_predecessor(body_block, cur);
        self.add_predecessor(exit, cur);
        self.finish_block(Terminator::Branch {
            cond: cond_val,
            then_block: body_block,
            else_block: exit,
        });

        // Body
        self.sealed.insert(body_block);
        self.start_block(body_block);
        self.loop_stack.push((header, exit));
        for s in body {
            self.lower_stmt(s);
        }
        self.loop_stack.pop();
        if !self.block_terminated {
            let cur = self.current_block;
            self.add_predecessor(header, cur);
            self.finish_block(Terminator::Jump(header));
        }

        // Seal header now that all predecessors are known
        self.seal_block(header);

        self.sealed.insert(exit);
        self.start_block(exit);
    }

    fn lower_for_range(
        &mut self,
        var: &str,
        start: &HirExpr,
        end: &HirExpr,
        inclusive: bool,
        body: &[HirStmt],
    ) {
        // Init counter
        let start_val = self.lower_expr(start);
        let end_val = self.lower_expr(end);
        let block = self.current_block;
        self.write_variable(var, block, start_val);

        let header = self.fresh_block();
        let body_block = self.fresh_block();
        let exit = self.fresh_block();

        let cur = self.current_block;
        self.add_predecessor(header, cur);
        self.finish_block(Terminator::Jump(header));

        // Header: check condition
        self.start_block(header);
        let counter = self.read_variable(var, header);
        let cmp_op = if inclusive { BinOp::Leq } else { BinOp::Lt };
        let cond = self.emit(LirInst::BinOp {
            op: cmp_op,
            lhs: counter,
            rhs: end_val,
            is_fixed: false,
        });
        let cur = self.current_block;
        self.add_predecessor(body_block, cur);
        self.add_predecessor(exit, cur);
        self.finish_block(Terminator::Branch {
            cond,
            then_block: body_block,
            else_block: exit,
        });

        // Latch block: increment counter then jump to header
        let latch = self.fresh_block();

        // Body
        self.sealed.insert(body_block);
        self.start_block(body_block);
        self.loop_stack.push((latch, exit));
        for s in body {
            self.lower_stmt(s);
        }
        self.loop_stack.pop();
        if !self.block_terminated {
            let cur = self.current_block;
            self.add_predecessor(latch, cur);
            self.finish_block(Terminator::Jump(latch));
        }

        // Latch: increment counter
        self.seal_block(latch);
        self.start_block(latch);
        let cur_counter = self.read_variable(var, self.current_block);
        let one = self.emit(LirInst::Const(1));
        let next = self.emit(LirInst::BinOp {
            op: BinOp::Add,
            lhs: cur_counter,
            rhs: one,
            is_fixed: false,
        });
        let block = self.current_block;
        self.write_variable(var, block, next);
        self.add_predecessor(header, block);
        self.finish_block(Terminator::Jump(header));

        self.seal_block(header);
        self.sealed.insert(exit);
        self.start_block(exit);
    }

    fn lower_for_in(&mut self, index_var: &str, elem_var: &str, iter: &HirExpr, body: &[HirStmt]) {
        let (elem_ty, count) = match &iter.ty {
            HirType::Array(elem, count) => (elem.as_ref().clone(), *count),
            _ => return,
        };

        // Init index = 0
        let zero = self.emit(LirInst::Const(0));
        let count_val = self.emit(LirInst::Const(count));
        let block = self.current_block;
        self.write_variable(index_var, block, zero);

        let header = self.fresh_block();
        let body_block = self.fresh_block();
        let exit = self.fresh_block();

        let cur = self.current_block;
        self.add_predecessor(header, cur);
        self.finish_block(Terminator::Jump(header));

        // Header: index < count
        self.start_block(header);
        let idx = self.read_variable(index_var, header);
        let cond = self.emit(LirInst::BinOp {
            op: BinOp::Lt,
            lhs: idx,
            rhs: count_val,
            is_fixed: false,
        });
        let cur = self.current_block;
        self.add_predecessor(body_block, cur);
        self.add_predecessor(exit, cur);
        self.finish_block(Terminator::Branch {
            cond,
            then_block: body_block,
            else_block: exit,
        });

        // Body: load element
        self.sealed.insert(body_block);
        self.start_block(body_block);

        // Load element if not wildcard
        if elem_var != "_" {
            let cur_idx = self.read_variable(index_var, self.current_block);
            let base_addr = self.lower_addr_of(iter);
            let elem_size = self.type_size_u16(&elem_ty);
            let size_val = self.emit(LirInst::Const(elem_size as i16));
            let offset = self.emit(LirInst::BinOp {
                op: BinOp::Mul,
                lhs: cur_idx,
                rhs: size_val,
                is_fixed: false,
            });
            let elem_addr = self.emit(LirInst::BinOp {
                op: BinOp::Add,
                lhs: base_addr,
                rhs: offset,
                is_fixed: false,
            });
            if self.is_scalar(&elem_ty) {
                let size = self.type_size(&elem_ty);
                let elem_val = self.emit(LirInst::Load {
                    addr: elem_addr,
                    size,
                });
                let block = self.current_block;
                self.write_variable(elem_var, block, elem_val);
            } else {
                let block = self.current_block;
                self.write_variable(elem_var, block, elem_addr);
            }
        }

        // Latch block: increment index then jump to header
        let latch = self.fresh_block();

        self.loop_stack.push((latch, exit));
        for s in body {
            self.lower_stmt(s);
        }
        self.loop_stack.pop();

        if !self.block_terminated {
            let cur = self.current_block;
            self.add_predecessor(latch, cur);
            self.finish_block(Terminator::Jump(latch));
        }

        // Latch: increment index
        self.seal_block(latch);
        self.start_block(latch);
        let cur_idx = self.read_variable(index_var, self.current_block);
        let one = self.emit(LirInst::Const(1));
        let next_idx = self.emit(LirInst::BinOp {
            op: BinOp::Add,
            lhs: cur_idx,
            rhs: one,
            is_fixed: false,
        });
        let block = self.current_block;
        self.write_variable(index_var, block, next_idx);
        self.add_predecessor(header, block);
        self.finish_block(Terminator::Jump(header));

        self.seal_block(header);
        self.sealed.insert(exit);
        self.start_block(exit);
    }

    fn lower_stmts(&mut self, stmts: &[HirStmt]) {
        for s in stmts {
            self.lower_stmt(s);
        }
    }
}

fn alloc_compound_locals(ctx: &mut LowerCtx, func: &HirFunc) {
    for (name, ty) in &func.locals {
        if !ctx.is_scalar(ty) {
            let addr = ctx.next_compound_addr;
            ctx.next_compound_addr += ctx.type_size_u16(ty);
            ctx.compound_local_addrs.insert(name.clone(), addr);
        }
    }
}

fn lower_function(module: &HirModule, func: &HirFunc, globals_size: u16) -> (LirFunc, u16) {
    let mut ctx = LowerCtx::new(module, globals_size);

    // Allocate compound locals
    alloc_compound_locals(&mut ctx, func);

    // Params get the first N values
    let mut params = Vec::new();
    for p in &func.params {
        let v = ctx.fresh_val();
        let block = ctx.current_block;
        ctx.write_variable(&p.name, block, v);
        params.push(v);
    }

    // If this is main(), inline global initializers first
    if func.name == "main" {
        // Global inits with explicit init values
        for g in &module.globals {
            if let Some(init) = &g.init {
                let val = ctx.lower_expr(init);
                let addr = ctx.emit(LirInst::GlobalAddr(g.address));
                let size = ctx.type_size(&g.ty);
                ctx.emit(LirInst::Store { addr, val, size });
            }
        }
        // global_init statements
        ctx.lower_stmts(&module.global_init);
    }

    // Lower function body
    ctx.lower_stmts(&func.body);

    // Ensure the last block is terminated
    if !ctx.block_terminated {
        ctx.finish_block(Terminator::Return(None));
    }

    ctx.apply_replacements();

    let compound_end = ctx.next_compound_addr;

    (
        LirFunc {
            name: func.name.clone(),
            params,
            blocks: ctx.blocks,
        },
        compound_end,
    )
}

pub fn lower_to_lir(module: &HirModule) -> LirModule {
    let globals_size = module
        .globals
        .iter()
        .map(|g| g.address + super::type_check::type_size_standalone(&g.ty, &module.structs))
        .max()
        .unwrap_or(0);

    let mut functions = Vec::new();
    let mut max_compound_end = globals_size;

    for func in &module.functions {
        let (lir_func, compound_end) = lower_function(module, func, globals_size);
        if compound_end > max_compound_end {
            max_compound_end = compound_end;
        }
        functions.push(lir_func);
    }

    LirModule {
        functions,
        string_pool: module.string_pool.clone(),
        globals_size: max_compound_end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::hir::Intrinsic;
    use crate::compiler::lir::*;
    use crate::compiler::{lower, parse};

    fn lir(src: &str) -> LirModule {
        let ast = parse(src).unwrap();
        let hir = lower(&ast).unwrap();
        lower_to_lir(&hir)
    }

    fn find_func<'a>(m: &'a LirModule, name: &str) -> &'a LirFunc {
        m.functions.iter().find(|f| f.name == name).unwrap()
    }

    #[test]
    fn empty_function() {
        let m = lir("fn f()\nend");
        let f = find_func(&m, "f");
        assert_eq!(f.blocks.len(), 1);
        assert_eq!(f.blocks[0].terminator, Terminator::Return(None));
    }

    #[test]
    fn constant_return() {
        let m = lir("fn f(): int\n  return 42\nend");
        let f = find_func(&m, "f");
        // Should have Const(42) and Return(Some(..))
        let block = &f.blocks[0];
        assert!(block
            .insts
            .iter()
            .any(|(_, inst)| matches!(inst, LirInst::Const(42))));
        assert!(matches!(block.terminator, Terminator::Return(Some(_))));
    }

    #[test]
    fn binary_op() {
        let m = lir("fn f(a: int, b: int): int\n  return a + b\nend");
        let f = find_func(&m, "f");
        assert_eq!(f.params.len(), 2);
        let block = &f.blocks[0];
        assert!(block
            .insts
            .iter()
            .any(|(_, inst)| matches!(inst, LirInst::BinOp { op: BinOp::Add, .. })));
        assert!(matches!(block.terminator, Terminator::Return(Some(_))));
    }

    #[test]
    fn if_simple() {
        let m = lir("fn f(x: bool)\n  if x\n    cls(0)\n  end\nend");
        let f = find_func(&m, "f");
        // Should have 3 blocks: entry (branch), then, merge
        assert!(f.blocks.len() >= 3);
        assert!(f
            .blocks
            .iter()
            .any(|b| matches!(b.terminator, Terminator::Branch { .. })));
    }

    #[test]
    fn if_else() {
        let m = lir("fn f(x: bool)\n  if x\n    cls(0)\n  else\n    cls(1)\n  end\nend");
        let f = find_func(&m, "f");
        // entry (branch), then, else, merge = 4 blocks
        assert!(f.blocks.len() >= 4);
    }

    #[test]
    fn while_loop() {
        let m = lir("fn f()\n  x: int = 0\n  while x < 10\n    x = x + 1\n  end\nend");
        let f = find_func(&m, "f");
        // Should have phi at header for x
        let has_phi = f.blocks.iter().any(|b| {
            b.insts
                .iter()
                .any(|(_, inst)| matches!(inst, LirInst::Phi(_)))
        });
        assert!(has_phi);
        // Should have back-edge Jump to header
        let header_id = f.blocks[1].id;
        let has_back_edge = f.blocks.iter().any(|b| {
            b.id != f.blocks[0].id
                && matches!(b.terminator, Terminator::Jump(target) if target == header_id)
        });
        assert!(has_back_edge);
    }

    #[test]
    fn for_range() {
        let m = lir("fn f()\n  for i in 0..5\n    cls(i)\n  end\nend");
        let f = find_func(&m, "f");
        // Should desugar to while with counter phi
        let has_phi = f.blocks.iter().any(|b| {
            b.insts
                .iter()
                .any(|(_, inst)| matches!(inst, LirInst::Phi(_)))
        });
        assert!(has_phi);
    }

    #[test]
    fn for_in() {
        let m =
            lir("enemies: array[3] of int\nfn f()\n  for i, e in enemies\n    cls(e)\n  end\nend");
        let f = find_func(&m, "f");
        // Should have Load for element
        let has_load = f.blocks.iter().any(|b| {
            b.insts
                .iter()
                .any(|(_, inst)| matches!(inst, LirInst::Load { .. }))
        });
        assert!(has_load);
    }

    #[test]
    fn break_continue() {
        let m = lir("fn f()\n  while true\n    break\n  end\nend");
        let f = find_func(&m, "f");
        // break should jump to exit block
        let exit_block = f
            .blocks
            .iter()
            .find(|b| matches!(b.terminator, Terminator::Return(None)));
        assert!(exit_block.is_some());
    }

    #[test]
    fn global_read_write() {
        let m = lir("x: int\nfn f()\n  x = 5\n  cls(x)\nend");
        let f = find_func(&m, "f");
        // Should have GlobalAddr and Store for x = 5, GlobalAddr and Load for cls(x)
        let has_store = f.blocks.iter().any(|b| {
            b.insts
                .iter()
                .any(|(_, inst)| matches!(inst, LirInst::Store { .. }))
        });
        let has_load = f.blocks.iter().any(|b| {
            b.insts
                .iter()
                .any(|(_, inst)| matches!(inst, LirInst::Load { .. }))
        });
        assert!(has_store);
        assert!(has_load);
    }

    #[test]
    fn global_init() {
        let m = lir("x: int = 42\nfn main()\nend");
        let f = find_func(&m, "main");
        // main() should start with GlobalAddr + Store for x = 42
        let block = &f.blocks[0];
        let has_const_42 = block
            .insts
            .iter()
            .any(|(_, inst)| matches!(inst, LirInst::Const(42)));
        let has_store = block
            .insts
            .iter()
            .any(|(_, inst)| matches!(inst, LirInst::Store { .. }));
        assert!(has_const_42);
        assert!(has_store);
    }

    #[test]
    fn field_access() {
        let m = lir("struct P\n  x: int\n  y: int\nend\np: P\nfn f()\n  p.x = 10\nend");
        let f = find_func(&m, "f");
        // Should have addr + offset + store
        let has_store = f.blocks.iter().any(|b| {
            b.insts
                .iter()
                .any(|(_, inst)| matches!(inst, LirInst::Store { .. }))
        });
        assert!(has_store);
    }

    #[test]
    fn intrinsic_call() {
        let m = lir("fn f()\n  cls(0)\nend");
        let f = find_func(&m, "f");
        let has_intrinsic = f.blocks.iter().any(|b| {
            b.insts.iter().any(|(_, inst)| {
                matches!(
                    inst,
                    LirInst::Intrinsic {
                        op: Intrinsic::Cls,
                        ..
                    }
                )
            })
        });
        assert!(has_intrinsic);
    }

    #[test]
    fn function_call() {
        let m = lir(
            "fn add(a: int, b: int): int\n  return a + b\nend\nfn f(): int\n  return add(1, 2)\nend",
        );
        let f = find_func(&m, "f");
        let has_call = f.blocks.iter().any(|b| {
            b.insts
                .iter()
                .any(|(_, inst)| matches!(inst, LirInst::Call { .. }))
        });
        assert!(has_call);
    }
}
