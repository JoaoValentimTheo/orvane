//! Item parsing: `program`, `fn`, `data`, `enum`, `use` (SPEC §5.2).
//!
//! `intent`, `how`, `test` and `use py` are out of the 0.1.0-alpha scope
//! (ADR 0012) and are reported with `E0105` rather than parsed.

use crate::ast::{
    DataDecl, EnumDecl, FieldDecl, FnDecl, Item, ItemKind, Param, Program, UseDecl, VariantDecl,
};
use crate::lexer::{Keyword, TokenKind};
use crate::span::Span;

use super::{Parser, describe};

impl Parser<'_> {
    /// Parses `program = { NEWLINE | item }` (SPEC §5.2).
    ///
    /// Item-level errors recover by skipping to the next item keyword, so a
    /// broken declaration does not hide the rest of the file.
    pub(crate) fn program(&mut self) -> Option<Program> {
        let start = self.current().span;
        let mut items: Vec<Item> = Vec::new();

        loop {
            self.skip_newlines();
            if self.at_eof() {
                let end = self.current().span;
                let mut program = Program {
                    items,
                    span: start.to(end),
                };
                if program.items.is_empty() {
                    program.span = start;
                }
                return Some(program);
            }

            match self.item() {
                Some(item) => items.push(item),
                None => self.synchronize_item(),
            }
        }
    }

    /// Skips a declaration that was refused with `E0105`.
    ///
    /// Consumes its braces (if any) and stops after the closing one, or at the
    /// end of the line for a brace-less form, so the refused body is not parsed
    /// and reported a second time.
    fn skip_refused_item(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.kind() {
                TokenKind::Eof => return,
                TokenKind::LBrace => {
                    depth += 1;
                    self.bump();
                }
                TokenKind::RBrace => {
                    self.bump();
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return;
                    }
                }
                TokenKind::Newline | TokenKind::Semicolon if depth == 0 => {
                    self.bump();
                    return;
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    /// Skips to the next plausible item start, always making progress.
    fn synchronize_item(&mut self) {
        let start = self.cursor_index();
        loop {
            match self.kind() {
                TokenKind::Eof => return,
                TokenKind::Kw(
                    Keyword::Fn
                    | Keyword::Data
                    | Keyword::Enum
                    | Keyword::Use
                    | Keyword::Intent
                    | Keyword::How
                    | Keyword::Test
                    | Keyword::Pub,
                ) => {
                    if self.cursor_index() != start {
                        return;
                    }
                    // Already at an item keyword that failed to parse: consume
                    // it so the loop cannot spin.
                    self.bump();
                    return;
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    /// Parses one item.
    fn item(&mut self) -> Option<Item> {
        let public = if self.kind() == &TokenKind::Kw(Keyword::Pub) {
            self.bump();
            true
        } else {
            false
        };

        let token = self.current().clone();
        match token.kind {
            TokenKind::Kw(Keyword::Fn) => self.fn_decl(public).map(|decl| {
                let span = decl.span();
                Item::new(ItemKind::Fn(decl), span)
            }),
            TokenKind::Kw(Keyword::Data) => {
                if public {
                    self.report(
                        "E0105",
                        "`pub data` is not supported in 0.1.0-alpha",
                        token.span,
                        Some("module visibility lands in a later milestone (ADR 0012)".to_owned()),
                    );
                    self.skip_refused_item();
                    return None;
                }
                self.data_decl().map(|decl| {
                    let span = decl.span;
                    Item::new(ItemKind::Data(decl), span)
                })
            }
            TokenKind::Kw(Keyword::Enum) => {
                if public {
                    self.report(
                        "E0105",
                        "`pub enum` is not supported in 0.1.0-alpha",
                        token.span,
                        Some("module visibility lands in a later milestone (ADR 0012)".to_owned()),
                    );
                    self.skip_refused_item();
                    return None;
                }
                self.enum_decl().map(|decl| {
                    let span = decl.span;
                    Item::new(ItemKind::Enum(decl), span)
                })
            }
            TokenKind::Kw(Keyword::Use) => {
                if public {
                    self.report_expected("item", &token);
                    return None;
                }
                self.use_decl().map(|decl| {
                    let span = decl.span;
                    Item::new(ItemKind::Use(decl), span)
                })
            }
            TokenKind::Kw(Keyword::Intent | Keyword::How | Keyword::Test) => {
                let name = match token.kind {
                    TokenKind::Kw(keyword) => keyword.as_str(),
                    _ => "?",
                };
                self.report(
                    "E0105",
                    format!("`{name}` is not supported in 0.1.0-alpha"),
                    token.span,
                    Some(
                        "intents, strategies and embedded tests land after the alpha (ADR 0012)"
                            .to_owned(),
                    ),
                );
                // Skip the whole declaration so its body is not re-parsed as a
                // second, meaningless diagnostic (one cause, one message).
                self.skip_refused_item();
                None
            }
            // A token that is well-formed but cannot begin an item is
            // "unexpected" (E0101) rather than "expected something else"
            // (E0102): the author did not forget a token, they wrote one that
            // does not belong here.
            TokenKind::Eof => {
                self.report_expected("item", &token);
                None
            }
            _ => {
                self.report(
                    "E0101",
                    format!("unexpected {} at item level", describe(&token.kind)),
                    token.span,
                    None,
                );
                None
            }
        }
    }

    /// Parses `fn NAME(params) [-> Type] block`.
    fn fn_decl(&mut self, public: bool) -> Option<FnDecl> {
        let start = self.bump().span; // the `fn`

        let name_token = self.current().clone();
        let TokenKind::Ident(name) = name_token.kind.clone() else {
            self.report_expected("function name", &name_token);
            return None;
        };
        self.bump();

        if self.kind() != &TokenKind::LParen {
            let found = self.current().clone();
            self.report_expected("`(`", &found);
            return None;
        }
        self.bump();

        let mut params: Vec<Param> = Vec::new();
        loop {
            if self.kind() == &TokenKind::RParen {
                break;
            }
            params.push(self.param()?);
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

        let ret = if self.kind() == &TokenKind::Arrow {
            self.bump();
            Some(self.parse_type()?)
        } else {
            None
        };

        let signature_span = match &ret {
            Some(ty) => start.to(ty.span),
            None => start.to(close.span),
        };

        self.skip_newlines();
        let body = self.block()?;
        Some(FnDecl {
            name,
            params,
            ret,
            body,
            public,
            signature_span,
        })
    }

    /// Parses `IDENT ":" Type [ "=" expr ]`.
    fn param(&mut self) -> Option<Param> {
        let name_token = self.current().clone();
        let TokenKind::Ident(name) = name_token.kind.clone() else {
            self.report_expected("parameter name", &name_token);
            return None;
        };
        self.bump();

        if self.kind() != &TokenKind::Colon {
            let found = self.current().clone();
            self.report_expected("`:`", &found);
            return None;
        }
        self.bump();

        let ty = self.parse_type()?;

        let default = if self.kind() == &TokenKind::Eq {
            self.bump();
            Some(self.expr()?)
        } else {
            None
        };
        let span = match &default {
            Some(value) => name_token.span.to(value.span),
            None => name_token.span.to(ty.span),
        };
        Some(Param {
            name,
            ty,
            default,
            span,
        })
    }

    /// Parses `data NAME { field, ... }`.
    fn data_decl(&mut self) -> Option<DataDecl> {
        let start = self.bump().span; // the `data`

        let name_token = self.current().clone();
        let TokenKind::Ident(name) = name_token.kind.clone() else {
            self.report_expected("type name", &name_token);
            return None;
        };
        self.bump();

        if self.kind() != &TokenKind::LBrace {
            let found = self.current().clone();
            self.report_expected("`{`", &found);
            return None;
        }
        self.bump();

        let mut fields: Vec<FieldDecl> = Vec::new();
        let mut end = name_token.span;
        loop {
            self.skip_newlines();
            if self.kind() == &TokenKind::RBrace {
                end = self.bump().span;
                break;
            }
            if self.at_eof() {
                let found = self.current().clone();
                self.report_expected("`}`", &found);
                return None;
            }
            fields.push(self.field_decl()?);
            if self.kind() == &TokenKind::Comma {
                self.bump();
            } else {
                self.skip_newlines();
                if self.kind() == &TokenKind::RBrace {
                    self.bump();
                    break;
                }
                let found = self.current().clone();
                self.report_expected("`,` or `}`", &found);
                return None;
            }
        }
        Some(DataDecl {
            name,
            fields,
            public: false,
            span: start.to(end),
        })
    }

    /// Parses `IDENT ":" Type [ "=" expr ]` inside a `data`.
    fn field_decl(&mut self) -> Option<FieldDecl> {
        let name_token = self.current().clone();
        let TokenKind::Ident(name) = name_token.kind.clone() else {
            self.report_expected("field name", &name_token);
            return None;
        };
        self.bump();

        if self.kind() != &TokenKind::Colon {
            let found = self.current().clone();
            self.report_expected("`:`", &found);
            return None;
        }
        self.bump();

        let ty = self.parse_type()?;
        let default = if self.kind() == &TokenKind::Eq {
            self.bump();
            Some(self.expr()?)
        } else {
            None
        };
        let span = match &default {
            Some(value) => name_token.span.to(value.span),
            None => name_token.span.to(ty.span),
        };
        Some(FieldDecl {
            name,
            ty,
            default,
            span,
        })
    }

    /// Parses `enum NAME { Variant, ... }`.
    fn enum_decl(&mut self) -> Option<EnumDecl> {
        let start = self.bump().span; // the `enum`

        let name_token = self.current().clone();
        let TokenKind::Ident(name) = name_token.kind.clone() else {
            self.report_expected("type name", &name_token);
            return None;
        };
        self.bump();

        if self.kind() != &TokenKind::LBrace {
            let found = self.current().clone();
            self.report_expected("`{`", &found);
            return None;
        }
        self.bump();

        let mut variants: Vec<VariantDecl> = Vec::new();
        let mut end = name_token.span;
        loop {
            self.skip_newlines();
            if self.kind() == &TokenKind::RBrace {
                end = self.bump().span;
                break;
            }
            if self.at_eof() {
                let found = self.current().clone();
                self.report_expected("`}`", &found);
                return None;
            }
            variants.push(self.variant_decl()?);
            if self.kind() == &TokenKind::Comma {
                self.bump();
            } else {
                self.skip_newlines();
                if self.kind() == &TokenKind::RBrace {
                    self.bump();
                    break;
                }
                let found = self.current().clone();
                self.report_expected("`,` or `}`", &found);
                return None;
            }
        }
        Some(EnumDecl {
            name,
            variants,
            public: false,
            span: start.to(end),
        })
    }

    /// Parses `IDENT [ "(" Type { "," Type } ")" ]`.
    fn variant_decl(&mut self) -> Option<VariantDecl> {
        let name_token = self.current().clone();
        let TokenKind::Ident(name) = name_token.kind.clone() else {
            self.report_expected("variant name", &name_token);
            return None;
        };
        self.bump();

        let mut payload = Vec::new();
        let mut end = name_token.span;
        if self.kind() == &TokenKind::LParen {
            self.bump();
            loop {
                if self.kind() == &TokenKind::RParen {
                    break;
                }
                payload.push(self.parse_type()?);
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
            end = close.span;
        }
        Some(VariantDecl {
            name,
            payload,
            span: name_token.span.to(end),
        })
    }

    /// Parses `use path [as IDENT]` (file modules only).
    fn use_decl(&mut self) -> Option<UseDecl> {
        let start = self.bump().span; // the `use`

        // `use py ...` is out of the alpha (ADR 0012). `py` is contextual
        // (§5.1), so it arrives as an identifier; naming it here gives a clear
        // message instead of a confusing "expected path".
        if let TokenKind::Ident(name) = self.kind().clone() {
            if name == "py" {
                let span = self.current().span;
                self.report(
                    "E0105",
                    "`use py` is not supported in 0.1.0-alpha",
                    span,
                    Some("Python interop lands after the alpha (ADR 0012)".to_owned()),
                );
                self.skip_refused_item();
                return None;
            }
        }

        let mut path: Vec<String> = Vec::new();
        // The span of the last token consumed for this declaration.
        let mut end;
        loop {
            let token = self.current().clone();
            let TokenKind::Ident(segment) = token.kind.clone() else {
                self.report_expected("module path", &token);
                return None;
            };
            self.bump();
            end = token.span;
            path.push(segment);
            if self.kind() == &TokenKind::Dot {
                self.bump();
            } else {
                break;
            }
        }

        let alias = if self.kind() == &TokenKind::Kw(Keyword::As) {
            self.bump();
            let token = self.current().clone();
            let TokenKind::Ident(alias) = token.kind.clone() else {
                self.report_expected("alias", &token);
                return None;
            };
            self.bump();
            end = token.span;
            Some(alias)
        } else {
            None
        };
        Some(UseDecl {
            path,
            alias,
            span: start.to(end),
        })
    }
}

impl FnDecl {
    /// The span of the whole declaration, from `fn` through the body.
    fn span(&self) -> Span {
        self.signature_span.to(self.body.span)
    }
}
