use logos::Logos;
use std::fmt;
use std::ops::Range;

#[derive(Logos, Clone, Debug, PartialEq)]
#[logos(skip r"[ \t\r]+")]
pub enum Token {
    // Keywords
    #[token("fn")]
    Fn,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("end")]
    End,
    #[token("return")]
    Return,
    #[token("struct")]
    Struct,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("not")]
    Not,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("array")]
    Array,
    #[token("of")]
    Of,

    // Constants
    #[token("PI")]
    Pi,
    #[token("E", priority = 3)]
    Euler,

    // Type keywords
    #[token("int")]
    Int,
    #[token("fixed")]
    Fixed,
    #[token("bool")]
    Bool,
    #[token("str")]
    Str,
    #[token("void")]
    Void,

    // Literals
    #[regex(r"0[xX][0-9a-fA-F]+", |lex| i16::from_str_radix(&lex.slice()[2..], 16).ok())]
    #[regex(r"0[bB][01]+", |lex| i16::from_str_radix(&lex.slice()[2..], 2).ok())]
    #[regex(r"0[oO][0-7]+", |lex| i16::from_str_radix(&lex.slice()[2..], 8).ok())]
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i16>().ok())]
    IntLit(i16),

    #[regex(r"[0-9]+\.[0-9]+", |lex| Some(lex.slice().to_string()))]
    FixedLit(String),

    #[regex(r#""[^"]*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    StrLit(String),

    // Identifier
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| Some(lex.slice().to_string()))]
    Ident(String),

    // Operators (multi-char before single-char)
    #[token("==")]
    Eq,
    #[token("!=")]
    Neq,
    #[token("<=")]
    Leq,
    #[token(">=")]
    Geq,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("=")]
    Assign,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("..=")]
    DotDotEq,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,

    // Comments
    #[regex(r"//[^\n]*")]
    LineComment,

    #[regex(r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/")]
    BlockComment,

    // Newline
    #[token("\n")]
    Newline,
}

pub fn tokenize(source: &str) -> Vec<(Token, Range<usize>)> {
    Token::lexer(source)
        .spanned()
        .filter_map(|(result, span)| result.ok().map(|tok| (tok, span)))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct LexicalError {
    pub message: String,
    pub span: Range<usize>,
}

impl fmt::Display for LexicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "lexical error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

/// Bridges logos tokens into the iterator format lalrpop expects:
/// `Iterator<Item = Result<(usize, Token, usize), LexicalError>>`
///
/// Filters comments, collapses consecutive newlines, skips leading newlines,
/// and suppresses newlines inside parentheses and brackets.
pub struct LexerAdapter {
    tokens: Vec<Result<(usize, Token, usize), LexicalError>>,
    pos: usize,
}

impl LexerAdapter {
    pub fn new(source: &str) -> Self {
        let raw: Vec<_> = Token::lexer(source)
            .spanned()
            .map(|(result, span)| match result {
                Ok(tok) => Ok((span.start, tok, span.end)),
                Err(()) => Err(LexicalError {
                    message: format!("unexpected character '{}'", &source[span.start..span.end]),
                    span,
                }),
            })
            .collect();

        let mut filtered = Vec::new();
        let mut depth = 0i32;
        let mut last_was_newline = true; // treat start-of-file as "after newline" to skip leading

        for item in raw {
            match &item {
                Ok((_, Token::LineComment | Token::BlockComment, _)) => continue,
                Ok((_, Token::LParen | Token::LBracket, _)) => {
                    depth += 1;
                    last_was_newline = false;
                    filtered.push(item);
                }
                Ok((_, Token::RParen | Token::RBracket, _)) => {
                    depth -= 1;
                    last_was_newline = false;
                    filtered.push(item);
                }
                Ok((_, Token::Newline, _)) => {
                    if depth > 0 || last_was_newline {
                        continue;
                    }
                    last_was_newline = true;
                    filtered.push(item);
                }
                _ => {
                    last_was_newline = false;
                    filtered.push(item);
                }
            }
        }

        // Strip trailing newline
        if let Some(Ok((_, Token::Newline, _))) = filtered.last() {
            filtered.pop();
        }

        LexerAdapter {
            tokens: filtered,
            pos: 0,
        }
    }
}

impl Iterator for LexerAdapter {
    type Item = Result<(usize, Token, usize), LexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.tokens.len() {
            let item = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(item)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(source: &str) -> Vec<Token> {
        tokenize(source).into_iter().map(|(t, _)| t).collect()
    }

    fn adapted(source: &str) -> Vec<Token> {
        LexerAdapter::new(source).map(|r| r.unwrap().1).collect()
    }

    #[test]
    fn keywords() {
        assert_eq!(
            tokens("fn if else while for in end return"),
            vec![
                Token::Fn,
                Token::If,
                Token::Else,
                Token::While,
                Token::For,
                Token::In,
                Token::End,
                Token::Return,
            ]
        );
    }

    #[test]
    fn type_keywords() {
        assert_eq!(
            tokens("int fixed bool str void"),
            vec![
                Token::Int,
                Token::Fixed,
                Token::Bool,
                Token::Str,
                Token::Void
            ]
        );
    }

    #[test]
    fn logic_keywords() {
        assert_eq!(
            tokens("and or not true false"),
            vec![Token::And, Token::Or, Token::Not, Token::True, Token::False]
        );
    }

    #[test]
    fn struct_keywords() {
        assert_eq!(
            tokens("struct break continue array of"),
            vec![
                Token::Struct,
                Token::Break,
                Token::Continue,
                Token::Array,
                Token::Of
            ]
        );
    }

    #[test]
    fn int_literals() {
        assert_eq!(tokens("0"), vec![Token::IntLit(0)]);
        assert_eq!(tokens("42"), vec![Token::IntLit(42)]);
        assert_eq!(tokens("32767"), vec![Token::IntLit(32767)]);
    }

    #[test]
    fn negative_is_minus_plus_literal() {
        assert_eq!(tokens("-1"), vec![Token::Minus, Token::IntLit(1)]);
        assert_eq!(
            tokens("-0.5"),
            vec![Token::Minus, Token::FixedLit("0.5".to_string())]
        );
    }

    #[test]
    fn fixed_literals() {
        assert_eq!(tokens("3.14"), vec![Token::FixedLit("3.14".to_string())]);
    }

    #[test]
    fn string_literals() {
        assert_eq!(
            tokens(r#""hello""#),
            vec![Token::StrLit("hello".to_string())]
        );
        assert_eq!(
            tokens(r#""hello world""#),
            vec![Token::StrLit("hello world".to_string())]
        );
        assert_eq!(tokens(r#""""#), vec![Token::StrLit(String::new())]);
    }

    #[test]
    fn identifiers() {
        assert_eq!(tokens("x"), vec![Token::Ident("x".into())]);
        assert_eq!(tokens("foo_bar"), vec![Token::Ident("foo_bar".into())]);
        assert_eq!(tokens("_private"), vec![Token::Ident("_private".into())]);
        assert_eq!(tokens("x2"), vec![Token::Ident("x2".into())]);
    }

    #[test]
    fn identifier_not_keyword() {
        assert_eq!(tokens("fns"), vec![Token::Ident("fns".into())]);
        assert_eq!(tokens("integer"), vec![Token::Ident("integer".into())]);
        assert_eq!(tokens("iff"), vec![Token::Ident("iff".into())]);
    }

    #[test]
    fn operators() {
        assert_eq!(
            tokens("+ - * / %"),
            vec![
                Token::Plus,
                Token::Minus,
                Token::Star,
                Token::Slash,
                Token::Percent
            ]
        );
        assert_eq!(
            tokens("= == != < > <= >="),
            vec![
                Token::Assign,
                Token::Eq,
                Token::Neq,
                Token::Lt,
                Token::Gt,
                Token::Leq,
                Token::Geq,
            ]
        );
    }

    #[test]
    fn range_tokens() {
        assert_eq!(tokens(".."), vec![Token::DotDot]);
        assert_eq!(tokens("..="), vec![Token::DotDotEq]);
        assert_eq!(
            tokens("0..10"),
            vec![Token::IntLit(0), Token::DotDot, Token::IntLit(10)]
        );
        assert_eq!(
            tokens("0..=10"),
            vec![Token::IntLit(0), Token::DotDotEq, Token::IntLit(10)]
        );
    }

    #[test]
    fn delimiters() {
        assert_eq!(
            tokens("( ) [ ] , : ."),
            vec![
                Token::LParen,
                Token::RParen,
                Token::LBracket,
                Token::RBracket,
                Token::Comma,
                Token::Colon,
                Token::Dot,
            ]
        );
    }

    #[test]
    fn line_comment() {
        assert_eq!(
            tokens("x = 5 // a comment"),
            vec![
                Token::Ident("x".into()),
                Token::Assign,
                Token::IntLit(5),
                Token::LineComment
            ]
        );
    }

    #[test]
    fn block_comment() {
        assert_eq!(
            tokens("x /* comment */ y"),
            vec![
                Token::Ident("x".into()),
                Token::BlockComment,
                Token::Ident("y".into()),
            ]
        );
    }

    #[test]
    fn newlines() {
        assert_eq!(
            tokens("x\ny"),
            vec![
                Token::Ident("x".into()),
                Token::Newline,
                Token::Ident("y".into()),
            ]
        );
    }

    #[test]
    fn spans() {
        let result = tokenize("x = 5");
        assert_eq!(result[0], (Token::Ident("x".into()), 0..1));
        assert_eq!(result[1], (Token::Assign, 2..3));
        assert_eq!(result[2], (Token::IntLit(5), 4..5));
    }

    #[test]
    fn small_program() {
        let source = "fn main()\n  cls(0)\n  flip()\nend";
        let toks = tokens(source);
        assert_eq!(
            toks,
            vec![
                Token::Fn,
                Token::Ident("main".into()),
                Token::LParen,
                Token::RParen,
                Token::Newline,
                Token::Ident("cls".into()),
                Token::LParen,
                Token::IntLit(0),
                Token::RParen,
                Token::Newline,
                Token::Ident("flip".into()),
                Token::LParen,
                Token::RParen,
                Token::Newline,
                Token::End,
            ]
        );
    }

    #[test]
    fn struct_and_array() {
        let source = "enemies: array[40] of Enemy";
        let toks = tokens(source);
        assert_eq!(
            toks,
            vec![
                Token::Ident("enemies".into()),
                Token::Colon,
                Token::Array,
                Token::LBracket,
                Token::IntLit(40),
                Token::RBracket,
                Token::Of,
                Token::Ident("Enemy".into()),
            ]
        );
    }

    #[test]
    fn for_loop() {
        let source = "for i, e in enemies\nend";
        let toks = tokens(source);
        assert_eq!(
            toks,
            vec![
                Token::For,
                Token::Ident("i".into()),
                Token::Comma,
                Token::Ident("e".into()),
                Token::In,
                Token::Ident("enemies".into()),
                Token::Newline,
                Token::End,
            ]
        );
    }

    #[test]
    fn hex_literals() {
        assert_eq!(tokens("0xFF"), vec![Token::IntLit(255)]);
        assert_eq!(tokens("0x0"), vec![Token::IntLit(0)]);
        assert_eq!(tokens("0x7FFF"), vec![Token::IntLit(32767)]);
        assert_eq!(tokens("0XAB"), vec![Token::IntLit(0xAB)]);
    }

    #[test]
    fn binary_literals() {
        assert_eq!(tokens("0b1010"), vec![Token::IntLit(0b1010)]);
        assert_eq!(tokens("0b0"), vec![Token::IntLit(0)]);
        assert_eq!(tokens("0B11111111"), vec![Token::IntLit(255)]);
    }

    #[test]
    fn octal_literals() {
        assert_eq!(tokens("0o17"), vec![Token::IntLit(15)]);
        assert_eq!(tokens("0o0"), vec![Token::IntLit(0)]);
        assert_eq!(tokens("0O377"), vec![Token::IntLit(255)]);
    }

    #[test]
    fn overflow_hex_skipped() {
        assert!(tokens("0xFFFF").is_empty());
    }

    #[test]
    fn overflow_bin_skipped() {
        assert!(tokens("0b1000000000000000").is_empty());
    }

    #[test]
    fn overflow_oct_skipped() {
        assert!(tokens("0o200000").is_empty());
    }

    #[test]
    fn overflow_int_skipped() {
        let toks = tokens("99999");
        assert!(toks.is_empty());
    }

    #[test]
    fn whitespace_skipped() {
        assert_eq!(tokens("  x  "), vec![Token::Ident("x".into())]);
    }

    #[test]
    fn empty_source() {
        assert!(tokens("").is_empty());
    }

    // LexerAdapter tests

    #[test]
    fn adapter_filters_comments() {
        assert_eq!(
            adapted("x // comment\ny"),
            vec![
                Token::Ident("x".into()),
                Token::Newline,
                Token::Ident("y".into()),
            ]
        );
        assert_eq!(
            adapted("x /* block */ y"),
            vec![Token::Ident("x".into()), Token::Ident("y".into()),]
        );
    }

    #[test]
    fn adapter_collapses_newlines() {
        assert_eq!(
            adapted("x\n\n\ny"),
            vec![
                Token::Ident("x".into()),
                Token::Newline,
                Token::Ident("y".into()),
            ]
        );
    }

    #[test]
    fn adapter_skips_leading_newlines() {
        assert_eq!(
            adapted("\n\nx = 1"),
            vec![Token::Ident("x".into()), Token::Assign, Token::IntLit(1),]
        );
    }

    #[test]
    fn adapter_strips_trailing_newline() {
        assert_eq!(
            adapted("x = 1\n"),
            vec![Token::Ident("x".into()), Token::Assign, Token::IntLit(1),]
        );
    }

    #[test]
    fn adapter_suppresses_newlines_in_parens() {
        assert_eq!(
            adapted("f(\nx,\ny\n)"),
            vec![
                Token::Ident("f".into()),
                Token::LParen,
                Token::Ident("x".into()),
                Token::Comma,
                Token::Ident("y".into()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn adapter_suppresses_newlines_in_brackets() {
        assert_eq!(
            adapted("a[\n0\n]"),
            vec![
                Token::Ident("a".into()),
                Token::LBracket,
                Token::IntLit(0),
                Token::RBracket,
            ]
        );
    }

    #[test]
    fn adapter_range_tokens() {
        assert_eq!(
            adapted("0..10"),
            vec![Token::IntLit(0), Token::DotDot, Token::IntLit(10)]
        );
        assert_eq!(
            adapted("0..=10"),
            vec![Token::IntLit(0), Token::DotDotEq, Token::IntLit(10)]
        );
    }
}
