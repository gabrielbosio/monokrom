pub mod ast;
pub mod error;
pub mod hir;
pub mod hir_to_lir;
pub mod lexer;
pub mod lir;
pub mod type_check;

lalrpop_util::lalrpop_mod!(
    #[allow(clippy::all)]
    pub grammar,
    "/compiler/grammar.rs"
);

pub fn parse(
    source: &str,
) -> Result<ast::Module, lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>> {
    let lexer = lexer::LexerAdapter::new(source);
    grammar::ModuleParser::new().parse(lexer)
}

pub fn lower(module: &ast::Module) -> Result<hir::HirModule, Vec<error::CompileError>> {
    type_check::lower_to_hir(module)
}

pub fn lower_lir(hir: &hir::HirModule) -> lir::LirModule {
    hir_to_lir::lower_to_lir(hir)
}

#[cfg(test)]
mod tests {
    use super::ast::*;
    use super::*;

    fn p(source: &str) -> Module {
        parse(source).unwrap()
    }

    #[test]
    fn empty_module() {
        assert_eq!(p(""), Module { items: vec![] });
    }

    #[test]
    fn empty_function() {
        let m = p("fn foo()\nend");
        assert_eq!(
            m.items,
            vec![TopLevel::Function(FuncDef {
                name: "foo".into(),
                params: vec![],
                ret_type: None,
                body: vec![],
            })]
        );
    }

    #[test]
    fn function_with_params_and_return() {
        let m = p("fn add(a: int, b: int): int\n  return a + b\nend");
        let func = match &m.items[0] {
            TopLevel::Function(f) => f,
            _ => panic!("expected function"),
        };
        assert_eq!(func.name, "add");
        assert_eq!(func.params.len(), 2);
        assert_eq!(func.params[0].name, "a");
        assert_eq!(func.params[0].ty, TypeExpr::Int);
        assert_eq!(func.params[1].name, "b");
        assert_eq!(func.params[1].ty, TypeExpr::Int);
        assert_eq!(func.ret_type, Some(TypeExpr::Int));
        assert_eq!(func.body.len(), 1);
    }

    #[test]
    fn struct_definition() {
        let m = p("struct Enemy\n  x: int\n  y: int\n  alive: bool\nend");
        assert_eq!(
            m.items,
            vec![TopLevel::Struct(StructDef {
                name: "Enemy".into(),
                fields: vec![
                    Field {
                        name: "x".into(),
                        ty: TypeExpr::Int,
                    },
                    Field {
                        name: "y".into(),
                        ty: TypeExpr::Int,
                    },
                    Field {
                        name: "alive".into(),
                        ty: TypeExpr::Bool,
                    },
                ],
            })]
        );
    }

    #[test]
    fn if_else_if_else() {
        let m = p("if x == 1\n  a()\nelse if x == 2\n  b()\nelse\n  c()\nend");
        match &m.items[0] {
            TopLevel::Global(Stmt::If {
                else_ifs,
                else_body,
                ..
            }) => {
                assert_eq!(else_ifs.len(), 1);
                assert_eq!(else_body.len(), 1);
            }
            other => panic!("expected if, got {:?}", other),
        }
    }

    #[test]
    fn while_loop() {
        let m = p("while x > 0\n  x = x - 1\nend");
        match &m.items[0] {
            TopLevel::Global(Stmt::While { body, .. }) => {
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected while, got {:?}", other),
        }
    }

    #[test]
    fn for_in_loop() {
        let m = p("for i, e in enemies\n  update(e)\nend");
        match &m.items[0] {
            TopLevel::Global(Stmt::ForIn {
                index, elem, body, ..
            }) => {
                assert_eq!(index, "i");
                assert_eq!(elem, "e");
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected for-in, got {:?}", other),
        }
    }

    #[test]
    fn for_range_exclusive() {
        let m = p("for i in 0..10\n  f(i)\nend");
        match &m.items[0] {
            TopLevel::Global(Stmt::ForRange { var, inclusive, .. }) => {
                assert_eq!(var, "i");
                assert!(!inclusive);
            }
            other => panic!("expected for-range, got {:?}", other),
        }
    }

    #[test]
    fn for_range_inclusive() {
        let m = p("for i in 0..=10\n  f(i)\nend");
        match &m.items[0] {
            TopLevel::Global(Stmt::ForRange { var, inclusive, .. }) => {
                assert_eq!(var, "i");
                assert!(inclusive);
            }
            other => panic!("expected for-range, got {:?}", other),
        }
    }

    #[test]
    fn expression_precedence() {
        // 1 + 2 * 3 should parse as 1 + (2 * 3)
        let m = p("1 + 2 * 3");
        match &m.items[0] {
            TopLevel::Global(Stmt::Expression(Expr::BinOp { op, lhs, rhs })) => {
                assert_eq!(*op, BinOp::Add);
                assert_eq!(**lhs, Expr::IntLit(1));
                match rhs.as_ref() {
                    Expr::BinOp { op, lhs, rhs } => {
                        assert_eq!(*op, BinOp::Mul);
                        assert_eq!(**lhs, Expr::IntLit(2));
                        assert_eq!(**rhs, Expr::IntLit(3));
                    }
                    other => panic!("expected mul, got {:?}", other),
                }
            }
            other => panic!("expected expr stmt, got {:?}", other),
        }
    }

    #[test]
    fn function_call() {
        let m = p("cls(0)");
        match &m.items[0] {
            TopLevel::Global(Stmt::Expression(Expr::Call { func, args })) => {
                assert_eq!(**func, Expr::Ident("cls".into()));
                assert_eq!(args, &vec![Expr::IntLit(0)]);
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn array_access_and_field() {
        let m = p("enemies[0].x");
        match &m.items[0] {
            TopLevel::Global(Stmt::Expression(Expr::FieldAccess { expr, field })) => {
                assert_eq!(field, "x");
                match expr.as_ref() {
                    Expr::Index { expr, index } => {
                        assert_eq!(**expr, Expr::Ident("enemies".into()));
                        assert_eq!(**index, Expr::IntLit(0));
                    }
                    other => panic!("expected index, got {:?}", other),
                }
            }
            other => panic!("expected field access, got {:?}", other),
        }
    }

    #[test]
    fn global_assignment() {
        let m = p("x = 5");
        assert_eq!(
            m.items,
            vec![TopLevel::Global(Stmt::Assign {
                target: Expr::Ident("x".into()),
                value: Expr::IntLit(5),
            })]
        );
    }

    #[test]
    fn unary_operators() {
        let m = p("-x");
        match &m.items[0] {
            TopLevel::Global(Stmt::Expression(Expr::UnaryOp { op, expr })) => {
                assert_eq!(*op, UnaryOp::Neg);
                assert_eq!(**expr, Expr::Ident("x".into()));
            }
            other => panic!("expected unary, got {:?}", other),
        }

        let m = p("not true");
        match &m.items[0] {
            TopLevel::Global(Stmt::Expression(Expr::UnaryOp { op, expr })) => {
                assert_eq!(*op, UnaryOp::Not);
                assert_eq!(**expr, Expr::BoolLit(true));
            }
            other => panic!("expected unary, got {:?}", other),
        }
    }

    #[test]
    fn syntax_error() {
        assert!(parse("fn 123").is_err());
    }

    #[test]
    fn multiline_parens() {
        // Newlines inside parens should be suppressed by LexerAdapter
        let m = p("f(\n  1,\n  2\n)");
        match &m.items[0] {
            TopLevel::Global(Stmt::Expression(Expr::Call { args, .. })) => {
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn array_type() {
        let m = p("struct Grid\n  cells: array[100] of int\nend");
        match &m.items[0] {
            TopLevel::Struct(s) => {
                assert_eq!(
                    s.fields[0].ty,
                    TypeExpr::Array(Box::new(TypeExpr::Int), 100)
                );
            }
            other => panic!("expected struct, got {:?}", other),
        }
    }

    #[test]
    fn multiple_top_level_items() {
        let m = p("fn foo()\nend\nfn bar()\nend");
        assert_eq!(m.items.len(), 2);
    }

    #[test]
    fn break_and_continue() {
        let m = p("while true\n  break\n  continue\nend");
        match &m.items[0] {
            TopLevel::Global(Stmt::While { body, .. }) => {
                assert_eq!(body[0], Stmt::Break);
                assert_eq!(body[1], Stmt::Continue);
            }
            other => panic!("expected while, got {:?}", other),
        }
    }

    #[test]
    fn return_with_value() {
        let m = p("fn f(): int\n  return 42\nend");
        match &m.items[0] {
            TopLevel::Function(f) => {
                assert_eq!(f.body[0], Stmt::Return(Some(Expr::IntLit(42))));
            }
            other => panic!("expected function, got {:?}", other),
        }
    }

    #[test]
    fn return_without_value() {
        let m = p("fn f()\n  return\nend");
        match &m.items[0] {
            TopLevel::Function(f) => {
                assert_eq!(f.body[0], Stmt::Return(None));
            }
            other => panic!("expected function, got {:?}", other),
        }
    }

    #[test]
    fn logic_operators() {
        let m = p("a and b or c");
        // Should parse as (a and b) or c
        match &m.items[0] {
            TopLevel::Global(Stmt::Expression(Expr::BinOp { op, .. })) => {
                assert_eq!(*op, BinOp::Or);
            }
            other => panic!("expected binop, got {:?}", other),
        }
    }

    #[test]
    fn field_assignment() {
        let m = p("enemies[0].x = 10");
        match &m.items[0] {
            TopLevel::Global(Stmt::Assign { target, value }) => {
                match target {
                    Expr::FieldAccess { field, .. } => assert_eq!(field, "x"),
                    other => panic!("expected field access, got {:?}", other),
                }
                assert_eq!(*value, Expr::IntLit(10));
            }
            other => panic!("expected assign, got {:?}", other),
        }
    }

    #[test]
    fn string_literal_expr() {
        let m = p(r#"print("hello", 0, 0)"#);
        match &m.items[0] {
            TopLevel::Global(Stmt::Expression(Expr::Call { args, .. })) => {
                assert_eq!(args[0], Expr::StrLit("hello".into()));
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn var_decl_with_value() {
        let m = p("x: int = 5");
        assert_eq!(
            m.items,
            vec![TopLevel::Global(Stmt::VarDecl {
                name: "x".into(),
                ty: TypeExpr::Int,
                value: Some(Expr::IntLit(5)),
            })]
        );
    }

    #[test]
    fn var_decl_without_value() {
        let m = p("x: int");
        assert_eq!(
            m.items,
            vec![TopLevel::Global(Stmt::VarDecl {
                name: "x".into(),
                ty: TypeExpr::Int,
                value: None,
            })]
        );
    }

    #[test]
    fn var_decl_array_type() {
        let m = p("enemies: array[40] of Enemy");
        assert_eq!(
            m.items,
            vec![TopLevel::Global(Stmt::VarDecl {
                name: "enemies".into(),
                ty: TypeExpr::Array(Box::new(TypeExpr::Named("Enemy".into())), 40),
                value: None,
            })]
        );
    }
}
