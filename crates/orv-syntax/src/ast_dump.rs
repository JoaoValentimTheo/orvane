//! A stable textual dump of the AST, for `orv ast` and golden tests.
//!
//! The format is fixed so goldens can rely on it (ADR 0014): one node per line,
//! indented by two spaces per level, with the node kind and the parts that
//! identify it. Spans are deliberately omitted to keep diffs about structure.

use std::fmt::Write as _;

use crate::ast::{
    Arg, Block, DataDecl, EnumDecl, Expr, ExprKind, FieldDecl, FnDecl, Item, ItemKind, Literal,
    MatchArm, Param, Pattern, PatternKind, Program, Stmt, StmtKind, StrSegment, Type, TypeKind,
    VariantDecl,
};

/// Renders a whole program.
pub fn dump_program(program: &Program) -> String {
    let mut out = String::new();
    for item in &program.items {
        dump_item(&mut out, item, 0);
    }
    out
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn line(out: &mut String, depth: usize, text: &str) {
    indent(out, depth);
    let _ = writeln!(out, "{text}");
}

fn dump_item(out: &mut String, item: &Item, depth: usize) {
    match &item.kind {
        ItemKind::Fn(decl) => dump_fn(out, decl, depth),
        ItemKind::Data(decl) => dump_data(out, decl, depth),
        ItemKind::Enum(decl) => dump_enum(out, decl, depth),
        ItemKind::Use(decl) => {
            line(out, depth, &format!("Use {}", decl.path.join(".")));
            if let Some(alias) = &decl.alias {
                line(out, depth + 1, &format!("alias {alias}"));
            }
        }
    }
}

fn dump_fn(out: &mut String, decl: &FnDecl, depth: usize) {
    let visibility = if decl.public { "pub " } else { "" };
    line(out, depth, &format!("{visibility}Fn {}", decl.name));
    for param in &decl.params {
        dump_param(out, param, depth + 1);
    }
    if let Some(ret) = &decl.ret {
        indent(out, depth + 1);
        let _ = writeln!(out, "ret {}", type_text(ret));
    }
    dump_block(out, &decl.body, depth + 1);
}

fn dump_param(out: &mut String, param: &Param, depth: usize) {
    line(
        out,
        depth,
        &format!("Param {}: {}", param.name, type_text(&param.ty)),
    );
    if let Some(default) = &param.default {
        line(out, depth + 1, "default");
        dump_expr(out, default, depth + 2);
    }
}

fn dump_data(out: &mut String, decl: &DataDecl, depth: usize) {
    line(out, depth, &format!("Data {}", decl.name));
    for field in &decl.fields {
        dump_field(out, field, depth + 1);
    }
}

fn dump_field(out: &mut String, field: &FieldDecl, depth: usize) {
    line(
        out,
        depth,
        &format!("Field {}: {}", field.name, type_text(&field.ty)),
    );
    if let Some(default) = &field.default {
        line(out, depth + 1, "default");
        dump_expr(out, default, depth + 2);
    }
}

fn dump_enum(out: &mut String, decl: &EnumDecl, depth: usize) {
    line(out, depth, &format!("Enum {}", decl.name));
    for variant in &decl.variants {
        dump_variant(out, variant, depth + 1);
    }
}

fn dump_variant(out: &mut String, variant: &VariantDecl, depth: usize) {
    let payload: Vec<String> = variant.payload.iter().map(type_text).collect();
    if payload.is_empty() {
        line(out, depth, &format!("Variant {}", variant.name));
    } else {
        line(
            out,
            depth,
            &format!("Variant {}({})", variant.name, payload.join(", ")),
        );
    }
}

fn dump_block(out: &mut String, block: &Block, depth: usize) {
    line(out, depth, "Block");
    for statement in &block.statements {
        dump_stmt(out, statement, depth + 1);
    }
}

fn dump_stmt(out: &mut String, statement: &Stmt, depth: usize) {
    match &statement.kind {
        StmtKind::Let {
            pattern,
            mutable,
            ty,
            value,
        } => {
            let mutability = if *mutable { "mut " } else { "" };
            let annotation = match ty {
                Some(ty) => format!(": {}", type_text(ty)),
                None => String::new(),
            };
            line(
                out,
                depth,
                &format!("Let {mutability}{}{annotation} =", pattern_text(pattern)),
            );
            dump_expr(out, value, depth + 1);
        }
        StmtKind::Assign { target, op, value } => {
            line(out, depth, &format!("Assign {}", op.as_str()));
            dump_expr(out, target, depth + 1);
            dump_expr(out, value, depth + 1);
        }
        StmtKind::While { condition, body } => {
            line(out, depth, "While");
            dump_expr(out, condition, depth + 1);
            dump_block(out, body, depth + 1);
        }
        StmtKind::For {
            pattern,
            iterable,
            body,
        } => {
            line(out, depth, &format!("For {}", pattern_text(pattern)));
            dump_expr(out, iterable, depth + 1);
            dump_block(out, body, depth + 1);
        }
        StmtKind::Return(value) => {
            line(out, depth, "Return");
            if let Some(value) = value {
                dump_expr(out, value, depth + 1);
            }
        }
        StmtKind::Break => line(out, depth, "Break"),
        StmtKind::Continue => line(out, depth, "Continue"),
        StmtKind::Fail(value) => {
            line(out, depth, "Fail");
            dump_expr(out, value, depth + 1);
        }
        StmtKind::Expr(expr) => dump_expr(out, expr, depth),
    }
}

fn dump_expr(out: &mut String, expr: &Expr, depth: usize) {
    match &expr.kind {
        ExprKind::Literal(literal) => {
            line(out, depth, &format!("Literal {}", literal_text(literal)))
        }
        ExprKind::Ident(name) => line(out, depth, &format!("Ident {name}")),
        ExprKind::Paren(inner) => {
            line(out, depth, "Paren");
            dump_expr(out, inner, depth + 1);
        }
        ExprKind::Block(block) => dump_block(out, block, depth),
        ExprKind::Field { receiver, name } => {
            line(out, depth, &format!("Field {name}"));
            dump_expr(out, receiver, depth + 1);
        }
        ExprKind::OptionalField { receiver, name } => {
            line(out, depth, &format!("OptionalField {name}"));
            dump_expr(out, receiver, depth + 1);
        }
        ExprKind::Call { callee, args } => {
            line(out, depth, "Call");
            dump_expr(out, callee, depth + 1);
            for arg in args {
                dump_arg(out, arg, depth + 1);
            }
        }
        ExprKind::Index { receiver, index } => {
            line(out, depth, "Index");
            dump_expr(out, receiver, depth + 1);
            dump_expr(out, index, depth + 1);
        }
        ExprKind::Unary { op, operand } => {
            line(out, depth, &format!("Unary {}", op.as_str()));
            dump_expr(out, operand, depth + 1);
        }
        ExprKind::Binary { op, left, right } => {
            line(out, depth, &format!("Binary {}", op.as_str()));
            dump_expr(out, left, depth + 1);
            dump_expr(out, right, depth + 1);
        }
        ExprKind::Cast { expr, ty } => {
            line(out, depth, &format!("Cast {}", type_text(ty)));
            dump_expr(out, expr, depth + 1);
        }
        ExprKind::Tuple(elements) => {
            line(out, depth, "Tuple");
            for element in elements {
                dump_expr(out, element, depth + 1);
            }
        }
        ExprKind::List(elements) => {
            line(out, depth, "List");
            for element in elements {
                dump_expr(out, element, depth + 1);
            }
        }
        ExprKind::Map(entries) => {
            line(out, depth, "Map");
            for (key, value) in entries {
                line(out, depth + 1, "entry");
                dump_expr(out, key, depth + 2);
                dump_expr(out, value, depth + 2);
            }
        }
        ExprKind::If {
            condition,
            then_block,
            else_branch,
        } => {
            line(out, depth, "If");
            dump_expr(out, condition, depth + 1);
            dump_block(out, then_block, depth + 1);
            if let Some(branch) = else_branch {
                line(out, depth + 1, "Else");
                dump_expr(out, branch, depth + 2);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            line(out, depth, "Match");
            dump_expr(out, scrutinee, depth + 1);
            for arm in arms {
                dump_arm(out, arm, depth + 1);
            }
        }
        ExprKind::Try(inner) => {
            line(out, depth, "Try");
            dump_expr(out, inner, depth + 1);
        }
        ExprKind::Fail(inner) => {
            line(out, depth, "Fail");
            dump_expr(out, inner, depth + 1);
        }
        ExprKind::Lambda { params, body } => {
            line(out, depth, &format!("Lambda({})", params.join(", ")));
            dump_expr(out, body, depth + 1);
        }
    }
}

fn dump_arg(out: &mut String, arg: &Arg, depth: usize) {
    match &arg.name {
        Some(name) => line(out, depth, &format!("arg {name}:")),
        None => line(out, depth, "arg"),
    }
    dump_expr(out, &arg.value, depth + 1);
}

fn dump_arm(out: &mut String, arm: &MatchArm, depth: usize) {
    line(out, depth, &format!("Arm {}", pattern_text(&arm.pattern)));
    if let Some(guard) = &arm.guard {
        line(out, depth + 1, "guard");
        dump_expr(out, guard, depth + 2);
    }
    dump_expr(out, &arm.body, depth + 1);
}

fn pattern_text(pattern: &Pattern) -> String {
    match &pattern.kind {
        PatternKind::Wildcard => "_".to_owned(),
        PatternKind::Literal(literal) => literal_text(literal),
        PatternKind::Bind(name) => name.clone(),
        PatternKind::Variant { name, fields } => {
            if fields.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = fields.iter().map(pattern_text).collect();
                format!("{name}({})", inner.join(", "))
            }
        }
    }
}

fn literal_text(literal: &Literal) -> String {
    match literal {
        Literal::Int(value) => value.to_string(),
        Literal::Float(value) => crate::dump::kind_text(&crate::TokenKind::Float(*value)),
        Literal::Str(parts) => {
            let rendered: Vec<String> = parts
                .iter()
                .map(|part| match part {
                    StrSegment::Lit(text) => format!("\"{text}\""),
                    StrSegment::Expr { src, .. } | StrSegment::Raw(src) => format!("{{{src}}}"),
                })
                .collect();
            rendered.join(" ")
        }
        Literal::Bool(value) => value.to_string(),
        Literal::None => "none".to_owned(),
    }
}

/// The stable spelling of a type.
pub fn type_text(ty: &Type) -> String {
    match &ty.kind {
        TypeKind::Named { name, args } => {
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(type_text).collect();
                format!("{name}<{}>", inner.join(", "))
            }
        }
        TypeKind::Optional(inner) => format!("{}?", type_text(inner)),
        TypeKind::Unit => "()".to_owned(),
        TypeKind::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(type_text).collect();
            format!("({})", inner.join(", "))
        }
        TypeKind::Fn { params, ret } => {
            let inner: Vec<String> = params.iter().map(type_text).collect();
            format!("fn({}) -> {}", inner.join(", "), type_text(ret))
        }
        TypeKind::Py => "Py".to_owned(),
    }
}
