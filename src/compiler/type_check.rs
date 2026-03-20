use std::collections::{HashMap, HashSet};

use crate::compiler::ast::{self, BinOp, Expr, ExprKind, Span, TypeExpr, UnaryOp};
use crate::compiler::error::CompileError;
use crate::compiler::hir::*;
use crate::config::SPRITE_REGION_START;

struct IntrinsicSig {
    op: Intrinsic,
    params: Vec<HirType>,
    ret: HirType,
}

struct TypeCheckCtx {
    structs: HashMap<String, HirStruct>,
    globals: HashMap<String, HirType>,
    global_addresses: HashMap<String, u16>,
    global_inits: HashMap<String, HirExpr>,
    functions: HashMap<String, (Vec<HirType>, HirType)>,
    intrinsics: HashMap<String, IntrinsicSig>,
    scopes: Vec<HashMap<String, HirType>>,
    string_pool: Vec<String>,
    next_global_addr: u16,
    errors: Vec<CompileError>,
    promoted_locals: HashSet<String>,
}

impl TypeCheckCtx {
    fn new() -> Self {
        let mut ctx = Self {
            structs: HashMap::new(),
            globals: HashMap::new(),
            global_addresses: HashMap::new(),
            global_inits: HashMap::new(),
            functions: HashMap::new(),
            intrinsics: HashMap::new(),
            scopes: Vec::new(),
            string_pool: Vec::new(),
            next_global_addr: 0,
            errors: Vec::new(),
            promoted_locals: HashSet::new(),
        };
        ctx.register_intrinsics();
        ctx
    }

    fn register_intrinsics(&mut self) {
        use HirType::*;
        use Intrinsic::*;
        let sigs: Vec<(&str, Intrinsic, Vec<HirType>, HirType)> = vec![
            ("pset", Pset, vec![Int, Int, Int], Void),
            ("pget", Pget, vec![Int, Int], Int),
            ("cls", Cls, vec![Int], Void),
            ("line", Line, vec![Int, Int, Int, Int, Int], Void),
            ("rect", Rect, vec![Int, Int, Int, Int, Int], Void),
            ("circ", Circ, vec![Int, Int, Int, Int], Void),
            ("spr", Spr, vec![Int, Int, Int], Void),
            ("prints", Prints, vec![Str, Int, Int, Int], Void),
            ("printi", Printi, vec![Int, Int, Int, Int], Void),
            ("btn", Btn, vec![Int], Bool),
            ("btnp", Btnp, vec![Int], Bool),
            ("sfx", Sfx, vec![Int], Void),
            ("music", Music, vec![Int], Void),
            ("peek", Peek, vec![Int], Int),
            ("poke", Poke, vec![Int, Int], Void),
            ("sin", Sin, vec![Fixed], Fixed),
            ("cos", Cos, vec![Fixed], Fixed),
            ("sqrt", Sqrt, vec![Fixed], Fixed),
            ("abs", Abs, vec![Int], Int),
            ("min", Min, vec![Int, Int], Int),
            ("max", Max, vec![Int, Int], Int),
            ("flip", Flip, vec![], Void),
            ("tracei", Tracei, vec![Int], Void),
            ("traces", Traces, vec![Str], Void),
            ("time", Time, vec![], Int),
            ("rnd", Rnd, vec![Int], Int),
            ("exp", Exp, vec![Fixed], Fixed),
            ("log", Log, vec![Fixed], Fixed),
            ("pow", Pow, vec![Fixed, Fixed], Fixed),
            ("atan2", Atan2, vec![Fixed, Fixed], Fixed),
            ("ftoi", Ftoi, vec![Fixed], Int),
            ("itof", Itof, vec![Int], Fixed),
            ("tracef", Tracef, vec![Fixed], Void),
            ("printf", Printf, vec![Fixed, Int, Int, Int], Void),
        ];
        for (name, op, params, ret) in sigs {
            self.intrinsics
                .insert(name.to_string(), IntrinsicSig { op, params, ret });
        }
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(CompileError::new(msg));
    }

    fn error_at(&mut self, span: Span, msg: impl Into<String>) {
        self.errors.push(CompileError::with_span(msg, span));
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define_local(&mut self, name: &str, ty: HirType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn lookup_var(&self, name: &str) -> Option<(HirType, bool)> {
        // Search local scopes (innermost first)
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some((ty.clone(), false));
            }
        }
        // Search globals
        if let Some(ty) = self.globals.get(name) {
            return Some((ty.clone(), true));
        }
        None
    }

    fn resolve_type(&mut self, ty: &TypeExpr) -> HirType {
        match ty {
            TypeExpr::Int => HirType::Int,
            TypeExpr::Fixed => HirType::Fixed,
            TypeExpr::Bool => HirType::Bool,
            TypeExpr::Str => HirType::Str,
            TypeExpr::Void => HirType::Void,
            TypeExpr::Array(elem, size) => {
                let elem_ty = self.resolve_type(elem);
                HirType::Array(Box::new(elem_ty), *size)
            }
            TypeExpr::Ref(inner) => {
                let inner_ty = self.resolve_type(inner);
                if matches!(&inner_ty, HirType::Ref(_)) {
                    self.error("nested ref types are not allowed".to_string());
                }
                HirType::Ref(Box::new(inner_ty))
            }
            TypeExpr::Named(name) => {
                if self.structs.contains_key(name) {
                    HirType::Struct(name.clone())
                } else {
                    self.error(format!("unknown type: {name}"));
                    HirType::Void
                }
            }
        }
    }

    fn type_size(&self, ty: &HirType) -> u16 {
        match ty {
            HirType::Int | HirType::Fixed | HirType::Str | HirType::Ref(_) => 2,
            HirType::Bool => 1,
            HirType::Void => 0,
            HirType::Array(elem, count) => self.type_size(elem) * (*count as u16),
            HirType::Struct(name) => self.structs.get(name).map(|s| s.size).unwrap_or(0),
        }
    }

    fn intern_string(&mut self, s: &str) -> u16 {
        if let Some(idx) = self.string_pool.iter().position(|x| x == s) {
            idx as u16
        } else {
            let idx = self.string_pool.len() as u16;
            self.string_pool.push(s.to_string());
            idx
        }
    }

    fn alloc_global(&mut self, ty: &HirType) -> u16 {
        let addr = self.next_global_addr;
        self.next_global_addr += self.type_size(ty);
        if self.next_global_addr as usize > SPRITE_REGION_START {
            self.error(format!(
                "global variables exceed memory limit (0x{:04X} > 0x{:04X})",
                self.next_global_addr, SPRITE_REGION_START
            ));
        }
        addr
    }

    fn deref_type(ty: &HirType) -> &HirType {
        match ty {
            HirType::Ref(inner) => inner.as_ref(),
            other => other,
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> HirExpr {
        let span = expr.span;
        match &expr.kind {
            ExprKind::IntLit(n) => HirExpr {
                kind: HirExprKind::IntLit(*n),
                ty: HirType::Int,
            },
            ExprKind::FixedLit(s) => {
                let val = parse_fixed(s);
                HirExpr {
                    kind: HirExprKind::FixedLit(val),
                    ty: HirType::Fixed,
                }
            }
            ExprKind::BoolLit(b) => HirExpr {
                kind: HirExprKind::BoolLit(*b),
                ty: HirType::Bool,
            },
            ExprKind::StrLit(s) => {
                let idx = self.intern_string(s);
                HirExpr {
                    kind: HirExprKind::StrLit(idx),
                    ty: HirType::Str,
                }
            }
            ExprKind::Ident(name) => {
                if let Some((ty, is_global)) = self.lookup_var(name) {
                    HirExpr {
                        kind: if is_global {
                            HirExprKind::Global(name.clone())
                        } else {
                            HirExprKind::Var(name.clone())
                        },
                        ty,
                    }
                } else {
                    self.error_at(span, format!("undeclared variable: {name}"));
                    HirExpr {
                        kind: HirExprKind::Var(name.clone()),
                        ty: HirType::Void,
                    }
                }
            }
            ExprKind::BinOp { op, lhs, rhs } => self.check_binop(*op, lhs, rhs, span),
            ExprKind::UnaryOp { op, expr: e } => self.check_unaryop(*op, e, span),
            ExprKind::Call { func, args } => self.check_call(func, args, span),
            ExprKind::Index { expr: e, index } => self.check_index(e, index, span),
            ExprKind::FieldAccess { expr: e, field } => self.check_field_access(e, field, span),
            ExprKind::Ref(inner) => {
                let hir_inner = self.check_expr(inner);
                if !matches!(
                    &inner.kind,
                    ExprKind::Ident(_) | ExprKind::FieldAccess { .. } | ExprKind::Index { .. }
                ) {
                    self.error_at(span, "ref requires an lvalue (variable, field, or index)");
                }
                let result_ty = if matches!(&hir_inner.ty, HirType::Ref(_)) {
                    // Forwarding: ref of ref var keeps the same ref type (no nesting)
                    hir_inner.ty.clone()
                } else {
                    // Mark scalar local for promotion to memory
                    if let ExprKind::Ident(name) = &inner.kind {
                        if let Some((ref ty, false)) = self.lookup_var(name) {
                            if matches!(
                                ty,
                                HirType::Int | HirType::Fixed | HirType::Bool | HirType::Str
                            ) {
                                self.promoted_locals.insert(name.clone());
                            }
                        }
                    }
                    HirType::Ref(Box::new(hir_inner.ty.clone()))
                };
                HirExpr {
                    kind: HirExprKind::AddrOf(Box::new(hir_inner)),
                    ty: result_ty,
                }
            }
        }
    }

    fn check_binop(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr, span: Span) -> HirExpr {
        let lhs_hir = self.check_expr(lhs);
        let rhs_hir = self.check_expr(rhs);
        let lhs_ty = Self::deref_type(&lhs_hir.ty);
        let rhs_ty = Self::deref_type(&rhs_hir.ty);

        let is_comparison = matches!(
            op,
            BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Leq | BinOp::Geq
        );
        let is_logical = matches!(op, BinOp::And | BinOp::Or);

        if is_logical {
            if *lhs_ty != HirType::Bool {
                self.error_at(span, format!("expected bool, got {}", lhs_hir.ty));
            }
            if *rhs_ty != HirType::Bool {
                self.error_at(span, format!("expected bool, got {}", rhs_hir.ty));
            }
            HirExpr {
                kind: HirExprKind::BinOp {
                    op,
                    lhs: Box::new(lhs_hir),
                    rhs: Box::new(rhs_hir),
                },
                ty: HirType::Bool,
            }
        } else {
            if lhs_ty != rhs_ty {
                self.error_at(
                    span,
                    format!("type mismatch: {} vs {}", lhs_hir.ty, rhs_hir.ty),
                );
            }
            let is_arithmetic = !is_comparison;
            if is_arithmetic && *lhs_ty != HirType::Int && *lhs_ty != HirType::Fixed {
                self.error_at(span, format!("arithmetic on {}", lhs_hir.ty));
            }
            let result_ty = if is_comparison {
                HirType::Bool
            } else {
                lhs_ty.clone()
            };
            HirExpr {
                kind: HirExprKind::BinOp {
                    op,
                    lhs: Box::new(lhs_hir),
                    rhs: Box::new(rhs_hir),
                },
                ty: result_ty,
            }
        }
    }

    fn check_unaryop(&mut self, op: UnaryOp, expr: &Expr, span: Span) -> HirExpr {
        let inner = self.check_expr(expr);
        let inner_ty = Self::deref_type(&inner.ty);
        match op {
            UnaryOp::Neg => {
                if *inner_ty != HirType::Int && *inner_ty != HirType::Fixed {
                    self.error_at(span, format!("cannot negate {}", inner.ty));
                }
                let ty = inner_ty.clone();
                HirExpr {
                    kind: HirExprKind::UnaryOp {
                        op,
                        expr: Box::new(inner),
                    },
                    ty,
                }
            }
            UnaryOp::Not => {
                if *inner_ty != HirType::Bool {
                    self.error_at(span, format!("'not' requires bool, got {}", inner.ty));
                }
                HirExpr {
                    kind: HirExprKind::UnaryOp {
                        op,
                        expr: Box::new(inner),
                    },
                    ty: HirType::Bool,
                }
            }
        }
    }

    fn check_call(&mut self, func: &Expr, args: &[Expr], span: Span) -> HirExpr {
        // Check if it's an intrinsic or user function
        if let ExprKind::Ident(name) = &func.kind {
            // Try intrinsic first
            if let Some(sig) = self.intrinsics.get(name) {
                let op = sig.op;
                let expected_params = sig.params.clone();
                let ret = sig.ret.clone();
                if args.len() != expected_params.len() {
                    self.error_at(
                        span,
                        format!(
                            "{name}: expected {} args, got {}",
                            expected_params.len(),
                            args.len()
                        ),
                    );
                }
                let mut hir_args = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let hir_arg = self.check_expr(arg);
                    if let Some(expected) = expected_params.get(i) {
                        if hir_arg.ty != *expected {
                            self.error_at(
                                arg.span,
                                format!(
                                    "{name} arg {}: expected {}, got {}",
                                    i + 1,
                                    expected,
                                    hir_arg.ty
                                ),
                            );
                        }
                    }
                    hir_args.push(hir_arg);
                }
                return HirExpr {
                    kind: HirExprKind::Intrinsic { op, args: hir_args },
                    ty: ret,
                };
            }

            // Try user function
            if let Some((param_types, ret_type)) = self.functions.get(name).cloned() {
                if args.len() != param_types.len() {
                    self.error_at(
                        span,
                        format!(
                            "{name}: expected {} args, got {}",
                            param_types.len(),
                            args.len()
                        ),
                    );
                }
                let mut hir_args = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let hir_arg = self.check_expr(arg);
                    if let Some(expected) = param_types.get(i) {
                        if matches!(expected, HirType::Ref(_)) {
                            // Ref param: arg must be Ref(T) (explicit ref or forwarding)
                            if hir_arg.ty != *expected {
                                self.error_at(
                                    arg.span,
                                    format!(
                                        "{name} arg {}: expected {}, got {} (use 'ref' at call site)",
                                        i + 1,
                                        expected,
                                        hir_arg.ty
                                    ),
                                );
                            }
                        } else {
                            // Non-ref param: accept exact match or auto-deref
                            let arg_deref = Self::deref_type(&hir_arg.ty);
                            if hir_arg.ty != *expected && arg_deref != expected {
                                self.error_at(
                                    arg.span,
                                    format!(
                                        "{name} arg {}: expected {}, got {}",
                                        i + 1,
                                        expected,
                                        hir_arg.ty
                                    ),
                                );
                            }
                        }
                    }
                    hir_args.push(hir_arg);
                }
                return HirExpr {
                    kind: HirExprKind::Call {
                        name: name.clone(),
                        args: hir_args,
                    },
                    ty: ret_type,
                };
            }

            self.error_at(span, format!("unknown function: {name}"));
            let hir_args: Vec<HirExpr> = args.iter().map(|a| self.check_expr(a)).collect();
            return HirExpr {
                kind: HirExprKind::Call {
                    name: name.clone(),
                    args: hir_args,
                },
                ty: HirType::Void,
            };
        }

        // Non-ident call (e.g. expr(), not supported but handle gracefully)
        self.error_at(span, "only named function calls are supported".to_string());
        let hir_args: Vec<HirExpr> = args.iter().map(|a| self.check_expr(a)).collect();
        HirExpr {
            kind: HirExprKind::Call {
                name: "<unknown>".to_string(),
                args: hir_args,
            },
            ty: HirType::Void,
        }
    }

    fn check_index(&mut self, expr: &Expr, index: &Expr, span: Span) -> HirExpr {
        let base = self.check_expr(expr);
        let idx = self.check_expr(index);
        let idx_ty = Self::deref_type(&idx.ty);
        if *idx_ty != HirType::Int {
            self.error_at(span, format!("index must be int, got {}", idx.ty));
        }
        let base_ty = Self::deref_type(&base.ty);
        let elem_ty = match base_ty {
            HirType::Array(elem, _) => *elem.clone(),
            other => {
                self.error_at(span, format!("cannot index into {}", other));
                HirType::Void
            }
        };
        HirExpr {
            kind: HirExprKind::Index {
                expr: Box::new(base),
                index: Box::new(idx),
            },
            ty: elem_ty,
        }
    }

    fn check_field_access(&mut self, expr: &Expr, field: &str, span: Span) -> HirExpr {
        let base = self.check_expr(expr);
        let base_ty = Self::deref_type(&base.ty);
        let struct_name = match base_ty {
            HirType::Struct(name) => name.clone(),
            other => {
                self.error_at(span, format!("cannot access field on {}", other));
                return HirExpr {
                    kind: HirExprKind::FieldAccess {
                        expr: Box::new(base),
                        field: field.to_string(),
                        offset: 0,
                    },
                    ty: HirType::Void,
                };
            }
        };
        if let Some(s) = self.structs.get(&struct_name) {
            if let Some(f) = s.fields.iter().find(|f| f.name == field) {
                let ty = f.ty.clone();
                let offset = f.offset;
                return HirExpr {
                    kind: HirExprKind::FieldAccess {
                        expr: Box::new(base),
                        field: field.to_string(),
                        offset,
                    },
                    ty,
                };
            }
            self.error_at(span, format!("struct {struct_name} has no field '{field}'"));
        }
        HirExpr {
            kind: HirExprKind::FieldAccess {
                expr: Box::new(base),
                field: field.to_string(),
                offset: 0,
            },
            ty: HirType::Void,
        }
    }

    fn check_stmt(&mut self, stmt: &ast::Stmt) -> HirStmt {
        let span = stmt.span;
        match &stmt.kind {
            ast::StmtKind::VarDecl { name, ty, value } => {
                let hir_ty = self.resolve_type(ty);
                let hir_value = value.as_ref().map(|v| {
                    let hv = self.check_expr(v);
                    // Allow auto-deref: Ref(T) value for T declared type
                    if hv.ty != hir_ty && *Self::deref_type(&hv.ty) != hir_ty {
                        self.error_at(span, format!("{name}: expected {}, got {}", hir_ty, hv.ty));
                    }
                    hv
                });
                self.define_local(name, hir_ty.clone());
                HirStmt::VarDecl {
                    name: name.clone(),
                    ty: hir_ty,
                    value: hir_value,
                }
            }
            ast::StmtKind::Assign { target, value } => {
                // Infer-declare: `y = 10` inside a function defines a new local
                if let ExprKind::Ident(name) = &target.kind {
                    if self.lookup_var(name).is_none() {
                        let hir_value = self.check_expr(value);
                        // Auto-deref for inference unless explicit ref
                        let ty = if matches!(&hir_value.kind, HirExprKind::AddrOf(_)) {
                            hir_value.ty.clone()
                        } else {
                            Self::deref_type(&hir_value.ty).clone()
                        };
                        self.define_local(name, ty.clone());
                        return HirStmt::VarDecl {
                            name: name.clone(),
                            ty,
                            value: Some(hir_value),
                        };
                    }
                }
                let hir_target = self.check_expr(target);
                let hir_value = self.check_expr(value);
                if let HirType::Ref(inner) = &hir_target.ty {
                    // Ref target: accept Ref(T) (rebind) or T (write-through)
                    if hir_value.ty != hir_target.ty && hir_value.ty != **inner {
                        self.error_at(
                            span,
                            format!("assign: {} vs {}", hir_target.ty, hir_value.ty),
                        );
                    }
                } else {
                    // Non-ref target: accept exact match or auto-deref
                    if hir_target.ty != hir_value.ty
                        && hir_target.ty != *Self::deref_type(&hir_value.ty)
                    {
                        self.error_at(
                            span,
                            format!("assign: {} vs {}", hir_target.ty, hir_value.ty),
                        );
                    }
                }
                HirStmt::Assign {
                    target: hir_target,
                    value: hir_value,
                }
            }
            ast::StmtKind::If {
                cond,
                body,
                else_ifs,
                else_body,
            } => {
                let hir_cond = self.check_expr(cond);
                if hir_cond.ty != HirType::Bool {
                    self.error_at(cond.span, format!("if: expected bool, got {}", hir_cond.ty));
                }
                self.push_scope();
                let hir_body = self.check_stmts(body);
                self.pop_scope();
                let hir_else_ifs: Vec<(HirExpr, Vec<HirStmt>)> = else_ifs
                    .iter()
                    .map(|(c, b)| {
                        let hc = self.check_expr(c);
                        if hc.ty != HirType::Bool {
                            self.error_at(c.span, format!("else if: expected bool, got {}", hc.ty));
                        }
                        self.push_scope();
                        let hb = self.check_stmts(b);
                        self.pop_scope();
                        (hc, hb)
                    })
                    .collect();
                self.push_scope();
                let hir_else = self.check_stmts(else_body);
                self.pop_scope();
                HirStmt::If {
                    cond: hir_cond,
                    body: hir_body,
                    else_ifs: hir_else_ifs,
                    else_body: hir_else,
                }
            }
            ast::StmtKind::While { cond, body } => {
                let hir_cond = self.check_expr(cond);
                if hir_cond.ty != HirType::Bool {
                    self.error_at(
                        cond.span,
                        format!("while: expected bool, got {}", hir_cond.ty),
                    );
                }
                self.push_scope();
                let hir_body = self.check_stmts(body);
                self.pop_scope();
                HirStmt::While {
                    cond: hir_cond,
                    body: hir_body,
                }
            }
            ast::StmtKind::ForIn {
                index,
                elem,
                iter,
                body,
            } => {
                let hir_iter = self.check_expr(iter);
                let elem_ty = match &hir_iter.ty {
                    HirType::Array(elem, _) => *elem.clone(),
                    other => {
                        self.error_at(iter.span, format!("for-in: expected array, got {}", other));
                        HirType::Void
                    }
                };
                self.push_scope();
                if index != "_" {
                    self.define_local(index, HirType::Int);
                }
                if elem != "_" {
                    self.define_local(elem, elem_ty);
                }
                let hir_body = self.check_stmts(body);
                self.pop_scope();
                HirStmt::ForIn {
                    index: index.clone(),
                    elem: elem.clone(),
                    iter: hir_iter,
                    body: hir_body,
                }
            }
            ast::StmtKind::ForRange {
                var,
                start,
                end,
                inclusive,
                body,
            } => {
                let hir_start = self.check_expr(start);
                let hir_end = self.check_expr(end);
                if hir_start.ty != HirType::Int {
                    self.error_at(
                        start.span,
                        format!("for: start must be int, got {}", hir_start.ty),
                    );
                }
                if hir_end.ty != HirType::Int {
                    self.error_at(
                        end.span,
                        format!("for: end must be int, got {}", hir_end.ty),
                    );
                }
                self.push_scope();
                self.define_local(var, HirType::Int);
                let hir_body = self.check_stmts(body);
                self.pop_scope();
                HirStmt::ForRange {
                    var: var.clone(),
                    start: hir_start,
                    end: hir_end,
                    inclusive: *inclusive,
                    body: hir_body,
                }
            }
            ast::StmtKind::Return(expr) => {
                let hir_expr = expr.as_ref().map(|e| self.check_expr(e));
                HirStmt::Return(hir_expr)
            }
            ast::StmtKind::Expression(expr) => {
                let hir_expr = self.check_expr(expr);
                HirStmt::Expression(hir_expr)
            }
            ast::StmtKind::Break => HirStmt::Break,
            ast::StmtKind::Continue => HirStmt::Continue,
        }
    }

    fn check_stmts(&mut self, stmts: &[ast::Stmt]) -> Vec<HirStmt> {
        stmts.iter().map(|s| self.check_stmt(s)).collect()
    }

    fn check_global_stmt(&mut self, stmt: &ast::Stmt) -> Option<HirStmt> {
        let span = stmt.span;
        match &stmt.kind {
            ast::StmtKind::VarDecl { name, ty, value } => {
                let hir_ty = self.resolve_type(ty);
                if matches!(&hir_ty, HirType::Ref(_)) {
                    self.error_at(span, "global variables cannot be ref");
                }
                let hir_value = value.as_ref().map(|v| {
                    let hv = self.check_expr(v);
                    if hv.ty != hir_ty {
                        self.error_at(span, format!("{name}: expected {}, got {}", hir_ty, hv.ty));
                    }
                    hv
                });
                let addr = self.alloc_global(&hir_ty);
                self.globals.insert(name.clone(), hir_ty.clone());
                self.global_addresses.insert(name.clone(), addr);
                if let Some(init) = hir_value {
                    self.global_inits.insert(name.clone(), init);
                }
                None
            }
            ast::StmtKind::Assign { target, value } => {
                // Untyped global assignment: `x = 5`
                if let ExprKind::Ident(name) = &target.kind {
                    if !self.globals.contains_key(name) {
                        // New global, infer type from RHS
                        let hir_value = self.check_expr(value);
                        let ty = hir_value.ty.clone();
                        let addr = self.alloc_global(&ty);
                        self.globals.insert(name.clone(), ty.clone());
                        self.global_addresses.insert(name.clone(), addr);
                        self.global_inits.insert(name.clone(), hir_value);
                        return None;
                    }
                }
                // Re-assignment to existing global, or complex target
                Some(self.check_stmt(stmt))
            }
            _ => Some(self.check_stmt(stmt)),
        }
    }
}

pub fn lower_to_hir(module: &ast::Module) -> Result<HirModule, Vec<CompileError>> {
    let mut ctx = TypeCheckCtx::new();

    // Pass 1: Register structs
    let mut hir_structs = Vec::new();
    for item in &module.items {
        if let ast::TopLevel::Struct(s) = item {
            let mut fields = Vec::new();
            let mut offset: u16 = 0;
            for f in &s.fields {
                let ty = ctx.resolve_type(&f.ty);
                if matches!(&ty, HirType::Ref(_)) {
                    ctx.error(format!("struct field '{}' cannot be ref", f.name));
                }
                fields.push(HirStructField {
                    name: f.name.clone(),
                    ty: ty.clone(),
                    offset,
                });
                offset += ctx.type_size(&ty);
            }
            let hir_struct = HirStruct {
                name: s.name.clone(),
                fields,
                size: offset,
            };
            ctx.structs.insert(s.name.clone(), hir_struct.clone());
            hir_structs.push(hir_struct);
        }
    }

    // Pass 2: Register function signatures
    for item in &module.items {
        if let ast::TopLevel::Function(f) = item {
            let param_types: Vec<HirType> =
                f.params.iter().map(|p| ctx.resolve_type(&p.ty)).collect();
            let ret_type = f
                .ret_type
                .as_ref()
                .map(|t| ctx.resolve_type(t))
                .unwrap_or(HirType::Void);
            ctx.functions
                .insert(f.name.clone(), (param_types, ret_type));
        }
    }

    // Pass 3: Process globals
    let mut hir_globals = Vec::new();
    let mut global_init = Vec::new();
    for item in &module.items {
        if let ast::TopLevel::Global(stmt) = item {
            if let Some(hir_stmt) = ctx.check_global_stmt(stmt) {
                global_init.push(hir_stmt);
            }
        }
    }
    // Build HirGlobal list from registered globals
    for item in &module.items {
        if let ast::TopLevel::Global(stmt) = item {
            let name = match &stmt.kind {
                ast::StmtKind::VarDecl { name, .. } => Some(name.clone()),
                ast::StmtKind::Assign { target, .. } => {
                    if let ExprKind::Ident(name) = &target.kind {
                        Some(name.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(name) = name {
                if let (Some(ty), Some(&addr)) =
                    (ctx.globals.get(&name), ctx.global_addresses.get(&name))
                {
                    // Avoid duplicates
                    if !hir_globals.iter().any(|g: &HirGlobal| g.name == name) {
                        let init = ctx.global_inits.remove(&name);
                        hir_globals.push(HirGlobal {
                            name: name.clone(),
                            ty: ty.clone(),
                            address: addr,
                            init,
                        });
                    }
                }
            }
        }
    }

    // Pass 4: Check function bodies
    let mut hir_functions = Vec::new();
    for item in &module.items {
        if let ast::TopLevel::Function(f) = item {
            let (param_types, ret_type) = ctx.functions.get(&f.name).cloned().unwrap();
            if matches!(&ret_type, HirType::Ref(_)) {
                ctx.error(format!("{}: cannot return ref", f.name));
            }
            ctx.promoted_locals.clear();
            ctx.push_scope();
            let mut params = Vec::new();
            for (i, p) in f.params.iter().enumerate() {
                let ty = param_types[i].clone();
                ctx.define_local(&p.name, ty.clone());
                params.push(HirParam {
                    name: p.name.clone(),
                    ty,
                });
            }
            let body = ctx.check_stmts(&f.body);

            // Collect locals (all VarDecls in body)
            let mut locals = Vec::new();
            collect_locals(&body, &mut locals);

            // Check return type
            check_returns(&body, &ret_type, &mut ctx);
            if ret_type != HirType::Void && !always_returns(&body) {
                ctx.error(format!("{}: not all paths return a value", f.name));
            }

            ctx.pop_scope();
            let promoted = ctx.promoted_locals.clone();
            hir_functions.push(HirFunc {
                name: f.name.clone(),
                params,
                ret_type,
                locals,
                body,
                promoted_locals: promoted,
            });
        }
    }

    if ctx.errors.is_empty() {
        Ok(HirModule {
            structs: hir_structs,
            globals: hir_globals,
            functions: hir_functions,
            global_init,
            string_pool: ctx.string_pool,
        })
    } else {
        Err(ctx.errors)
    }
}

fn collect_locals(stmts: &[HirStmt], locals: &mut Vec<(String, HirType)>) {
    for stmt in stmts {
        match stmt {
            HirStmt::VarDecl { name, ty, .. } => {
                locals.push((name.clone(), ty.clone()));
            }
            HirStmt::If {
                body,
                else_ifs,
                else_body,
                ..
            } => {
                collect_locals(body, locals);
                for (_, b) in else_ifs {
                    collect_locals(b, locals);
                }
                collect_locals(else_body, locals);
            }
            HirStmt::While { body, .. }
            | HirStmt::ForIn { body, .. }
            | HirStmt::ForRange { body, .. } => {
                collect_locals(body, locals);
            }
            _ => {}
        }
    }
}

fn check_returns(stmts: &[HirStmt], expected: &HirType, ctx: &mut TypeCheckCtx) {
    for stmt in stmts {
        match stmt {
            HirStmt::Return(Some(expr)) => {
                if *expected == HirType::Void {
                    ctx.error("void function cannot return a value".to_string());
                } else if expr.ty != *expected {
                    let deref = TypeCheckCtx::deref_type(&expr.ty);
                    if deref != expected {
                        ctx.error(format!("return: expected {}, got {}", expected, expr.ty));
                    }
                }
            }
            HirStmt::Return(None) => {
                if *expected != HirType::Void {
                    ctx.error(format!("return: expected {}, got void", expected));
                }
            }
            HirStmt::If {
                body,
                else_ifs,
                else_body,
                ..
            } => {
                check_returns(body, expected, ctx);
                for (_, b) in else_ifs {
                    check_returns(b, expected, ctx);
                }
                check_returns(else_body, expected, ctx);
            }
            HirStmt::While { body, .. }
            | HirStmt::ForIn { body, .. }
            | HirStmt::ForRange { body, .. } => {
                check_returns(body, expected, ctx);
            }
            _ => {}
        }
    }
}

fn always_returns(stmts: &[HirStmt]) -> bool {
    for stmt in stmts {
        match stmt {
            HirStmt::Return(_) => return true,
            HirStmt::If {
                body,
                else_body,
                else_ifs,
                ..
            } => {
                if else_body.is_empty() {
                    continue;
                }
                let then_returns = always_returns(body);
                let else_returns = always_returns(else_body);
                let else_ifs_return = else_ifs.iter().all(|(_, b)| always_returns(b));
                if then_returns && else_returns && else_ifs_return {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

pub fn type_size_standalone(ty: &HirType, structs: &[HirStruct]) -> u16 {
    match ty {
        HirType::Int | HirType::Fixed | HirType::Str | HirType::Ref(_) => 2,
        HirType::Bool => 1,
        HirType::Void => 0,
        HirType::Array(elem, count) => type_size_standalone(elem, structs) * (*count as u16),
        HirType::Struct(name) => structs
            .iter()
            .find(|s| s.name == *name)
            .map(|s| s.size)
            .unwrap_or(0),
    }
}

fn parse_fixed(s: &str) -> i16 {
    let val: f64 = s.parse().unwrap_or(0.0);
    (val * 128.0).round() as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::parse;

    fn lower(src: &str) -> HirModule {
        let module = parse(src).unwrap();
        lower_to_hir(&module).unwrap()
    }

    fn lower_err(src: &str) -> Vec<CompileError> {
        let module = parse(src).unwrap();
        lower_to_hir(&module).unwrap_err()
    }

    #[test]
    fn typed_decl_with_value() {
        let hir = lower("x: int = 5");
        assert_eq!(hir.globals.len(), 1);
        assert_eq!(hir.globals[0].name, "x");
        assert_eq!(hir.globals[0].ty, HirType::Int);
    }

    #[test]
    fn typed_decl_without_value() {
        let hir = lower("x: int");
        assert_eq!(hir.globals.len(), 1);
        assert_eq!(hir.globals[0].name, "x");
        assert_eq!(hir.globals[0].ty, HirType::Int);
    }

    #[test]
    fn infer_int_from_literal() {
        let hir = lower("x = 5");
        assert_eq!(hir.globals[0].ty, HirType::Int);
    }

    #[test]
    fn infer_fixed_from_literal() {
        let hir = lower("y = 1.5");
        assert_eq!(hir.globals[0].ty, HirType::Fixed);
    }

    #[test]
    fn infer_str_from_literal() {
        let hir = lower("s = \"hi\"");
        assert_eq!(hir.globals[0].ty, HirType::Str);
    }

    #[test]
    fn type_mismatch_error() {
        let errs = lower_err("x: int = true");
        assert!(errs
            .iter()
            .any(|e| e.message.contains("expected int, got bool")));
    }

    #[test]
    fn binop_type_check() {
        let hir = lower("x: int = 1 + 2");
        assert_eq!(hir.globals[0].ty, HirType::Int);
    }

    #[test]
    fn binop_mismatch() {
        let errs = lower_err("x: bool = true\ny: int = 1 + x");
        assert!(errs
            .iter()
            .any(|e| e.message.contains("type mismatch: int vs bool")));
    }

    #[test]
    fn comparison_returns_bool() {
        let hir = lower("x: bool = 1 < 2");
        assert_eq!(hir.globals[0].ty, HirType::Bool);
    }

    #[test]
    fn struct_layout() {
        let hir = lower("struct Enemy\n  x: int\n  y: int\n  alive: bool\nend");
        assert_eq!(hir.structs.len(), 1);
        let s = &hir.structs[0];
        assert_eq!(s.size, 5);
        assert_eq!(s.fields[0].offset, 0);
        assert_eq!(s.fields[1].offset, 2);
        assert_eq!(s.fields[2].offset, 4);
    }

    #[test]
    fn field_access_type() {
        let hir = lower("struct Enemy\n  x: int\n  y: int\nend\ne: Enemy\ne.x = 10");
        // The assignment e.x = 10 should be in global_init
        assert!(!hir.global_init.is_empty());
    }

    #[test]
    fn intrinsic_recognition() {
        let hir = lower("cls(0)");
        match &hir.global_init[0] {
            HirStmt::Expression(expr) => match &expr.kind {
                HirExprKind::Intrinsic { op, .. } => assert_eq!(*op, Intrinsic::Cls),
                other => panic!("expected intrinsic, got {:?}", other),
            },
            other => panic!("expected expression, got {:?}", other),
        }
    }

    #[test]
    fn intrinsic_wrong_arg_count() {
        let errs = lower_err("cls(0, 1)");
        assert!(errs.iter().any(|e| e.message.contains("expected 1 args")));
    }

    #[test]
    fn function_call_return_type() {
        let hir = lower("fn add(a: int, b: int): int\n  return a + b\nend\nx: int = add(1, 2)");
        assert_eq!(hir.functions.len(), 1);
        assert_eq!(hir.globals[0].ty, HirType::Int);
    }

    #[test]
    fn string_pool_dedup() {
        let hir = lower("x = \"hello\"\ny = \"hello\"");
        assert_eq!(hir.string_pool.len(), 1);
        assert_eq!(hir.string_pool[0], "hello");
    }

    #[test]
    fn fixed_point_conversion() {
        let hir = lower("x = 1.5");
        // 1.5 * 128 = 192
        // The global should be of Fixed type
        assert_eq!(hir.globals[0].ty, HirType::Fixed);
    }

    #[test]
    fn undeclared_variable() {
        let errs = lower_err("fn f(): int\n  return x + 1\nend");
        assert!(errs
            .iter()
            .any(|e| e.message.contains("undeclared variable")));
    }

    #[test]
    fn return_type_mismatch() {
        let errs = lower_err("fn f(): int\n  return true\nend");
        assert!(errs
            .iter()
            .any(|e| e.message.contains("return: expected int, got bool")));
    }

    #[test]
    fn global_memory_addresses() {
        let hir = lower("x: int = 1\ny: int = 2");
        assert_eq!(hir.globals[0].address, 0);
        assert_eq!(hir.globals[1].address, 2);
    }

    #[test]
    fn array_type() {
        let hir = lower("a: array[10] of int");
        assert_eq!(
            hir.globals[0].ty,
            HirType::Array(Box::new(HirType::Int), 10)
        );
    }

    #[test]
    fn for_range_var_is_int() {
        // for-range in a function to test local var typing
        let hir = lower("fn f()\n  for i in 0..10\n    cls(i)\n  end\nend");
        assert_eq!(hir.functions.len(), 1);
    }

    #[test]
    fn for_in_types() {
        let hir = lower(
            "enemies: array[10] of int\nfn f()\n  for i, e in enemies\n    cls(e)\n  end\nend",
        );
        assert_eq!(hir.functions.len(), 1);
    }

    #[test]
    fn fixed_point_value() {
        assert_eq!(parse_fixed("1.5"), 192);
        assert_eq!(parse_fixed("0.0"), 0);
        assert_eq!(parse_fixed("1.0"), 128);
    }

    #[test]
    fn infer_local_from_assign() {
        let hir = lower("fn f(): int\n  y = 10\n  return y\nend");
        let f = &hir.functions[0];
        assert_eq!(f.locals.len(), 1);
        assert_eq!(f.locals[0].0, "y");
        assert_eq!(f.locals[0].1, HirType::Int);
    }

    #[test]
    fn infer_local_used_in_if() {
        // The program from the bug report
        let hir = lower(
            "fn update(x: int): int\n  y = 10\n  if x > 0\n    y = y + x\n  else\n    y = y - x\n  end\n  return y\nend",
        );
        let f = &hir.functions[0];
        assert!(f.locals.iter().any(|(n, t)| n == "y" && *t == HirType::Int));
    }

    #[test]
    fn void_fn_return_value_error() {
        let errs = lower_err("fn f()\n  return 1\nend");
        assert!(errs
            .iter()
            .any(|e| e.message.contains("void function cannot return a value")));
    }

    #[test]
    fn void_fn_return_value_nested_error() {
        let errs = lower_err("fn f(x: bool)\n  if x\n    return 1\n  end\nend");
        assert!(errs
            .iter()
            .any(|e| e.message.contains("void function cannot return a value")));
    }

    #[test]
    fn non_void_fn_bare_return_error() {
        let errs = lower_err("fn f(): int\n  return\nend");
        assert!(errs
            .iter()
            .any(|e| e.message.contains("return: expected int, got void")));
    }

    #[test]
    fn missing_return_on_else_path() {
        let errs = lower_err("fn f(x: int): int\n  if x > 100\n    return x\n  end\n  cls(0)\nend");
        assert!(errs
            .iter()
            .any(|e| e.message.contains("not all paths return")));
    }

    #[test]
    fn all_paths_return_ok() {
        let _hir =
            lower("fn f(x: int): int\n  if x > 0\n    return x\n  else\n    return 0\n  end\nend");
    }

    // --- Ref semantics tests ---

    #[test]
    fn ref_scalar_param() {
        let hir = lower("fn f(x: ref int)\nend");
        assert_eq!(
            hir.functions[0].params[0].ty,
            HirType::Ref(Box::new(HirType::Int))
        );
    }

    #[test]
    fn ref_struct_param() {
        let hir = lower("struct V\n  x: int\nend\nfn f(v: ref V)\nend");
        assert_eq!(
            hir.functions[0].params[0].ty,
            HirType::Ref(Box::new(HirType::Struct("V".into())))
        );
    }

    #[test]
    fn ref_auto_deref_binop() {
        // ref int should auto-deref in arithmetic
        let _hir = lower("fn f(x: ref int): int\n  return x + 1\nend");
    }

    #[test]
    fn ref_auto_deref_comparison() {
        let _hir = lower("fn f(x: ref int): bool\n  return x > 0\nend");
    }

    #[test]
    fn ref_auto_deref_unary() {
        let _hir = lower("fn f(x: ref int): int\n  return -x\nend");
    }

    #[test]
    fn ref_auto_deref_call_arg() {
        // ref int auto-derefs when passed to non-ref int param
        let _hir = lower("fn g(x: int)\nend\nfn f(x: ref int)\n  g(x)\nend");
    }

    #[test]
    fn ref_forwarding() {
        // forwarding a ref param to another ref param
        let _hir = lower("fn g(x: ref int)\nend\nfn f(x: ref int)\n  g(x)\nend");
    }

    #[test]
    fn ref_explicit_at_call_site() {
        // explicit ref required for non-ref args
        let _hir = lower("fn g(x: ref int)\nend\nfn f()\n  y = 5\n  g(ref y)\nend");
    }

    #[test]
    fn ref_missing_at_call_site_error() {
        let errs = lower_err("fn g(x: ref int)\nend\nfn f()\n  y = 5\n  g(y)\nend");
        assert!(errs.iter().any(|e| e.message.contains("ref")));
    }

    #[test]
    fn ref_non_lvalue_error() {
        let errs = lower_err("fn g(x: ref int)\nend\nfn f()\n  g(ref 5)\nend");
        assert!(errs.iter().any(|e| e.message.contains("lvalue")));
    }

    #[test]
    fn ref_in_struct_field_error() {
        let errs = lower_err("struct S\n  x: ref int\nend");
        assert!(errs.iter().any(|e| e.message.contains("cannot be ref")));
    }

    #[test]
    fn ref_return_type_error() {
        let errs = lower_err("fn f(x: ref int): ref int\n  return x\nend");
        assert!(errs.iter().any(|e| e.message.contains("cannot return ref")));
    }

    #[test]
    fn ref_nested_error() {
        let errs = lower_err("fn f(x: ref ref int)\nend");
        assert!(errs.iter().any(|e| e.message.contains("nested ref")));
    }

    #[test]
    fn ref_global_error() {
        let errs = lower_err("x: ref int");
        assert!(errs.iter().any(|e| e.message.contains("global")));
    }

    #[test]
    fn ref_write_through_type_check() {
        // x = 5 where x: ref int is write-through (OK)
        let _hir = lower("fn f(x: ref int)\n  x = 5\nend");
    }

    #[test]
    fn ref_rebind_type_check() {
        // x = ref y where x: ref int (OK, rebind)
        let _hir = lower("fn f(x: ref int)\n  y = 10\n  x = ref y\nend");
    }

    #[test]
    fn ref_field_access() {
        // Field access through ref struct
        let _hir = lower(
            "struct V\n  x: int\n  y: int\nend\nfn f(v: ref V): int\n  return v.x + v.y\nend",
        );
    }

    #[test]
    fn ref_field_assign() {
        // Field assignment through ref struct
        let _hir =
            lower("struct V\n  x: int\n  y: int\nend\nfn f(v: ref V)\n  v.x = 10\n  v.y = 20\nend");
    }

    #[test]
    fn ref_local_binding() {
        // p = ref game.player style binding
        let _hir = lower("struct V\n  x: int\nend\nv: V\nfn f()\n  p = ref v\n  p.x = 10\nend");
    }

    #[test]
    fn ref_promoted_scalar() {
        // Taking ref of a local scalar promotes it to memory
        let hir = lower("fn f()\n  x = 5\n  g(ref x)\nend\nfn g(p: ref int)\nend");
        assert!(hir.functions[0].promoted_locals.contains("x"));
    }

    #[test]
    fn ref_auto_deref_index() {
        // ref int used as index auto-derefs
        let _hir = lower("a: array[10] of int\nfn f(i: ref int)\n  cls(a[i])\nend");
    }
}
