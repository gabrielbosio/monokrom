use crate::compiler::lexer::{tokenize, Token};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CharStyle {
    Keyword,
    Normal,
    Comment,
}

/// Returns a Vec<CharStyle> with one entry per char of `source`.
/// Chars not covered by any token default to Normal.
pub fn highlight(source: &str) -> Vec<CharStyle> {
    let mut byte_styles = vec![CharStyle::Normal; source.len()];
    for (token, span) in tokenize(source) {
        let style = classify(&token);
        for i in span {
            byte_styles[i] = style;
        }
    }
    source
        .char_indices()
        .map(|(byte_idx, _)| byte_styles[byte_idx])
        .collect()
}

fn classify(token: &Token) -> CharStyle {
    match token {
        Token::Fn
        | Token::If
        | Token::Else
        | Token::While
        | Token::For
        | Token::In
        | Token::End
        | Token::Return
        | Token::Struct
        | Token::True
        | Token::False
        | Token::And
        | Token::Or
        | Token::Not
        | Token::Break
        | Token::Continue
        | Token::Array
        | Token::Of
        | Token::Int
        | Token::Fixed
        | Token::Bool
        | Token::Str
        | Token::Void
        | Token::Pi
        | Token::Euler => CharStyle::Keyword,

        Token::LineComment | Token::BlockComment => CharStyle::Comment,

        _ => CharStyle::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_are_keyword_style() {
        let styles = highlight("fn if end");
        assert_eq!(styles[0], CharStyle::Keyword); // f
        assert_eq!(styles[1], CharStyle::Keyword); // n
        assert_eq!(styles[2], CharStyle::Normal); // space
        assert_eq!(styles[3], CharStyle::Keyword); // i
        assert_eq!(styles[4], CharStyle::Keyword); // f
        assert_eq!(styles[5], CharStyle::Normal); // space
        assert_eq!(styles[6], CharStyle::Keyword); // e
        assert_eq!(styles[7], CharStyle::Keyword); // n
        assert_eq!(styles[8], CharStyle::Keyword); // d
    }

    #[test]
    fn line_comment() {
        let styles = highlight("x // comment");
        assert_eq!(styles[0], CharStyle::Normal); // x
        assert_eq!(styles[1], CharStyle::Normal); // space
        assert_eq!(styles[2], CharStyle::Comment); // /
        assert_eq!(styles[11], CharStyle::Comment); // t
    }

    #[test]
    fn block_comment() {
        let styles = highlight("x /* hi */ y");
        assert_eq!(styles[0], CharStyle::Normal); // x
        assert_eq!(styles[2], CharStyle::Comment); // /
        assert_eq!(styles[3], CharStyle::Comment); // *
        assert_eq!(styles[9], CharStyle::Comment); // /
        assert_eq!(styles[11], CharStyle::Normal); // y
    }

    #[test]
    fn identifiers_and_literals_are_normal() {
        let styles = highlight("foo 42");
        for s in &styles {
            assert_eq!(*s, CharStyle::Normal);
        }
    }

    #[test]
    fn mixed_program() {
        let styles = highlight("fn main()\n  cls(0)\nend");
        // "fn" = keyword
        assert_eq!(styles[0], CharStyle::Keyword);
        assert_eq!(styles[1], CharStyle::Keyword);
        // "main" = normal
        assert_eq!(styles[3], CharStyle::Normal);
        // "end" at the end
        let end_start = "fn main()\n  cls(0)\n".len();
        assert_eq!(styles[end_start], CharStyle::Keyword);
    }

    #[test]
    fn empty_source() {
        assert!(highlight("").is_empty());
    }

    #[test]
    fn type_keywords() {
        let styles = highlight("int bool str");
        assert_eq!(styles[0], CharStyle::Keyword); // i
        assert_eq!(styles[3], CharStyle::Normal); // space
        assert_eq!(styles[4], CharStyle::Keyword); // b
        assert_eq!(styles[9], CharStyle::Keyword); // s
    }
}
