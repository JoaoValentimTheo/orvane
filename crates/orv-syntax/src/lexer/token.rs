//! Token and token-kind definitions for the Orvane lexer (SPEC §5.1).

use crate::span::Span;

/// A lexed token: its kind plus the byte span it occupies (ADR 0001).
#[derive(Clone, PartialEq, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    /// Creates a token.
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// The `Keyword` behind this token, if it is a keyword.
    pub const fn keyword(&self) -> Option<Keyword> {
        match self.kind {
            TokenKind::Kw(kw) => Some(kw),
            _ => None,
        }
    }

    /// Whether this token is a `Newline`.
    pub const fn is_newline(&self) -> bool {
        matches!(self.kind, TokenKind::Newline)
    }

    /// Whether this token is the end of file.
    pub const fn is_eof(&self) -> bool {
        matches!(self.kind, TokenKind::Eof)
    }
}

/// Every token kind in v0.1.
///
/// `py` is deliberately **not** a keyword (SPEC §5.1): it is contextual, so the
/// lexer always produces [`TokenKind::Ident`] for it and the parser decides.
#[derive(Clone, PartialEq, Debug)]
pub enum TokenKind {
    /// Integer literal, already parsed into `i64`.
    Int(i64),
    /// Float literal, already parsed into `f64`.
    Float(f64),
    /// String literal, split into literal and interpolated parts.
    Str(Vec<StrPart>),
    /// Identifier. `py` arrives here, never as [`TokenKind::Kw`].
    Ident(String),
    /// Reserved keyword (see [`Keyword`]).
    Kw(Keyword),
    /// A collapsed line break, emitted per the delimiter-stack rule (§5.1).
    Newline,
    /// End of input. Always the last token.
    Eof,

    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// The `#{` map-literal opener. Lexed as a single token.
    HashLBrace,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `;`
    Semicolon,
    /// `.`
    Dot,
    /// `..`
    DotDot,
    /// `..=`
    DotDotEq,
    /// `->`
    Arrow,
    /// `=>`
    FatArrow,
    /// `?`
    Question,
    /// `?.`
    QuestionDot,
    /// `??`
    QuestionQuestion,
    /// `=`
    Eq,
    /// `==`
    EqEq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `+=`
    PlusEq,
    /// `-=`
    MinusEq,
    /// `*=`
    StarEq,
    /// `/=`
    SlashEq,
}

/// One piece of a string literal.
///
/// The lexer resolves escapes in [`StrPart::Lit`] and keeps the raw source of
/// interpolated expressions in [`StrPart::Expr`], so the parser can re-lex and
/// parse them with correct spans (SPEC §5.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StrPart {
    /// Literal text with escapes already resolved.
    Lit(String),
    /// An interpolated `{expr}`; `src` is the expression source and `span` its
    /// byte range **inside the file**.
    Expr { src: String, span: Span },
}

/// Reserved keywords (SPEC §5.1, without `py`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Keyword {
    Fn,
    Intent,
    How,
    Given,
    Ensure,
    When,
    Via,
    Priority,
    Data,
    Enum,
    Let,
    Mut,
    If,
    Else,
    Match,
    For,
    In,
    While,
    Return,
    Break,
    Continue,
    Use,
    As,
    Try,
    Fail,
    True,
    False,
    None,
    And,
    Or,
    Not,
    Pub,
    Test,
}

impl Keyword {
    /// Looks a keyword up by identifier text.
    ///
    /// Returns `None` for `py`, which is contextual rather than reserved.
    pub fn from_str(text: &str) -> Option<Keyword> {
        let keyword = match text {
            "fn" => Keyword::Fn,
            "intent" => Keyword::Intent,
            "how" => Keyword::How,
            "given" => Keyword::Given,
            "ensure" => Keyword::Ensure,
            "when" => Keyword::When,
            "via" => Keyword::Via,
            "priority" => Keyword::Priority,
            "data" => Keyword::Data,
            "enum" => Keyword::Enum,
            "let" => Keyword::Let,
            "mut" => Keyword::Mut,
            "if" => Keyword::If,
            "else" => Keyword::Else,
            "match" => Keyword::Match,
            "for" => Keyword::For,
            "in" => Keyword::In,
            "while" => Keyword::While,
            "return" => Keyword::Return,
            "break" => Keyword::Break,
            "continue" => Keyword::Continue,
            "use" => Keyword::Use,
            "as" => Keyword::As,
            "try" => Keyword::Try,
            "fail" => Keyword::Fail,
            "true" => Keyword::True,
            "false" => Keyword::False,
            "none" => Keyword::None,
            "and" => Keyword::And,
            "or" => Keyword::Or,
            "not" => Keyword::Not,
            "pub" => Keyword::Pub,
            "test" => Keyword::Test,
            _ => return None,
        };
        Some(keyword)
    }

    /// The source spelling of the keyword.
    pub const fn as_str(self) -> &'static str {
        match self {
            Keyword::Fn => "fn",
            Keyword::Intent => "intent",
            Keyword::How => "how",
            Keyword::Given => "given",
            Keyword::Ensure => "ensure",
            Keyword::When => "when",
            Keyword::Via => "via",
            Keyword::Priority => "priority",
            Keyword::Data => "data",
            Keyword::Enum => "enum",
            Keyword::Let => "let",
            Keyword::Mut => "mut",
            Keyword::If => "if",
            Keyword::Else => "else",
            Keyword::Match => "match",
            Keyword::For => "for",
            Keyword::In => "in",
            Keyword::While => "while",
            Keyword::Return => "return",
            Keyword::Break => "break",
            Keyword::Continue => "continue",
            Keyword::Use => "use",
            Keyword::As => "as",
            Keyword::Try => "try",
            Keyword::Fail => "fail",
            Keyword::True => "true",
            Keyword::False => "false",
            Keyword::None => "none",
            Keyword::And => "and",
            Keyword::Or => "or",
            Keyword::Not => "not",
            Keyword::Pub => "pub",
            Keyword::Test => "test",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::FileId;

    fn span() -> Span {
        Span::new(FileId(0), 0, 1)
    }

    #[test]
    fn keyword_table_covers_spec_list() {
        let spec = [
            "fn", "intent", "how", "given", "ensure", "when", "via", "priority", "data", "enum",
            "let", "mut", "if", "else", "match", "for", "in", "while", "return", "break",
            "continue", "use", "as", "try", "fail", "true", "false", "none", "and", "or", "not",
            "pub", "test",
        ];
        for text in spec {
            let kw = Keyword::from_str(text).unwrap_or_else(|| panic!("missing keyword {text}"));
            assert_eq!(kw.as_str(), text);
        }
    }

    #[test]
    fn py_is_not_a_reserved_keyword() {
        assert_eq!(Keyword::from_str("py"), None);
    }

    #[test]
    fn keyword_lookup_rejects_identifiers() {
        for text in ["py", "main", "Fn", "FN", "lets", "if_", "none2", ""] {
            assert_eq!(Keyword::from_str(text), None, "{text} must not be a keyword");
        }
    }

    #[test]
    fn token_exposes_keyword() {
        let t = Token::new(TokenKind::Kw(Keyword::Fn), span());
        assert_eq!(t.keyword(), Some(Keyword::Fn));
        assert!(!t.is_newline());
        assert!(!t.is_eof());

        let i = Token::new(TokenKind::Ident("fn".to_owned()), span());
        assert_eq!(i.keyword(), None);
    }

    #[test]
    fn newline_and_eof_predicates() {
        assert!(Token::new(TokenKind::Newline, span()).is_newline());
        assert!(Token::new(TokenKind::Eof, span()).is_eof());
        assert!(!Token::new(TokenKind::Eof, span()).is_newline());
    }

    #[test]
    fn str_part_variants_are_distinct() {
        let lit = StrPart::Lit("a".to_owned());
        let expr = StrPart::Expr {
            src: "a".to_owned(),
            span: span(),
        };
        assert_ne!(lit, expr);
    }
}
