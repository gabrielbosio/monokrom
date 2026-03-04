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
    let hir = lower(&ast).map_err(|errs| {
        errs.iter()
            .map(|e| {
                if let Some(span) = e.span {
                    format!("line {}: {}", offset_to_line(source, span.0), e.message)
                } else {
                    e.message.clone()
                }
            })
            .collect::<Vec<_>>()
    })?;
    let mut lir = lower_lir(&hir);
    opt::optimize(&mut lir);
    let mut bc = codegen::generate(&lir).map_err(|e| vec![format!("{e}")])?;
    peephole::optimize(&mut bc);
    Ok(bc)
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::lir::*;
    use super::opt::optimize;
    use super::{lower, parse};

    pub fn optimized_lir(src: &str) -> LirModule {
        let ast = parse(src).unwrap();
        let hir = lower(&ast).unwrap();
        let mut module = super::hir_to_lir::lower_to_lir(&hir);
        optimize(&mut module);
        module
    }

    pub fn find_func<'a>(m: &'a LirModule, name: &str) -> &'a LirFunc {
        m.functions.iter().find(|f| f.name == name).unwrap()
    }

    pub fn has_inst(f: &LirFunc, pred: impl Fn(&LirInst) -> bool) -> bool {
        f.blocks
            .iter()
            .any(|b| b.insts.iter().any(|(_, inst)| pred(inst)))
    }
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
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::If {
            else_ifs,
            else_body,
            ..
        } = &s.kind
        else {
            panic!("expected if, got {:?}", s.kind)
        };
        assert_eq!(else_ifs.len(), 1);
        assert_eq!(else_body.len(), 1);
    }

    #[test]
    fn while_loop() {
        let m = p("while x > 0\n  x = x - 1\nend");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::While { body, .. } = &s.kind else {
            panic!("expected while, got {:?}", s.kind)
        };
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn for_in_loop() {
        let m = p("for i, e in enemies\n  update(e)\nend");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::ForIn {
            index, elem, body, ..
        } = &s.kind
        else {
            panic!("expected for-in, got {:?}", s.kind)
        };
        assert_eq!(index, "i");
        assert_eq!(elem, "e");
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn for_range_exclusive() {
        let m = p("for i in 0..10\n  f(i)\nend");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::ForRange { var, inclusive, .. } = &s.kind else {
            panic!("expected for-range, got {:?}", s.kind)
        };
        assert_eq!(var, "i");
        assert!(!inclusive);
    }

    #[test]
    fn for_range_inclusive() {
        let m = p("for i in 0..=10\n  f(i)\nend");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::ForRange { var, inclusive, .. } = &s.kind else {
            panic!("expected for-range, got {:?}", s.kind)
        };
        assert_eq!(var, "i");
        assert!(inclusive);
    }

    #[test]
    fn expression_precedence() {
        // 1 + 2 * 3 should parse as 1 + (2 * 3)
        let m = p("1 + 2 * 3");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::Expression(e) = &s.kind else {
            panic!("expected expression")
        };
        let ExprKind::BinOp { op, lhs, rhs } = &e.kind else {
            panic!("expected binop")
        };
        assert_eq!(*op, BinOp::Add);
        assert_eq!(lhs.kind, ExprKind::IntLit(1));
        let ExprKind::BinOp { op, lhs, rhs } = &rhs.kind else {
            panic!("expected mul")
        };
        assert_eq!(*op, BinOp::Mul);
        assert_eq!(lhs.kind, ExprKind::IntLit(2));
        assert_eq!(rhs.kind, ExprKind::IntLit(3));
    }

    #[test]
    fn function_call() {
        let m = p("cls(0)");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::Expression(e) = &s.kind else {
            panic!("expected expression")
        };
        let ExprKind::Call { func, args } = &e.kind else {
            panic!("expected call")
        };
        assert_eq!(func.kind, ExprKind::Ident("cls".into()));
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].kind, ExprKind::IntLit(0));
    }

    #[test]
    fn array_access_and_field() {
        let m = p("enemies[0].x");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::Expression(e) = &s.kind else {
            panic!("expected expression")
        };
        let ExprKind::FieldAccess { expr, field } = &e.kind else {
            panic!("expected field access")
        };
        assert_eq!(field, "x");
        let ExprKind::Index { expr, index } = &expr.kind else {
            panic!("expected index")
        };
        assert_eq!(expr.kind, ExprKind::Ident("enemies".into()));
        assert_eq!(index.kind, ExprKind::IntLit(0));
    }

    #[test]
    fn global_assignment() {
        let m = p("x = 5");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::Assign { target, value } = &s.kind else {
            panic!("expected assign")
        };
        assert_eq!(target.kind, ExprKind::Ident("x".into()));
        assert_eq!(value.kind, ExprKind::IntLit(5));
    }

    #[test]
    fn unary_operators() {
        let m = p("-x");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::Expression(e) = &s.kind else {
            panic!("expected expression")
        };
        let ExprKind::UnaryOp { op, expr } = &e.kind else {
            panic!("expected unary")
        };
        assert_eq!(*op, UnaryOp::Neg);
        assert_eq!(expr.kind, ExprKind::Ident("x".into()));

        let m = p("not true");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::Expression(e) = &s.kind else {
            panic!("expected expression")
        };
        let ExprKind::UnaryOp { op, expr } = &e.kind else {
            panic!("expected unary")
        };
        assert_eq!(*op, UnaryOp::Not);
        assert_eq!(expr.kind, ExprKind::BoolLit(true));
    }

    #[test]
    fn syntax_error() {
        assert!(parse("fn 123").is_err());
    }

    #[test]
    fn multiline_parens() {
        // Newlines inside parens should be suppressed by LexerAdapter
        let m = p("f(\n  1,\n  2\n)");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::Expression(e) = &s.kind else {
            panic!("expected expression")
        };
        let ExprKind::Call { args, .. } = &e.kind else {
            panic!("expected call")
        };
        assert_eq!(args.len(), 2);
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
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::While { body, .. } = &s.kind else {
            panic!("expected while")
        };
        assert_eq!(body[0].kind, StmtKind::Break);
        assert_eq!(body[1].kind, StmtKind::Continue);
    }

    #[test]
    fn return_with_value() {
        let m = p("fn f(): int\n  return 42\nend");
        let TopLevel::Function(f) = &m.items[0] else {
            panic!("expected function")
        };
        let StmtKind::Return(Some(e)) = &f.body[0].kind else {
            panic!("expected return with value")
        };
        assert_eq!(e.kind, ExprKind::IntLit(42));
    }

    #[test]
    fn return_without_value() {
        let m = p("fn f()\n  return\nend");
        let TopLevel::Function(f) = &m.items[0] else {
            panic!("expected function")
        };
        assert_eq!(f.body[0].kind, StmtKind::Return(None));
    }

    #[test]
    fn logic_operators() {
        let m = p("a and b or c");
        // Should parse as (a and b) or c
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::Expression(e) = &s.kind else {
            panic!("expected expression")
        };
        let ExprKind::BinOp { op, .. } = &e.kind else {
            panic!("expected binop")
        };
        assert_eq!(*op, BinOp::Or);
    }

    #[test]
    fn field_assignment() {
        let m = p("enemies[0].x = 10");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::Assign { target, value } = &s.kind else {
            panic!("expected assign")
        };
        let ExprKind::FieldAccess { field, .. } = &target.kind else {
            panic!("expected field access")
        };
        assert_eq!(field, "x");
        assert_eq!(value.kind, ExprKind::IntLit(10));
    }

    #[test]
    fn string_literal_expr() {
        let m = p(r#"print("hello", 0, 0)"#);
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::Expression(e) = &s.kind else {
            panic!("expected expression")
        };
        let ExprKind::Call { args, .. } = &e.kind else {
            panic!("expected call")
        };
        assert_eq!(args[0].kind, ExprKind::StrLit("hello".into()));
    }

    #[test]
    fn var_decl_with_value() {
        let m = p("x: int = 5");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::VarDecl { name, ty, value } = &s.kind else {
            panic!("expected var decl")
        };
        assert_eq!(name, "x");
        assert_eq!(*ty, TypeExpr::Int);
        assert_eq!(value.as_ref().unwrap().kind, ExprKind::IntLit(5));
    }

    #[test]
    fn var_decl_without_value() {
        let m = p("x: int");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::VarDecl { name, ty, value } = &s.kind else {
            panic!("expected var decl")
        };
        assert_eq!(name, "x");
        assert_eq!(*ty, TypeExpr::Int);
        assert!(value.is_none());
    }

    #[test]
    fn var_decl_array_type() {
        let m = p("enemies: array[40] of Enemy");
        let TopLevel::Global(s) = &m.items[0] else {
            panic!("expected global")
        };
        let StmtKind::VarDecl { name, ty, .. } = &s.kind else {
            panic!("expected var decl")
        };
        assert_eq!(name, "enemies");
        assert_eq!(
            *ty,
            TypeExpr::Array(Box::new(TypeExpr::Named("Enemy".into())), 40)
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

    #[test]
    fn compile_type_error_has_line_number() {
        let errs = super::compile("fn f()\n  x: int = true\nend").unwrap_err();
        assert!(errs.iter().any(|e| e.starts_with("line ")));
    }
}
