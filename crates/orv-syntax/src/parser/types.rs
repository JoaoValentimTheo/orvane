//! Type annotation parsing (SPEC §5.2 `type`, §5.3).

use crate::ast::{Type, TypeKind};
use crate::lexer::{Keyword, TokenKind};

use super::Parser;

impl Parser<'_> {
    /// Parses a type annotation in statement or item position.
    pub(crate) fn annotation_type(&mut self) -> Option<Type> {
        self.parse_type()
    }

    /// Parses `base_type [ "?" ]`.
    pub(crate) fn parse_type(&mut self) -> Option<Type> {
        let base = self.base_type()?;
        if self.kind() == &TokenKind::Question {
            let close = self.bump();
            let span = base.span.to(close.span);
            return Some(Type::new(TypeKind::Optional(Box::new(base)), span));
        }
        Some(base)
    }

    /// Parses a `base_type` (SPEC §5.2).
    fn base_type(&mut self) -> Option<Type> {
        let token = self.current().clone();
        match token.kind.clone() {
            // `Py` is a contextual name, not a keyword (§5.1); it is reserved
            // here so `Py as T` parses, even though the alpha cannot produce a
            // `Py` value (ADR 0012).
            TokenKind::Ident(name) if name == "Py" => {
                self.bump();
                Some(Type::new(TypeKind::Py, token.span))
            }
            TokenKind::Ident(name) => {
                self.bump();
                let mut args = Vec::new();
                if self.kind() == &TokenKind::Lt {
                    self.bump();
                    loop {
                        args.push(self.parse_type()?);
                        match self.kind() {
                            TokenKind::Comma => {
                                self.bump();
                            }
                            TokenKind::Gt => break,
                            _ => {
                                let found = self.current().clone();
                                self.report_expected("`,` or `>`", &found);
                                return None;
                            }
                        }
                    }
                    let close = self.current().clone();
                    if close.kind != TokenKind::Gt {
                        self.report_expected("`>`", &close);
                        return None;
                    }
                    self.bump();
                    let span = token.span.to(close.span);
                    return Some(Type::new(TypeKind::Named { name, args }, span));
                }
                Some(Type::new(TypeKind::Named { name, args }, token.span))
            }
            // `()` is Unit; `(A, B)` is a tuple of two or more.
            TokenKind::LParen => {
                self.bump();
                if self.kind() == &TokenKind::RParen {
                    let close = self.bump();
                    let span = token.span.to(close.span);
                    return Some(Type::new(TypeKind::Unit, span));
                }
                let mut elements = vec![self.parse_type()?];
                while self.eat(&TokenKind::Comma).is_some() {
                    if self.kind() == &TokenKind::RParen {
                        break;
                    }
                    elements.push(self.parse_type()?);
                }
                let close = self.current().clone();
                if close.kind != TokenKind::RParen {
                    self.report_expected("`)`", &close);
                    return None;
                }
                self.bump();
                let span = token.span.to(close.span);
                if elements.len() == 1 {
                    let inner = elements.remove(0);
                    return Some(Type::new(TypeKind::Tuple(vec![inner]), span));
                }
                Some(Type::new(TypeKind::Tuple(elements), span))
            }
            TokenKind::Kw(Keyword::Fn) => {
                self.bump();
                if self.kind() != &TokenKind::LParen {
                    let found = self.current().clone();
                    self.report_expected("`(`", &found);
                    return None;
                }
                self.bump();
                let mut params = Vec::new();
                while self.kind() != &TokenKind::RParen {
                    params.push(self.parse_type()?);
                    if self.kind() == &TokenKind::Comma {
                        self.bump();
                    } else {
                        break;
                    }
                }
                let close = self.current().clone();
                if close.kind != TokenKind::RParen {
                    self.report_expected("`)`", &close);
                    return None;
                }
                self.bump();
                if self.kind() != &TokenKind::Arrow {
                    let found = self.current().clone();
                    self.report_expected("`->`", &found);
                    return None;
                }
                self.bump();
                let ret = self.parse_type()?;
                let span = token.span.to(ret.span);
                Some(Type::new(
                    TypeKind::Fn {
                        params,
                        ret: Box::new(ret),
                    },
                    span,
                ))
            }
            _ => {
                self.report_expected("type", &token);
                None
            }
        }
    }
}
