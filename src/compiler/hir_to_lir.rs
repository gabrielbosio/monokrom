use std::collections::{HashMap, HashSet};

use crate::compiler::ast::BinOp;
use crate::compiler::hir::{
    deref_type, type_size, HirExpr, HirExprKind, HirFunc, HirModule, HirStmt, HirType, LocalId,
};
use crate::compiler::lir::{BasicBlock, BlockId, LirFunc, LirInst, LirModule, Terminator, Value};

/// A key in the SSA maps. Real locals use their binding id. The compiler uses
/// temporary keys for hidden loop counters that never appear in the HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LocalKey {
    Local(LocalId),
    Temp(u32),
}

struct LowerCtx<'a> {
    module: &'a HirModule,
    next_val: u32,
    next_block: u32,
    blocks: Vec<BasicBlock>,
    current_block: BlockId,
    current_insts: Vec<(Value, LirInst)>,
    block_terminated: bool,

    // Braun et al. SSA construction
    current_def: HashMap<BlockId, HashMap<LocalKey, Value>>,
    predecessors: HashMap<BlockId, Vec<BlockId>>,
    sealed: HashSet<BlockId>,
    incomplete_phis: HashMap<BlockId, Vec<(LocalKey, Value)>>,

    // Track phi values for trivial-phi removal
    phi_values: HashMap<Value, (BlockId, LocalKey)>,

    // Value replacements from trivial phi removal
    replacements: HashMap<Value, Value>,

    // Loops
    loop_stack: Vec<(BlockId, BlockId)>,

    // Byte offsets for frame-backed locals, relative to the frame byte area
    frame_local_addrs: HashMap<LocalId, u16>,
    next_byte_offset: u16,
    next_temp_local: u32,

    // Return type of the function being lowered
    ret_ty: HirType,

    // Hidden destination for compound return values
    hidden_dest: Option<Value>,
}

impl<'a> LowerCtx<'a> {
    fn new(module: &'a HirModule) -> Self {
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
            frame_local_addrs: HashMap::new(),
            next_byte_offset: 0,
            next_temp_local: 0,
            ret_ty: HirType::Void,
            hidden_dest: None,
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

    fn write_variable(&mut self, var: LocalKey, block: BlockId, val: Value) {
        self.current_def.entry(block).or_default().insert(var, val);
    }

    fn read_variable(&mut self, var: LocalKey, block: BlockId) -> Value {
        if let Some(val) = self.current_def.get(&block).and_then(|m| m.get(&var)) {
            return *val;
        }
        self.read_variable_recursive(var, block)
    }

    fn read_variable_recursive(&mut self, var: LocalKey, block: BlockId) -> Value {
        let preds = self.predecessors.get(&block).cloned().unwrap_or_default();
        let val = if !self.sealed.contains(&block) {
            // Incomplete CFG, so place incomplete phi
            let phi_val = self.fresh_val();
            self.incomplete_phis
                .entry(block)
                .or_default()
                .push((var, phi_val));
            self.phi_values.insert(phi_val, (block, var));
            phi_val
        } else if preds.len() == 1 {
            self.read_variable(var, preds[0])
        } else {
            // Multiple predecessors, so insert phi
            let phi_val = self.fresh_val();
            self.write_variable(var, block, phi_val);
            self.phi_values.insert(phi_val, (block, var));
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
                .map(|&p| (p, self.read_variable(var, p)))
                .collect();
            let resolved = self.add_phi_to_block(block, phi_val, operands);
            if resolved != phi_val {
                self.write_variable(var, block, resolved);
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

    fn type_size(&self, ty: &HirType) -> u16 {
        type_size(ty, &self.module.structs)
    }

    fn is_scalar(&self, ty: &HirType) -> bool {
        matches!(
            ty,
            HirType::Int | HirType::Fixed | HirType::Bool | HirType::Str | HirType::Ref(_)
        )
    }

    fn is_compound(&self, ty: &HirType) -> bool {
        matches!(ty, HirType::Array(..) | HirType::Struct(_))
    }

    fn function_ret_ty(&self, name: &str) -> Option<HirType> {
        self.module
            .functions
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.ret_type.clone())
    }

    fn returns_compound(&self, name: &str) -> bool {
        self.function_ret_ty(name)
            .map(|ty| self.is_compound(&ty))
            .unwrap_or(false)
    }

    /// Reserve a byte range in the current function's frame and return its offset.
    fn alloc_byte(&mut self, size: u16) -> u16 {
        let offset = self.next_byte_offset;
        self.next_byte_offset += size;
        offset
    }

    /// Reserve frame space for a temporary and return the value holding its address.
    fn alloc_temp(&mut self, size: u16) -> Value {
        let offset = self.alloc_byte(size);
        self.emit(LirInst::FrameAddr(offset))
    }

    fn fresh_temp_key(&mut self) -> LocalKey {
        let key = LocalKey::Temp(self.next_temp_local);
        self.next_temp_local += 1;
        key
    }

    /// Read a local from its frame slot when it has one. Scalars are loaded
    /// from memory, compound values are represented by their address.
    fn read_local(&mut self, id: LocalId, ty: &HirType) -> Value {
        if let Some(&offset) = self.frame_local_addrs.get(&id) {
            let addr = self.emit(LirInst::FrameAddr(offset));
            if self.is_scalar(ty) {
                let size = self.type_size(ty) as u8;
                self.emit(LirInst::Load { addr, size })
            } else {
                addr
            }
        } else {
            self.read_variable(LocalKey::Local(id), self.current_block)
        }
    }

    /// Write a local to its frame slot when it has one, otherwise update its
    /// SSA value. Compound values are copied from the source address.
    fn write_local(&mut self, id: LocalId, ty: &HirType, value: Value) {
        if let Some(&offset) = self.frame_local_addrs.get(&id) {
            let addr = self.emit(LirInst::FrameAddr(offset));
            if self.is_scalar(ty) {
                let size = self.type_size(ty) as u8;
                self.emit(LirInst::Store {
                    addr,
                    val: value,
                    size,
                });
            } else {
                let size = self.type_size(ty);
                self.emit_compound_copy(addr, value, size);
            }
        } else {
            self.write_variable(LocalKey::Local(id), self.current_block, value);
        }
    }

    /// Lower a compound expression directly into a frame-backed local.
    fn write_compound_local(&mut self, id: LocalId, ty: &HirType, expr: &HirExpr) {
        let offset = *self
            .frame_local_addrs
            .get(&id)
            .expect("compound local without frame slot");
        let dst = self.emit(LirInst::FrameAddr(offset));
        let size = self.type_size(ty);
        self.lower_compound_value_into(expr, dst, size);
    }

    fn addr_of_local(&mut self, id: LocalId, ty: &HirType) -> Value {
        if let Some(&offset) = self.frame_local_addrs.get(&id) {
            self.emit(LirInst::FrameAddr(offset))
        } else if matches!(ty, HirType::Ref(_)) {
            self.read_variable(LocalKey::Local(id), self.current_block)
        } else {
            panic!("address taken of non-address-taken local");
        }
    }

    fn read_binding(&mut self, key: LocalKey, ty: &HirType) -> Value {
        match key {
            LocalKey::Local(id) => self.read_local(id, ty),
            LocalKey::Temp(_) => self.read_variable(key, self.current_block),
        }
    }

    fn write_binding(&mut self, key: LocalKey, ty: &HirType, value: Value) {
        match key {
            LocalKey::Local(id) => self.write_local(id, ty, value),
            LocalKey::Temp(_) => self.write_variable(key, self.current_block, value),
        }
    }

    // Globals are validated by the type-checker, so a missing name here is a compiler bug.
    fn find_global_addr(&self, name: &str) -> u16 {
        self.module
            .globals
            .iter()
            .find(|g| g.name == name)
            .expect("unknown global in lowering")
            .address
    }

    // --- Lowering expressions ---

    fn lower_expr(&mut self, expr: &HirExpr) -> Value {
        match &expr.kind {
            HirExprKind::IntLit(n) => self.emit(LirInst::Const(*n)),
            HirExprKind::FixedLit(n) => self.emit(LirInst::Const(*n)),
            HirExprKind::BoolLit(b) => self.emit(LirInst::ConstBool(*b)),
            HirExprKind::StrLit(idx) => self.emit(LirInst::Const(*idx as i16)),

            HirExprKind::Var(id) => {
                if let HirType::Ref(inner) = &expr.ty {
                    // Ref variable: read the address it holds, auto-deref scalars
                    let addr = self.read_local(*id, &expr.ty);
                    if self.is_scalar(inner) {
                        let size = self.type_size(inner) as u8;
                        self.emit(LirInst::Load { addr, size })
                    } else {
                        addr // compound ref: address IS the value
                    }
                } else {
                    self.read_local(*id, &expr.ty)
                }
            }

            HirExprKind::Global(name) => {
                let addr = self.find_global_addr(name);
                let addr_val = self.emit(LirInst::GlobalAddr(addr));
                if self.is_scalar(&expr.ty) {
                    let size = self.type_size(&expr.ty) as u8;
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
                if self.returns_compound(name) {
                    let size = self.type_size(&expr.ty);
                    let tmp = self.alloc_temp(size);
                    self.emit_call(name, args, Some(tmp));
                    tmp
                } else {
                    self.emit_call(name, args, None)
                }
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
                    let size = self.type_size(&expr.ty) as u8;
                    self.emit(LirInst::Load { addr, size })
                } else {
                    addr
                }
            }

            HirExprKind::AddrOf(inner) => self.lower_addr_of(inner),

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
                    let size = self.type_size(&expr.ty) as u8;
                    self.emit(LirInst::Load { addr, size })
                } else {
                    addr
                }
            }
        }
    }

    /// Lower an expression that should produce a ref (address) value.
    /// Avoids auto-deref for Ref vars and returns the raw address.
    fn lower_as_ref(&mut self, expr: &HirExpr) -> Value {
        match &expr.kind {
            HirExprKind::AddrOf(inner) => self.lower_addr_of(inner),
            HirExprKind::Var(id) if matches!(&expr.ty, HirType::Ref(_)) => {
                self.read_local(*id, &expr.ty)
            }
            _ => self.lower_expr(expr),
        }
    }

    /// Lower a call, passing a hidden destination for compound return values.
    /// Returns the call result for scalar returns, or the destination for compounds.
    fn emit_call(&mut self, name: &str, args: &[HirExpr], dest: Option<Value>) -> Value {
        let param_tys: Vec<HirType> = self
            .module
            .functions
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.params.iter().map(|p| p.ty.clone()).collect())
            .unwrap_or_default();

        let mut arg_vals = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            let is_ref_param = param_tys
                .get(i)
                .map(|ty| matches!(ty, HirType::Ref(_)))
                .unwrap_or(false);
            if is_ref_param {
                arg_vals.push(self.lower_as_ref(arg));
            } else if self.is_compound(&arg.ty) {
                // By-value compound argument. A compound call result needs a temp
                // so its value survives until the callee copies it in.
                if let HirExprKind::Call {
                    name: inner_name,
                    args: inner_args,
                } = &arg.kind
                {
                    let size = self.type_size(&arg.ty);
                    let tmp = self.alloc_temp(size);
                    self.emit_call(inner_name, inner_args, Some(tmp));
                    arg_vals.push(tmp);
                } else {
                    arg_vals.push(self.lower_expr(arg));
                }
            } else {
                arg_vals.push(self.lower_expr(arg));
            }
        }

        if let Some(d) = dest {
            arg_vals.push(d);
        }

        let call_val = self.emit(LirInst::Call {
            name: name.to_string(),
            args: arg_vals,
        });
        dest.unwrap_or(call_val)
    }

    /// Lower a compound expression into an existing destination address.
    /// Direct calls forward the destination so the callee writes in place.
    fn lower_compound_value_into(&mut self, expr: &HirExpr, dst: Value, size: u16) {
        if let HirExprKind::Call { name, args } = &expr.kind {
            if self.returns_compound(name) {
                self.emit_call(name, args, Some(dst));
                return;
            }
        }
        let src = self.lower_expr(expr);
        self.emit_compound_copy(dst, src, size);
    }

    /// Copy a compound value word-by-word from src address to dst address.
    fn emit_compound_copy(&mut self, dst: Value, src: Value, size: u16) {
        let mut offset: u16 = 0;
        while offset + 1 < size {
            let off = self.emit(LirInst::Const(offset as i16));
            let sa = self.emit(LirInst::BinOp {
                op: BinOp::Add,
                lhs: src,
                rhs: off,
                is_fixed: false,
            });
            let w = self.emit(LirInst::Load { addr: sa, size: 2 });
            let da = self.emit(LirInst::BinOp {
                op: BinOp::Add,
                lhs: dst,
                rhs: off,
                is_fixed: false,
            });
            self.emit(LirInst::Store {
                addr: da,
                val: w,
                size: 2,
            });
            offset += 2;
        }
        if offset < size {
            let off = self.emit(LirInst::Const(offset as i16));
            let sa = self.emit(LirInst::BinOp {
                op: BinOp::Add,
                lhs: src,
                rhs: off,
                is_fixed: false,
            });
            let w = self.emit(LirInst::Load { addr: sa, size: 1 });
            let da = self.emit(LirInst::BinOp {
                op: BinOp::Add,
                lhs: dst,
                rhs: off,
                is_fixed: false,
            });
            self.emit(LirInst::Store {
                addr: da,
                val: w,
                size: 1,
            });
        }
    }

    /// Get the memory address of an expression (for lvalues and compound access)
    fn lower_addr_of(&mut self, expr: &HirExpr) -> Value {
        match &expr.kind {
            HirExprKind::Global(name) => {
                let addr = self.find_global_addr(name);
                self.emit(LirInst::GlobalAddr(addr))
            }
            HirExprKind::Var(id) => self.addr_of_local(*id, &expr.ty),
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
        let elem_size = self.type_size(elem_ty);
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
            HirStmt::VarDecl {
                id,
                name: _,
                ty,
                value,
            } => {
                if let HirType::Ref(_) = ty {
                    // Ref declaration: store the address as an SSA value
                    let val = if let Some(v) = value {
                        self.lower_as_ref(v)
                    } else {
                        self.emit(LirInst::Const(0))
                    };
                    self.write_local(*id, ty, val);
                } else if self.is_scalar(ty) {
                    let val = if let Some(v) = value {
                        self.lower_expr(v)
                    } else {
                        match ty {
                            HirType::Bool => self.emit(LirInst::ConstBool(false)),
                            _ => self.emit(LirInst::Const(0)),
                        }
                    };
                    self.write_local(*id, ty, val);
                } else if let Some(v) = value {
                    // Compound types: copy into the local's frame space
                    self.write_compound_local(*id, ty, v);
                }
            }

            HirStmt::Assign { target, value } => {
                match &target.kind {
                    // Ref variable assignment
                    HirExprKind::Var(id) if matches!(&target.ty, HirType::Ref(..)) => {
                        let inner = match &target.ty {
                            HirType::Ref(inner) => inner.as_ref(),
                            _ => unreachable!(),
                        };
                        if matches!(&value.kind, HirExprKind::AddrOf(_)) {
                            // Rebind: explicit `ref` expression changes what ref points to
                            let new_addr = self.lower_as_ref(value);
                            self.write_local(*id, &target.ty, new_addr);
                        } else {
                            // Write-through: store value at the address the ref holds
                            let addr = self.read_local(*id, &target.ty);
                            if self.is_scalar(inner) {
                                let val = self.lower_expr(value);
                                let size = self.type_size(inner) as u8;
                                self.emit(LirInst::Store { addr, val, size });
                            } else {
                                let size = self.type_size(inner);
                                self.lower_compound_value_into(value, addr, size);
                            }
                        }
                    }
                    // Scalar local
                    HirExprKind::Var(id) if self.is_scalar(&target.ty) => {
                        let val = self.lower_expr(value);
                        self.write_local(*id, &target.ty, val);
                    }
                    // Compound local: copy into the local's frame space
                    HirExprKind::Var(id) if !self.is_scalar(&target.ty) => {
                        self.write_compound_local(*id, &target.ty, value);
                    }
                    // Global or compound field
                    _ => {
                        let dst = self.lower_addr_of(target);
                        let size = self.type_size(&target.ty);
                        if self.is_scalar(&target.ty) {
                            let val = self.lower_expr(value);
                            self.emit(LirInst::Store {
                                addr: dst,
                                val,
                                size: size as u8,
                            });
                        } else {
                            self.lower_compound_value_into(value, dst, size);
                        }
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
                var_name: _,
                start,
                end,
                inclusive,
                body,
            } => {
                self.lower_for_range(*var, start, end, *inclusive, body);
            }

            HirStmt::ForIn {
                index,
                index_name: _,
                elem,
                elem_name: _,
                elem_ty,
                elem_is_ref,
                iter,
                body,
            } => {
                self.lower_for_in(*index, *elem, elem_ty, *elem_is_ref, iter, body);
            }

            HirStmt::Return(expr) => {
                let ret_ty = self.ret_ty.clone();
                if self.is_compound(&ret_ty) {
                    // Compound returns copy into the caller-provided destination
                    let dst = self
                        .hidden_dest
                        .expect("compound function without hidden destination");
                    if let Some(e) = expr {
                        let size = self.type_size(&ret_ty);
                        self.lower_compound_value_into(e, dst, size);
                    }
                    self.finish_block(Terminator::Return(None));
                } else {
                    let val = expr.as_ref().map(|e| self.lower_expr(e));
                    self.finish_block(Terminator::Return(val));
                }
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
        var: LocalId,
        start: &HirExpr,
        end: &HirExpr,
        inclusive: bool,
        body: &[HirStmt],
    ) {
        let int_ty = HirType::Int;

        // Init counter
        let start_val = self.lower_expr(start);
        let end_val = self.lower_expr(end);
        self.write_local(var, &int_ty, start_val);

        let header = self.fresh_block();
        let body_block = self.fresh_block();
        let exit = self.fresh_block();

        let cur = self.current_block;
        self.add_predecessor(header, cur);
        self.finish_block(Terminator::Jump(header));

        // Header: check condition
        self.start_block(header);
        let counter = self.read_local(var, &int_ty);
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
        let cur_counter = self.read_local(var, &int_ty);
        let one = self.emit(LirInst::Const(1));
        let next = self.emit(LirInst::BinOp {
            op: BinOp::Add,
            lhs: cur_counter,
            rhs: one,
            is_fixed: false,
        });
        self.write_local(var, &int_ty, next);
        let block = self.current_block;
        self.add_predecessor(header, block);
        self.finish_block(Terminator::Jump(header));

        self.seal_block(header);
        self.sealed.insert(exit);
        self.start_block(exit);
    }

    fn lower_for_in(
        &mut self,
        index: Option<LocalId>,
        elem: Option<LocalId>,
        elem_ty: &HirType,
        elem_is_ref: bool,
        iter: &HirExpr,
        body: &[HirStmt],
    ) {
        let int_ty = HirType::Int;
        let count = match deref_type(&iter.ty) {
            HirType::Array(_, count) => *count,
            _ => return,
        };

        let index_key = match index {
            Some(id) => LocalKey::Local(id),
            None => self.fresh_temp_key(),
        };

        // Init index = 0
        let zero = self.emit(LirInst::Const(0));
        let count_val = self.emit(LirInst::Const(count));
        self.write_binding(index_key, &int_ty, zero);

        let header = self.fresh_block();
        let body_block = self.fresh_block();
        let exit = self.fresh_block();

        let cur = self.current_block;
        self.add_predecessor(header, cur);
        self.finish_block(Terminator::Jump(header));

        // Header: index < count
        self.start_block(header);
        let idx = self.read_binding(index_key, &int_ty);
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
        if let Some(elem_id) = elem {
            let cur_idx = self.read_binding(index_key, &int_ty);
            let base_addr = self.lower_addr_of(iter);
            let elem_size = self.type_size(elem_ty);
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
            if elem_is_ref {
                let bind_ty = HirType::Ref(Box::new(elem_ty.clone()));
                self.write_local(elem_id, &bind_ty, elem_addr);
            } else if self.is_scalar(elem_ty) {
                let size = self.type_size(elem_ty) as u8;
                let elem_val = self.emit(LirInst::Load {
                    addr: elem_addr,
                    size,
                });
                self.write_local(elem_id, elem_ty, elem_val);
            } else {
                self.write_local(elem_id, elem_ty, elem_addr);
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
        let cur_idx = self.read_binding(index_key, &int_ty);
        let one = self.emit(LirInst::Const(1));
        let next_idx = self.emit(LirInst::BinOp {
            op: BinOp::Add,
            lhs: cur_idx,
            rhs: one,
            is_fixed: false,
        });
        self.write_binding(index_key, &int_ty, next_idx);
        let block = self.current_block;
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

fn alloc_frame_locals(ctx: &mut LowerCtx, func: &HirFunc) {
    for local in &func.locals {
        if !ctx.is_scalar(&local.ty) || local.address_taken {
            let size = ctx.type_size(&local.ty);
            let addr = ctx.alloc_byte(size);
            ctx.frame_local_addrs.insert(local.id, addr);
        }
    }
}

fn lower_function(module: &HirModule, func: &HirFunc) -> LirFunc {
    let mut ctx = LowerCtx::new(module);
    ctx.ret_ty = func.ret_type.clone();

    // Allocate frame slots for compound locals and address-taken scalars
    alloc_frame_locals(&mut ctx, func);

    // Params get the first N values, so allocate every param value before
    // emitting any prologue instructions.
    let mut params: Vec<Value> = (0..func.params.len()).map(|_| ctx.fresh_val()).collect();

    // Compound-returning functions receive a hidden destination right after
    // the source params so that OP_CALL copies it to the next value slot.
    if ctx.is_compound(&func.ret_type) {
        let v = ctx.fresh_val();
        ctx.hidden_dest = Some(v);
        params.push(v);
    }

    // write_local also materializes address-taken scalars and copies compound
    // values into their frame slots.
    for (p, &v) in func.params.iter().zip(params.iter()) {
        ctx.write_local(p.id, &p.ty, v);
    }

    // If this is main(), inline global initializers first
    if func.name == "main" {
        // Global inits with explicit init values
        for g in &module.globals {
            if let Some(init) = &g.init {
                let val = ctx.lower_expr(init);
                let addr = ctx.emit(LirInst::GlobalAddr(g.address));
                let size = ctx.type_size(&g.ty) as u8;
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

    let returns_value = ctx.is_scalar(&func.ret_type);
    let frame_bytes = ctx.next_byte_offset;

    LirFunc {
        name: func.name.clone(),
        params,
        blocks: ctx.blocks,
        returns_value,
        frame_bytes,
    }
}

pub fn lower_to_lir(module: &HirModule) -> LirModule {
    let globals_size = module
        .globals
        .iter()
        .map(|g| g.address + type_size(&g.ty, &module.structs))
        .max()
        .unwrap_or(0);

    let functions = module
        .functions
        .iter()
        .map(|func| lower_function(module, func))
        .collect();

    LirModule {
        functions,
        string_pool: module.string_pool.clone(),
        globals_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::hir::Intrinsic;
    use crate::compiler::lir::*;
    use crate::compiler::test_helpers::find_func;
    use crate::compiler::{lower, parse};

    fn lir(src: &str) -> LirModule {
        let ast = parse(src).unwrap();
        let hir = lower(&ast).unwrap();
        lower_to_lir(&hir)
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
    fn compound_local_uses_frame_offset() {
        let m = lir(
            "struct P\n  x: int\n  y: int\nend\nfn f()\n  p: P\n  p.x = 1\nend\nfn g()\n  q: array[4] of int\n  q[0] = 1\nend\nfn main()\n  f()\nend",
        );
        let f = find_func(&m, "f");
        let g = find_func(&m, "g");
        // Each function gets its own frame bytes, no cumulative static layout
        assert_eq!(f.frame_bytes, 4);
        assert_eq!(g.frame_bytes, 8);
        // Compound locals are addressed relative to the frame
        let has_frame_addr = f.blocks.iter().any(|b| {
            b.insts
                .iter()
                .any(|(_, inst)| matches!(inst, LirInst::FrameAddr(_)))
        });
        assert!(has_frame_addr);
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
    fn for_in_ref_array() {
        // Iterating a ref array must emit the loop, not silently skip it
        let m = lir("fn f(a: ref array[3] of int)\n  for _, e in a\n    cls(e)\n  end\nend");
        let f = find_func(&m, "f");
        let has_intrinsic = f.blocks.iter().any(|b| {
            b.insts
                .iter()
                .any(|(_, inst)| matches!(inst, LirInst::Intrinsic { .. }))
        });
        assert!(has_intrinsic);
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
