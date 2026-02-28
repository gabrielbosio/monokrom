pub mod ast;
pub mod bytecode;
pub mod codegen;
pub mod error;
pub mod hir;
pub mod hir_to_lir;
pub mod lexer;
pub mod lir;
pub mod opt;
pub mod peephole;
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

fn offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

fn token_description(token: &lexer::Token) -> &str {
    use lexer::Token::*;
    match token {
        Newline => "end of line",
        Ident(_) => "identifier",
        IntLit(_) => "number",
        FixedLit(_) => "number",
        StrLit(_) => "string",
        Fn => "'fn'",
        If => "'if'",
        Else => "'else'",
        While => "'while'",
        For => "'for'",
        In => "'in'",
        End => "'end'",
        Return => "'return'",
        Struct => "'struct'",
        True => "'true'",
        False => "'false'",
        And => "'and'",
        Or => "'or'",
        Not => "'not'",
        Break => "'break'",
        Continue => "'continue'",
        Array => "'array'",
        Of => "'of'",
        Int => "'int'",
        Fixed => "'fixed'",
        Bool => "'bool'",
        Str => "'str'",
        Void => "'void'",
        Pi => "'PI'",
        Euler => "'E'",
        Eq => "'=='",
        Neq => "'!='",
        Leq => "'<='",
        Geq => "'>='",
        Plus => "'+'",
        Minus => "'-'",
        Star => "'*'",
        Slash => "'/'",
        Percent => "'%'",
        Assign => "'='",
        Lt => "'<'",
        Gt => "'>'",
        LParen => "'('",
        RParen => "')'",
        LBracket => "'['",
        RBracket => "']'",
        Comma => "','",
        Colon => "':'",
        DotDotEq => "'..='",
        DotDot => "'..'",
        Dot => "'.'",
        LineComment | BlockComment => "comment",
    }
}

fn simplify_expected(expected: &[String]) -> String {
    // Expression-start tokens in lalrpop grammar names
    const EXPR_TOKENS: &[&str] = &[
        "INT",
        "FIXED",
        "STRING",
        "IDENT",
        "\"(\"",
        "\"true\"",
        "\"false\"",
        "\"not\"",
        "\"-\"",
        "\"PI\"",
        "\"E\"",
    ];

    let expr_count = expected
        .iter()
        .filter(|e| EXPR_TOKENS.contains(&e.as_str()))
        .count();
    if expr_count >= 4 {
        // Most expression-start tokens present, so summarize
        let extras: Vec<_> = expected
            .iter()
            .filter(|e| !EXPR_TOKENS.contains(&e.as_str()))
            .map(|e| grammar_name_to_readable(e))
            .collect();
        if extras.is_empty() {
            return "expression".to_string();
        }
        let mut parts = vec!["expression".to_string()];
        parts.extend(extras);
        return parts.join(", ");
    }

    let readable: Vec<_> = expected
        .iter()
        .map(|e| grammar_name_to_readable(e))
        .collect();
    readable.join(", ")
}

fn grammar_name_to_readable(name: &str) -> String {
    match name {
        "NL" => "end of line".to_string(),
        "INT" | "FIXED" => "number".to_string(),
        "STRING" => "string".to_string(),
        "IDENT" => "identifier".to_string(),
        s if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 => {
            // e.g. "\"fn\"" -> 'fn'
            let inner = &s[1..s.len() - 1];
            format!("'{inner}'")
        }
        s => s.to_string(),
    }
}

fn format_parse_error(
    source: &str,
    error: &lalrpop_util::ParseError<usize, lexer::Token, lexer::LexicalError>,
) -> String {
    use lalrpop_util::ParseError::*;
    match error {
        UnrecognizedToken {
            token: (offset, token, _),
            expected,
        } => {
            let line = offset_to_line(source, *offset);
            let desc = token_description(token);
            let exp = simplify_expected(expected);
            format!("line {line}: unexpected {desc}, expected {exp}")
        }
        UnrecognizedEof { location, expected } => {
            let line = offset_to_line(source, *location);
            let exp = simplify_expected(expected);
            format!("line {line}: unexpected end of file, expected {exp}")
        }
        InvalidToken { location } => {
            let line = offset_to_line(source, *location);
            format!("line {line}: unexpected character")
        }
        ExtraToken {
            token: (offset, token, _),
        } => {
            let line = offset_to_line(source, *offset);
            let desc = token_description(token);
            format!("line {line}: unexpected {desc}")
        }
        User { error } => format!("{error}"),
    }
}

pub fn compile(source: &str) -> Result<bytecode::Bytecode, Vec<String>> {
    let ast = parse(source).map_err(|e| vec![format_parse_error(source, &e)])?;
    let hir =
        lower(&ast).map_err(|errs| errs.iter().map(|e| format!("{e}")).collect::<Vec<_>>())?;
    let mut lir = lower_lir(&hir);
    opt::optimize(&mut lir);
    let mut bc = codegen::generate(&lir).map_err(|e| vec![format!("{e}")])?;
    peephole::optimize(&mut bc);
    Ok(bc)
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

    #[test]
    fn compile_end_to_end() {
        let bc = super::compile("fn main()\n  cls(0)\nend").unwrap();
        assert!(bc.entry_point.is_some());
        assert!(!bc.code.is_empty());
    }

    #[test]
    fn compile_parse_error() {
        let errs = super::compile("fn 123").unwrap_err();
        assert!(errs[0].contains("unexpected number"));
    }

    #[test]
    fn compile_type_error() {
        let errs = super::compile("fn f(): int\nend").unwrap_err();
        assert!(!errs.is_empty());
    }
}
