use logos::Logos;
use std::ops::Range;

#[derive(Logos, Clone, Debug, PartialEq)]
#[logos(skip r"[ \t\r]+")]
pub enum Token {
    // Keywords
    #[token("fn")]
    Fn,
    #[token("if")]
    If,
    #[token("then")]
    Then,
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
    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i16>().ok())]
    IntLit(i16),

    #[regex(r"-?[0-9]+\.[0-9]+", |lex| Some(lex.slice().to_string()))]
    FixedLit(String),

    #[regex(r#""[^"]*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    StrLit(String),

    // Identifier
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,

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

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(source: &str) -> Vec<Token> {
        tokenize(source).into_iter().map(|(t, _)| t).collect()
    }

    #[test]
    fn keywords() {
        assert_eq!(
            tokens("fn if then else while for in end return"),
            vec![
                Token::Fn,
                Token::If,
                Token::Then,
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
        assert_eq!(tokens("-1"), vec![Token::IntLit(-1)]);
        assert_eq!(tokens("32767"), vec![Token::IntLit(32767)]);
        assert_eq!(tokens("-32768"), vec![Token::IntLit(-32768)]);
    }

    #[test]
    fn fixed_literals() {
        assert_eq!(tokens("3.14"), vec![Token::FixedLit("3.14".to_string())]);
        assert_eq!(tokens("-0.5"), vec![Token::FixedLit("-0.5".to_string())]);
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
        assert_eq!(tokens("x"), vec![Token::Ident]);
        assert_eq!(tokens("foo_bar"), vec![Token::Ident]);
        assert_eq!(tokens("_private"), vec![Token::Ident]);
        assert_eq!(tokens("x2"), vec![Token::Ident]);
    }

    #[test]
    fn identifier_not_keyword() {
        // "fns" should be Ident, not Fn + Ident
        assert_eq!(tokens("fns"), vec![Token::Ident]);
        assert_eq!(tokens("integer"), vec![Token::Ident]);
        assert_eq!(tokens("iff"), vec![Token::Ident]);
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
                Token::Ident,
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
            vec![Token::Ident, Token::BlockComment, Token::Ident]
        );
    }

    #[test]
    fn newlines() {
        assert_eq!(
            tokens("x\ny"),
            vec![Token::Ident, Token::Newline, Token::Ident]
        );
    }

    #[test]
    fn spans() {
        let result = tokenize("x = 5");
        assert_eq!(result[0], (Token::Ident, 0..1));
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
                Token::Ident, // main
                Token::LParen,
                Token::RParen,
                Token::Newline,
                Token::Ident, // cls
                Token::LParen,
                Token::IntLit(0),
                Token::RParen,
                Token::Newline,
                Token::Ident, // flip
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
                Token::Ident, // enemies
                Token::Colon,
                Token::Array,
                Token::LBracket,
                Token::IntLit(40),
                Token::RBracket,
                Token::Of,
                Token::Ident, // Enemy
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
                Token::Ident, // i
                Token::Comma,
                Token::Ident, // e
                Token::In,
                Token::Ident, // enemies
                Token::Newline,
                Token::End,
            ]
        );
    }

    #[test]
    fn overflow_int_skipped() {
        // 99999 overflows i16, should be skipped (no token produced)
        let toks = tokens("99999");
        assert!(toks.is_empty());
    }

    #[test]
    fn whitespace_skipped() {
        assert_eq!(tokens("  x  "), vec![Token::Ident]);
    }

    #[test]
    fn empty_source() {
        assert!(tokens("").is_empty());
    }
}
