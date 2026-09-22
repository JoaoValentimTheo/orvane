//! Name resolution and type checking (SPEC §5.3, §5.2).
//!
//! Scope of the alpha (ADR 0012): primitives, `List`, `Map`, tuples, optionals,
//! functions, closures, `data`/`enum`, `as`. Generics on user types, modules and
//! Python interop are out.
//!
//! Design notes
//! ------------
//! * **One cause, one diagnostic.** A failed sub-expression becomes
//!   [`Ty::Unknown`], which is compatible with everything, so `1 + f(unknown)`
//!   reports the undefined name once instead of a type error per parent.
//! * **No panics.** Every accessor is checked; deeply recursive input is bounded
//!   by the parser's `E0104`, and the checker itself has no recursion guard of
//!   its own beyond the AST's depth (ADR 0013 keeps that bounded).
//! * Determinism: scopes are ordered vectors, so diagnostics come out in source
//!   order.

use std::collections::HashMap;

use orv_syntax::{
    Arg, BinaryOp, Block, DataDecl, EnumDecl, Expr, ExprKind, FnDecl, ItemKind, Literal, Pattern,
    PatternKind, Program, Stmt, StmtKind, UnaryOp,
};

use crate::Diagnostic;
use crate::scopes::{Scopes, Symbol};
use crate::ty::{Ty, resolve_type};

/// The result of checking a program.
#[derive(Debug)]
pub struct CheckResult {
    /// Diagnostics in source order.
    pub diagnostics: Vec<Diagnostic>,
    /// The type of each top-level function, by name.
    pub functions: HashMap<String, Ty>,
}

impl CheckResult {
    /// Whether any error was reported.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity.is_error())
    }
}

/// A declared user type.
#[derive(Clone, PartialEq, Debug)]
pub enum UserType {
    /// A `data` type with named fields, in declaration order.
    Data { fields: Vec<(String, Ty)> },
    /// An `enum` with its variants, in declaration order.
    Enum { variants: Vec<EnumVariant> },
}

/// One variant of an enum.
#[derive(Clone, PartialEq, Debug)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Vec<Ty>,
}

/// Checks a whole program: names first, then types.
///
/// Two passes are needed because a function may call one declared later, and a
/// `data` type may be mentioned before its declaration.
pub struct Checker {
    diagnostics: Vec<Diagnostic>,
    scopes: Scopes,
    /// Signatures of top-level functions, filled by the first pass.
    functions: HashMap<String, Ty>,
    /// User-declared `data`/`enum` types.
    user_types: HashMap<String, UserType>,
    /// Where each declared name was defined, for `E0202`.
    declarations: Vec<(String, orv_syntax::Span)>,
    /// The function whose body is being checked, for `return` typing.
    current_return: Option<Ty>,
    /// The type expected of the expression currently being checked.
    ///
    /// Only used by lambdas (§5.3: a lambda infers from the expected type); it
    /// is cleared as soon as the expression is done.
    expected: Option<Ty>,
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

impl Checker {
    /// Creates an empty checker.
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            scopes: Scopes::new(),
            functions: HashMap::new(),
            user_types: HashMap::new(),
            declarations: Vec::new(),
            current_return: None,
            expected: None,
        }
    }

    /// Runs both passes over `program`.
    pub fn check(mut self, program: &Program) -> CheckResult {
        self.install_prelude();
        self.collect_user_types(program);
        self.collect_signatures(program);
        self.check_items(program);
        CheckResult {
            diagnostics: self.diagnostics,
            functions: self.functions,
        }
    }

    /// Declares the implicit prelude (SPEC §5.6).
    ///
    /// The signatures are the ones the runtime provides; declaring them here
    /// means a call to `print` is checked like any other function, and a
    /// *shadowing* user definition simply replaces the binding in the global
    /// scope (which `Scopes::declare` refuses, so the prelude is registered
    /// through the same duplicate check as anything else).
    fn install_prelude(&mut self) {
        let prelude: [(&str, Vec<Ty>, Ty); 9] = [
            // `print(value)` — accepts anything, returns `()`.
            ("print", vec![Ty::Unknown], Ty::Unit),
            ("len", vec![Ty::Unknown], Ty::Int),
            ("str", vec![Ty::Unknown], Ty::Str),
            ("int", vec![Ty::Unknown], Ty::Int),
            ("float", vec![Ty::Unknown], Ty::Float),
            ("assert", vec![Ty::Bool], Ty::Unit),
            // `range` is overloaded in the runtime; the alpha models the
            // `range(Int, Int)` form and the `..` operator produces a list.
            ("range", vec![Ty::Int, Ty::Int], Ty::List(Box::new(Ty::Int))),
            // `Ok`/`Err` construct results; the alpha only needs the shape.
            ("Ok", vec![Ty::Unknown], Ty::Result(Box::new(Ty::Unknown))),
            ("Err", vec![Ty::Unknown], Ty::Result(Box::new(Ty::Unknown))),
        ];
        // `Failure` is a builtin type, not a function, so it is registered as a
        // user-visible name with a single `Str` field.
        self.user_types.insert(
            "Failure".to_owned(),
            UserType::Data {
                fields: vec![
                    ("kind".to_owned(), Ty::Str),
                    ("message".to_owned(), Ty::Str),
                    ("trace".to_owned(), Ty::List(Box::new(Ty::Str))),
                ],
            },
        );
        for (name, params, ret) in prelude {
            let signature = Ty::Fn {
                params,
                ret: Box::new(ret),
            };
            self.functions.insert(name.to_owned(), signature.clone());
            self.scopes.declare(Symbol {
                name: name.to_owned(),
                ty: signature,
                mutable: false,
            });
        }
    }

    // --- Pass 1: declarations ----------------------------------------------

    /// Records every `data`/`enum` and the shape of its fields.
    fn collect_user_types(&mut self, program: &Program) {
        for item in &program.items {
            match &item.kind {
                ItemKind::Data(decl) => {
                    if !self.declare_name(&decl.name, item.span) {
                        continue;
                    }
                    let fields = self.data_fields(decl);
                    self.user_types
                        .insert(decl.name.clone(), UserType::Data { fields });
                }
                ItemKind::Enum(decl) => {
                    if !self.declare_name(&decl.name, item.span) {
                        continue;
                    }
                    let variants = self.enum_variants(decl);
                    self.user_types
                        .insert(decl.name.clone(), UserType::Enum { variants });
                }
                ItemKind::Fn(_) | ItemKind::Use(_) => {}
            }
        }
    }

    /// Field names and types of a `data` declaration.
    fn data_fields(&mut self, decl: &DataDecl) -> Vec<(String, Ty)> {
        let mut fields = Vec::new();
        for field in &decl.fields {
            let ty = match resolve_type(&field.ty) {
                Ok(ty) => self.resolve_user_type(ty, field.ty.span),
                Err(_) => {
                    self.error(
                        "E0301",
                        format!("invalid type annotation for field `{}`", field.name),
                        field.ty.span,
                    );
                    Ty::Unknown
                }
            };
            if fields.iter().any(|(name, _)| name == &field.name) {
                self.error(
                    "E0202",
                    format!("duplicate field `{}` in `{}`", field.name, decl.name),
                    field.span,
                );
                continue;
            }
            fields.push((field.name.clone(), ty));
        }
        fields
    }

    /// Variants of an `enum` declaration.
    fn enum_variants(&mut self, decl: &EnumDecl) -> Vec<EnumVariant> {
        let mut variants = Vec::new();
        for variant in &decl.variants {
            let mut payload: Vec<Ty> = Vec::new();
            let mut failed = false;
            for ty in &variant.payload {
                match resolve_type(ty) {
                    Ok(resolved) => {
                        let resolved = self.resolve_user_type(resolved, ty.span);
                        payload.push(resolved);
                    }
                    Err(_) => {
                        self.error(
                            "E0301",
                            format!("invalid payload type for variant `{}`", variant.name),
                            variant.span,
                        );
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                continue;
            }
            if variants
                .iter()
                .any(|v: &EnumVariant| v.name == variant.name)
            {
                self.error(
                    "E0202",
                    format!("duplicate variant `{}` in `{}`", variant.name, decl.name),
                    variant.span,
                );
                continue;
            }
            variants.push(EnumVariant {
                name: variant.name.clone(),
                payload,
            });
        }
        variants
    }

    /// Records every function signature so calls can be checked in pass 2.
    fn collect_signatures(&mut self, program: &Program) {
        for item in &program.items {
            let ItemKind::Fn(decl) = &item.kind else {
                continue;
            };
            if !self.declare_name(&decl.name, item.span) {
                continue;
            }
            let signature = self.fn_signature(decl);
            self.functions.insert(decl.name.clone(), signature);
            self.scopes.declare(Symbol {
                name: decl.name.clone(),
                ty: self
                    .functions
                    .get(&decl.name)
                    .cloned()
                    .unwrap_or(Ty::Unknown),
                mutable: false,
            });
        }
    }

    /// The `fn(params) -> ret` type of a declaration.
    fn fn_signature(&mut self, decl: &FnDecl) -> Ty {
        let params: Vec<Ty> = decl
            .params
            .iter()
            .map(|param| match resolve_type(&param.ty) {
                Ok(ty) => self.resolve_user_type(ty, param.ty.span),
                Err(_) => {
                    self.error(
                        "E0301",
                        format!("invalid type annotation for parameter `{}`", param.name),
                        param.ty.span,
                    );
                    Ty::Unknown
                }
            })
            .collect();
        let ret = match &decl.ret {
            Some(annotation) => match resolve_type(annotation) {
                Ok(resolved) => self.resolve_user_type(resolved, annotation.span),
                Err(_) => {
                    self.error("E0301", "invalid return type annotation", annotation.span);
                    Ty::Unknown
                }
            },
            // §5.3: a `fn` without a return annotation returns `()`.
            None => Ty::Unit,
        };
        Ty::Fn {
            params,
            ret: Box::new(ret),
        }
    }

    // --- Pass 2: bodies -----------------------------------------------------

    /// Checks every function body.
    fn check_items(&mut self, program: &Program) {
        for item in &program.items {
            match &item.kind {
                ItemKind::Fn(decl) => self.check_fn(decl),
                ItemKind::Data(decl) => self.check_data_defaults(decl),
                ItemKind::Enum(_) | ItemKind::Use(_) => {}
            }
        }
    }

    /// Checks the default values of `data` fields in the global scope.
    fn check_data_defaults(&mut self, decl: &DataDecl) {
        for field in &decl.fields {
            let Some(default) = &field.default else {
                continue;
            };
            let expected = self
                .user_types
                .get(&decl.name)
                .and_then(|user| match user {
                    UserType::Data { fields } => fields
                        .iter()
                        .find(|(name, _)| name == &field.name)
                        .map(|(_, ty)| ty.clone()),
                    UserType::Enum { .. } => None,
                })
                .unwrap_or(Ty::Unknown);
            let found = self.check_expr(default);
            if !self.assignable(&expected, &found) {
                self.type_mismatch(&expected, &found, default.span);
            }
        }
    }

    /// Checks one function body in a fresh scope.
    fn check_fn(&mut self, decl: &FnDecl) {
        let signature = self
            .functions
            .get(&decl.name)
            .cloned()
            .unwrap_or(Ty::Unknown);
        let ret = match &signature {
            Ty::Fn { ret, .. } => (**ret).clone(),
            _ => Ty::Unit,
        };

        self.scopes.push();
        for (index, param) in decl.params.iter().enumerate() {
            let ty = match &signature {
                Ty::Fn { params, .. } => params.get(index).cloned().unwrap_or(Ty::Unknown),
                _ => Ty::Unknown,
            };
            if !self.scopes.declare(Symbol {
                name: param.name.clone(),
                ty: ty.clone(),
                mutable: false,
            }) {
                self.error(
                    "E0202",
                    format!("duplicate parameter `{}`", param.name),
                    param.span,
                );
            }
            // A default value must match the parameter's type.
            if let Some(default) = &param.default {
                let found = self.check_expr(default);
                if !self.assignable(&ty, &found) {
                    self.type_mismatch(&ty, &found, default.span);
                }
            }
        }

        let previous_return = self.current_return.replace(ret.clone());
        let body = self.check_block(&decl.body);
        self.current_return = previous_return;
        self.scopes.pop();

        // §5.3: a block's value is its last expression. The body must produce
        // the declared return type when it ends in an expression.
        if !ret.is_unknown()
            && !matches!(ret, Ty::Unit)
            && let Some(ty) = body
            && !self.assignable(&ret, &ty)
        {
            let span = decl.body.span;
            self.type_mismatch(&ret, &ty, span);
        }
    }

    /// Checks a block, returning the type of its trailing expression.
    fn check_block(&mut self, block: &Block) -> Option<Ty> {
        self.scopes.push();
        let mut tail = None;
        for statement in &block.statements {
            tail = self.check_stmt(statement);
        }
        self.scopes.pop();
        tail
    }

    /// Checks a statement, returning a type only for an expression statement.
    fn check_stmt(&mut self, statement: &Stmt) -> Option<Ty> {
        match &statement.kind {
            StmtKind::Let {
                pattern,
                mutable,
                ty,
                value,
            } => {
                // The annotation, when present, is the expected type: it is what
                // lets a lambda infer its parameters (§5.3).
                let annotation = ty
                    .as_ref()
                    .map(|annotation| match resolve_type(annotation) {
                        Ok(resolved) => self.resolve_user_type(resolved, annotation.span),
                        Err(_) => {
                            self.error("E0301", "invalid type annotation", annotation.span);
                            Ty::Unknown
                        }
                    });
                let found = self.check_expr_expected(value, annotation.clone());
                let expected = annotation.unwrap_or_else(|| found.clone());
                if !self.assignable(&expected, &found) {
                    self.type_mismatch(&expected, &found, value.span);
                }
                self.declare_pattern(pattern, expected, *mutable);
                None
            }
            StmtKind::Assign {
                target,
                value,
                op: _,
            } => {
                let found = self.check_expr(value);
                let expected = self.check_expr(target);
                self.check_assignable_target(target, expected.clone(), statement.span);
                if !self.assignable(&expected, &found) {
                    self.type_mismatch(&expected, &found, value.span);
                }
                None
            }
            StmtKind::While { condition, body } => {
                let cond = self.check_expr(condition);
                self.require_bool(&cond, condition.span);
                self.check_block(body);
                None
            }
            StmtKind::For {
                pattern,
                iterable,
                body,
            } => {
                let iterable_ty = self.check_expr(iterable);
                let element = self.element_type(&iterable_ty, iterable.span);
                self.scopes.push();
                self.declare_pattern(pattern, element, false);
                let mut tail = None;
                for statement in &body.statements {
                    tail = self.check_stmt(statement);
                }
                self.scopes.pop();
                tail
            }
            StmtKind::Return(value) => {
                let target = self.current_return.clone();
                let found = match value {
                    Some(value) => self.check_expr_expected(value, target),
                    None => Ty::Unit,
                };
                let expected = self.current_return.clone().unwrap_or(Ty::Unit);
                if !expected.is_unknown() && !self.assignable(&expected, &found) {
                    let span = value.as_ref().map(|e| e.span).unwrap_or(statement.span);
                    self.type_mismatch(&expected, &found, span);
                }
                None
            }
            StmtKind::Break | StmtKind::Continue => None,
            StmtKind::Fail(value) => {
                self.check_expr(value);
                None
            }
            StmtKind::Expr(expr) => Some(self.check_expr(expr)),
        }
    }

    /// Declares the names bound by `pattern` with the given type.
    fn declare_pattern(&mut self, pattern: &Pattern, ty: Ty, mutable: bool) {
        match &pattern.kind {
            PatternKind::Wildcard => {}
            PatternKind::Bind(name) => {
                if !self.scopes.declare(Symbol {
                    name: name.clone(),
                    ty,
                    mutable,
                }) {
                    self.error(
                        "E0202",
                        format!("duplicate definition of `{name}`"),
                        pattern.span,
                    );
                }
            }
            PatternKind::Literal(_) | PatternKind::Variant { .. } => {
                // Only `let PATTERN =` uses this path, and `let` takes a bound
                // name (§5.2 `let_stmt`), so these cannot appear here.
            }
        }
    }

    /// Checks that an assignment target is a mutable binding or a place.
    fn check_assignable_target(
        &mut self,
        target: &Expr,
        _ty: Ty,
        statement_span: orv_syntax::Span,
    ) {
        match &target.kind {
            ExprKind::Ident(name) => match self.scopes.lookup(name) {
                Some(symbol) if !symbol.mutable => {
                    self.error(
                        "E0230",
                        format!("cannot assign to immutable `{name}`"),
                        statement_span,
                    );
                }
                Some(_) => {}
                None => {
                    // The undefined name is already reported by `check_expr`.
                }
            },
            // `xs[i] = v` is allowed: the place is mutable even when the
            // binding is not (§5.2 `lvalue`), and the runtime supports it.
            ExprKind::Index { .. } => {}
            // Assigning to a field of a `data` value is **not supported in
            // 0.1.0-alpha** (ADR 0012, ADR 0018): the runtime cannot mutate a
            // `Value::Data` behind its `Rc`. Rejecting here keeps sema and
            // runtime from disagreeing about what is a valid program — the
            // same class of bug this sprint fixed for enum variants.
            ExprKind::Field { name, .. } => {
                self.error(
                    "E0231",
                    format!(
                        "assigning to field `{name}` is not supported in 0.1.0-alpha; rebuild the value instead"
                    ),
                    statement_span,
                );
            }
            ExprKind::OptionalField { name, .. } => {
                self.error(
                    "E0231",
                    format!("cannot assign to optional field `{name}`"),
                    statement_span,
                );
            }
            _ => {
                self.error("E0230", "invalid assignment target", statement_span);
            }
        }
    }

    /// The element type iterated by `for`.
    fn element_type(&mut self, ty: &Ty, span: orv_syntax::Span) -> Ty {
        match ty {
            Ty::List(element) => (**element).clone(),
            Ty::Map(key, _) => (**key).clone(),
            Ty::Unknown => Ty::Unknown,
            other => {
                self.error("E0301", format!("`{other}` is not iterable"), span);
                Ty::Unknown
            }
        }
    }

    // --- Expressions --------------------------------------------------------

    /// Infers the type of an expression, reporting what it can.
    pub fn check_expr(&mut self, expr: &Expr) -> Ty {
        match &expr.kind {
            ExprKind::Literal(literal) => self.literal_type(literal, expr.span),
            ExprKind::Ident(name) => self.ident_type(name, expr.span),
            ExprKind::Paren(inner) => self.check_expr(inner),
            ExprKind::Block(block) => self.check_block(block).unwrap_or(Ty::Unit),
            ExprKind::Field { receiver, name } => {
                let receiver_ty = self.check_expr(receiver);
                self.field_type(&receiver_ty, name, expr.span)
            }
            ExprKind::OptionalField { receiver, name } => {
                let receiver_ty = self.check_expr(receiver);
                let inner = match &receiver_ty {
                    Ty::Optional(inner) => (**inner).clone(),
                    other => other.clone(),
                };
                let field = self.field_type(&inner, name, expr.span);
                Ty::Optional(Box::new(field))
            }
            ExprKind::Call { callee, args } => self.check_call(callee, args, expr.span),
            ExprKind::Index { receiver, index } => {
                let receiver_ty = self.check_expr(receiver);
                let index_ty = self.check_expr(index);
                self.index_type(&receiver_ty, &index_ty, expr.span)
            }
            ExprKind::Unary { op, operand } => {
                let operand_ty = self.check_expr(operand);
                self.unary_type(*op, &operand_ty, expr.span)
            }
            ExprKind::Binary { op, left, right } => {
                let left_ty = self.check_expr(left);
                let right_ty = self.check_expr(right);
                self.binary_type(*op, &left_ty, &right_ty, expr.span)
            }
            ExprKind::Cast { expr: inner, ty } => {
                let source = self.check_expr(inner);
                match resolve_type(ty) {
                    Ok(resolved) => {
                        let target = self.resolve_user_type(resolved, ty.span);
                        // §5.3: only `Int as Float` and `Py as T` are valid
                        // implicitly; every other cast is `E0311`.
                        if !self.cast_allowed(&source, &target) {
                            self.error(
                                "E0311",
                                format!("cannot cast `{source}` to `{target}`"),
                                expr.span,
                            );
                        }
                        target
                    }
                    Err(_) => {
                        self.error("E0311", "invalid cast target type", ty.span);
                        Ty::Unknown
                    }
                }
            }
            ExprKind::Tuple(elements) => {
                Ty::Tuple(elements.iter().map(|e| self.check_expr(e)).collect())
            }
            ExprKind::List(elements) => {
                let mut element_ty: Option<Ty> = None;
                for element in elements {
                    let found = self.check_expr(element);
                    element_ty = Some(match element_ty {
                        None => found,
                        Some(previous) => self.unify(&previous, &found, element.span),
                    });
                }
                Ty::List(Box::new(element_ty.unwrap_or(Ty::Unknown)))
            }
            ExprKind::Map(entries) => {
                let mut key_ty: Option<Ty> = None;
                let mut value_ty: Option<Ty> = None;
                for (key, value) in entries {
                    let found_key = self.check_expr(key);
                    // §5.3: `Map<K, V>` with `K ∈ {Int, Str, Bool}`. Any other
                    // key type has no runtime representation (`MapKey`), so the
                    // sema must reject it here instead of letting the runtime
                    // fail with R0010 (sema↔runtime divergence).
                    if !self.is_valid_map_key(&found_key) {
                        self.error(
                            "E0301",
                            format!("map keys must be `Int`, `Str` or `Bool`, found `{found_key}`"),
                            key.span,
                        );
                    }
                    let found_value = self.check_expr(value);
                    key_ty = Some(match key_ty {
                        None => found_key,
                        Some(previous) => self.unify(&previous, &found_key, key.span),
                    });
                    value_ty = Some(match value_ty {
                        None => found_value,
                        Some(previous) => self.unify(&previous, &found_value, value.span),
                    });
                }
                Ty::Map(
                    Box::new(key_ty.unwrap_or(Ty::Unknown)),
                    Box::new(value_ty.unwrap_or(Ty::Unknown)),
                )
            }
            ExprKind::If {
                condition,
                then_block,
                else_branch,
            } => {
                let cond = self.check_expr(condition);
                self.require_bool(&cond, condition.span);
                let then_ty = self.check_block(then_block).unwrap_or(Ty::Unit);
                match else_branch {
                    Some(branch) => {
                        let else_ty = self.check_expr(branch);
                        self.unify(&then_ty, &else_ty, branch.span)
                    }
                    // Without `else`, an `if` used as a value is `()`.
                    None => Ty::Unit,
                }
            }
            ExprKind::Match { scrutinee, arms } => {
                let scrutinee_ty = self.check_expr(scrutinee);
                let mut result: Option<Ty> = None;
                for arm in arms {
                    self.check_pattern(&arm.pattern, &scrutinee_ty);
                    if let Some(guard) = &arm.guard {
                        let guard_ty = self.check_expr(guard);
                        self.require_bool(&guard_ty, guard.span);
                    }
                    let arm_ty = self.check_expr(&arm.body);
                    result = Some(match result {
                        None => arm_ty,
                        Some(previous) => self.unify(&previous, &arm_ty, arm.body.span),
                    });
                }
                result.unwrap_or(Ty::Unknown)
            }
            ExprKind::Try(inner) => {
                let inner_ty = self.check_expr(inner);
                Ty::Result(Box::new(inner_ty))
            }
            ExprKind::Fail(_) => {
                self.error("E0407", "`fail` is not supported in 0.1.0-alpha", expr.span);
                Ty::Unknown
            }
            ExprKind::Lambda { params, body } => {
                self.check_lambda(params, body, expr.span, self.expected.clone())
            }
        }
    }

    /// Checks a lambda against the type expected of it (§5.3).
    ///
    /// "Lambda infere parâmetros do tipo esperado; se não houver contexto →
    /// E0310." The expected type is set by [`Self::check_expr_expected`] before
    /// the lambda is reached, so `let f: fn(Int) -> Int = x => x` binds `x: Int`
    /// and a bare `let f = x => x` reports `E0310`.
    fn check_lambda(
        &mut self,
        params: &[String],
        body: &Expr,
        span: orv_syntax::Span,
        expected: Option<Ty>,
    ) -> Ty {
        let (expected_params, expected_ret) = match expected {
            Some(Ty::Fn { params, ret }) => (params, ret),
            // Either there is no context, or the context is not a function
            // type (`let f: Int = x => x`), which is the same failure to infer.
            _ => {
                self.error(
                    "E0310",
                    "cannot infer the type of this lambda without an expected function type",
                    span,
                );
                return Ty::Unknown;
            }
        };

        if expected_params.len() != params.len() {
            self.error(
                "E0302",
                format!(
                    "this function type takes {} parameter(s) but the lambda has {}",
                    expected_params.len(),
                    params.len()
                ),
                span,
            );
            return Ty::Unknown;
        }

        self.scopes.push();
        for (name, ty) in params.iter().zip(expected_params.iter()) {
            if !self.scopes.declare(Symbol {
                name: name.clone(),
                ty: ty.clone(),
                mutable: false,
            }) {
                self.error("E0202", format!("duplicate parameter `{name}`"), span);
            }
        }
        let found = self.check_expr_expected(body, Some((*expected_ret).clone()));
        self.scopes.pop();

        if !self.assignable(&expected_ret, &found) {
            self.type_mismatch(&expected_ret, &found, body.span);
        }
        Ty::Fn {
            params: expected_params,
            ret: expected_ret,
        }
    }

    /// Checks an expression with an expected type, so lambdas can infer.
    fn check_expr_expected(&mut self, expr: &Expr, expected: Option<Ty>) -> Ty {
        let previous = std::mem::replace(&mut self.expected, expected);
        let found = self.check_expr(expr);
        self.expected = previous;
        found
    }

    /// The type of a literal.
    fn literal_type(&mut self, literal: &Literal, _span: orv_syntax::Span) -> Ty {
        match literal {
            Literal::Int(_) => Ty::Int,
            Literal::Float(_) => Ty::Float,
            Literal::Bool(_) => Ty::Bool,
            Literal::None => Ty::Optional(Box::new(Ty::Unknown)),
            // Interpolation parts are not sub-parsed in the alpha (ADR 0008
            // rule 4 / ADR 0012), so a string is always `Str`.
            Literal::Str(_) => Ty::Str,
        }
    }

    /// The type of a name.
    ///
    /// Besides locals, parameters and functions, an enum *variant* is a value
    /// in scope: `Circle` is callable and produces a `Shape` (§5.3).
    fn ident_type(&mut self, name: &str, span: orv_syntax::Span) -> Ty {
        if let Some(symbol) = self.scopes.lookup(name) {
            return symbol.ty.clone();
        }
        if let Some(user) = self.user_types.get(name) {
            return match user {
                UserType::Data { .. } => Ty::Data(name.to_owned()),
                UserType::Enum { .. } => Ty::Enum(name.to_owned()),
            };
        }
        if let Some((enum_name, payload)) = self.find_variant(name) {
            // A variant with a payload behaves like a function from its payload
            // to the enum type; a unit variant is the value itself.
            if payload.is_empty() {
                return Ty::Enum(enum_name);
            }
            return Ty::Fn {
                params: payload,
                ret: Box::new(Ty::Enum(enum_name)),
            };
        }
        self.error("E0201", format!("undefined name `{name}`"), span);
        Ty::Unknown
    }

    /// Finds a variant by name across the declared enums.
    ///
    /// Returns the enum it belongs to and its payload types.
    fn find_variant(&self, name: &str) -> Option<(String, Vec<Ty>)> {
        for (enum_name, user) in &self.user_types {
            if let UserType::Enum { variants } = user
                && let Some(variant) = variants.iter().find(|v| v.name == name)
            {
                return Some((enum_name.clone(), variant.payload.clone()));
            }
        }
        None
    }

    /// The type of `receiver.name`.
    fn field_type(&mut self, receiver: &Ty, name: &str, span: orv_syntax::Span) -> Ty {
        match receiver {
            Ty::Data(type_name) => {
                let Some(UserType::Data { fields }) = self.user_types.get(type_name) else {
                    return Ty::Unknown;
                };
                match fields.iter().find(|(field, _)| field == name) {
                    Some((_, ty)) => ty.clone(),
                    None => {
                        self.error(
                            "E0304",
                            format!("`{type_name}` has no field `{name}`"),
                            span,
                        );
                        Ty::Unknown
                    }
                }
            }
            Ty::Unknown => Ty::Unknown,
            other => {
                self.error("E0304", format!("`{other}` has no field `{name}`"), span);
                Ty::Unknown
            }
        }
    }

    /// The type of a call.
    fn check_call(&mut self, callee: &Expr, args: &[Arg], span: orv_syntax::Span) -> Ty {
        // A call whose callee is a type name constructs a value (§5.3).
        if let ExprKind::Ident(name) = &callee.kind
            && let Some(user) = self.user_types.get(name).cloned()
        {
            return self.check_construction(name, user, args, span);
        }

        let callee_ty = self.check_expr(callee);
        let Ty::Fn { params, ret } = callee_ty else {
            if callee_ty.is_unknown() {
                // The callee already failed; still check the arguments.
                for arg in args {
                    self.check_expr(&arg.value);
                }
                return Ty::Unknown;
            }
            self.error(
                "E0303",
                format!("`{callee_ty}` is not callable"),
                callee.span,
            );
            return Ty::Unknown;
        };

        // Positional and named arguments are matched by position; named
        // arguments reorder against the declared parameter names where known.
        if args.len() != params.len() {
            self.error(
                "E0302",
                format!(
                    "this function takes {} argument(s) but {} were given",
                    params.len(),
                    args.len()
                ),
                span,
            );
        }
        for (index, arg) in args.iter().enumerate() {
            let expected = params.get(index).cloned();
            let found = self.check_expr_expected(&arg.value, expected.clone());
            if let Some(expected) = expected
                && !self.assignable(&expected, &found)
            {
                self.type_mismatch(&expected, &found, arg.value.span);
            }
        }
        (*ret).clone()
    }

    /// Checks `Type(field: value, ...)` or `Variant(payload, ...)`.
    fn check_construction(
        &mut self,
        name: &str,
        user: UserType,
        args: &[Arg],
        span: orv_syntax::Span,
    ) -> Ty {
        match user {
            UserType::Data { fields } => {
                // Positional arguments fill fields in order; named ones must
                // match a field name.
                if args.len() > fields.len() {
                    self.error(
                        "E0302",
                        format!(
                            "`{name}` has {} field(s) but {} argument(s) were given",
                            fields.len(),
                            args.len()
                        ),
                        span,
                    );
                }
                for (index, arg) in args.iter().enumerate() {
                    let expected = match &arg.name {
                        Some(field_name) => fields
                            .iter()
                            .find(|(field, _)| field == field_name)
                            .map(|(_, ty)| ty.clone()),
                        None => fields.get(index).map(|(_, ty)| ty.clone()),
                    };
                    let found = self.check_expr_expected(&arg.value, expected.clone());
                    match expected {
                        Some(expected) => {
                            if !self.assignable(&expected, &found) {
                                self.type_mismatch(&expected, &found, arg.value.span);
                            }
                        }
                        None => {
                            let label = arg.name.clone().unwrap_or_else(|| index.to_string());
                            self.error(
                                "E0304",
                                format!("`{name}` has no field `{label}`"),
                                arg.value.span,
                            );
                        }
                    }
                }
                Ty::Data(name.to_owned())
            }
            UserType::Enum { variants } => {
                // A call on the enum name itself is only valid for a single
                // unit variant; payload construction goes through the variant
                // expression form handled by `ident_type` below.
                let _ = variants;
                self.error(
                    "E0303",
                    format!("`{name}` is an enum; construct a variant instead"),
                    span,
                );
                Ty::Unknown
            }
        }
    }

    /// The type of `receiver[index]`.
    fn index_type(&mut self, receiver: &Ty, index: &Ty, span: orv_syntax::Span) -> Ty {
        match receiver {
            Ty::List(element) => {
                if !matches!(index, Ty::Int | Ty::Unknown) {
                    self.type_mismatch(&Ty::Int, index, span);
                }
                (**element).clone()
            }
            Ty::Map(key, value) => {
                if !self.assignable(key, index) {
                    self.type_mismatch(key, index, span);
                }
                (**value).clone()
            }
            Ty::Str => {
                if !matches!(index, Ty::Int | Ty::Unknown) {
                    self.type_mismatch(&Ty::Int, index, span);
                }
                Ty::Str
            }
            Ty::Unknown => Ty::Unknown,
            other => {
                self.error("E0303", format!("`{other}` cannot be indexed"), span);
                Ty::Unknown
            }
        }
    }

    /// The type of a prefix operation.
    fn unary_type(&mut self, op: UnaryOp, operand: &Ty, span: orv_syntax::Span) -> Ty {
        match op {
            UnaryOp::Neg => {
                if !operand.is_numeric() && !operand.is_unknown() {
                    self.error("E0301", format!("cannot negate `{operand}`"), span);
                    return Ty::Unknown;
                }
                operand.clone()
            }
            UnaryOp::Not => {
                if !operand.is_bool() && !operand.is_unknown() {
                    self.error(
                        "E0301",
                        format!("`not` expects `Bool`, found `{operand}`"),
                        span,
                    );
                    return Ty::Unknown;
                }
                Ty::Bool
            }
        }
    }

    /// The type of a binary operation.
    fn binary_type(&mut self, op: BinaryOp, left: &Ty, right: &Ty, span: orv_syntax::Span) -> Ty {
        use BinaryOp::*;
        match op {
            Coalesce => {
                // `x ?? default` unwraps an optional, or passes a concrete
                // value through (§5.3).
                match left {
                    Ty::Optional(inner) => {
                        if !self.assignable(inner, right) {
                            self.type_mismatch(inner, right, span);
                        }
                        (**inner).clone()
                    }
                    Ty::Unknown => Ty::Unknown,
                    concrete => {
                        if !self.assignable(concrete, right) {
                            self.type_mismatch(concrete, right, span);
                        }
                        concrete.clone()
                    }
                }
            }
            Or | And => {
                if !left.is_bool() && !left.is_unknown() {
                    self.type_mismatch(&Ty::Bool, left, span);
                }
                if !right.is_bool() && !right.is_unknown() {
                    self.type_mismatch(&Ty::Bool, right, span);
                }
                Ty::Bool
            }
            Eq | NotEq => {
                if !self.assignable(left, right) && !self.assignable(right, left) {
                    self.type_mismatch(left, right, span);
                }
                Ty::Bool
            }
            Lt | Le | Gt | Ge => {
                if !self.comparable(left, right) {
                    self.type_mismatch(left, right, span);
                }
                Ty::Bool
            }
            Range | RangeInclusive => {
                if !matches!(left, Ty::Int | Ty::Unknown) {
                    self.type_mismatch(&Ty::Int, left, span);
                }
                if !matches!(right, Ty::Int | Ty::Unknown) {
                    self.type_mismatch(&Ty::Int, right, span);
                }
                Ty::List(Box::new(Ty::Int))
            }
            Add => {
                // `+` concatenates strings and adds numbers.
                if matches!(left, Ty::Str) || matches!(right, Ty::Str) {
                    if !matches!(left, Ty::Str | Ty::Unknown) {
                        self.type_mismatch(&Ty::Str, left, span);
                    }
                    if !matches!(right, Ty::Str | Ty::Unknown) {
                        self.type_mismatch(&Ty::Str, right, span);
                    }
                    return Ty::Str;
                }
                self.numeric_result(left, right, span)
            }
            Sub | Mul | Div | Rem => self.numeric_result(left, right, span),
        }
    }

    /// The result of an arithmetic operation on two operands.
    fn numeric_result(&mut self, left: &Ty, right: &Ty, span: orv_syntax::Span) -> Ty {
        if left.is_unknown() || right.is_unknown() {
            return Ty::Unknown;
        }
        if !left.is_numeric() {
            self.error("E0301", format!("`{left}` is not numeric"), span);
            return Ty::Unknown;
        }
        if !right.is_numeric() {
            self.error("E0301", format!("`{right}` is not numeric"), span);
            return Ty::Unknown;
        }
        // Int op Float promotes to Float; Int op Int stays Int.
        if matches!(left, Ty::Float) || matches!(right, Ty::Float) {
            Ty::Float
        } else {
            Ty::Int
        }
    }

    /// Whether `source as target` is a conversion §5.3 allows.
    ///
    /// Only `Int as Float` is defined for the alpha's builtin types (§5.3:
    /// "`Int as Float` sempre ok"; `Py as T` is checked at runtime and `Py` is
    /// out of scope). A cast to the same type is harmless and allowed.
    fn cast_allowed(&self, source: &Ty, target: &Ty) -> bool {
        if source.is_unknown() || target.is_unknown() {
            return true;
        }
        if source == target {
            return true;
        }
        matches!((source, target), (Ty::Int, Ty::Float))
    }

    /// Whether two types may be compared with `<` and friends.
    fn comparable(&self, left: &Ty, right: &Ty) -> bool {
        left.is_unknown()
            || right.is_unknown()
            || (left.is_numeric() && right.is_numeric())
            || (matches!(left, Ty::Str) && matches!(right, Ty::Str))
    }

    // --- Patterns -----------------------------------------------------------

    /// Checks that a unit-variant pattern belongs to the scrutinee's enum.
    fn check_unit_variant_pattern(&mut self, name: &str, scrutinee: &Ty, span: orv_syntax::Span) {
        match scrutinee {
            Ty::Enum(enum_name) => {
                let belongs = matches!(
                    self.user_types.get(enum_name),
                    Some(UserType::Enum { variants })
                        if variants.iter().any(|v| v.name == name)
                );
                if !belongs {
                    self.error(
                        "E0321",
                        format!("`{enum_name}` has no variant `{name}`"),
                        span,
                    );
                }
            }
            // A variant pattern against a non-enum is a type error, but only
            // when the scrutinee was actually resolved.
            Ty::Unknown => {}
            other => {
                self.error(
                    "E0301",
                    format!("`{other}` is not an enum, so `{name}` is not a variant"),
                    span,
                );
            }
        }
    }

    /// Checks a pattern against the scrutinee's type.
    fn check_pattern(&mut self, pattern: &Pattern, scrutinee: &Ty) {
        match &pattern.kind {
            PatternKind::Wildcard => {}
            // §5.2's grammar cannot tell a unit-variant pattern (`Circle`)
            // from a binding (`x`): both are a bare `IDENT`. Resolution decides
            // — a name that matches a declared variant is a *variant pattern*,
            // anything else binds the value (§5.3), which is also how Rust
            // reads the same ambiguity.
            PatternKind::Bind(name) => match self.find_variant(name) {
                Some((_, payload)) if payload.is_empty() => {
                    self.check_unit_variant_pattern(name, scrutinee, pattern.span);
                }
                Some((_, payload)) => {
                    self.error(
                        "E0302",
                        format!(
                            "variant `{name}` has {} field(s) but none were bound",
                            payload.len()
                        ),
                        pattern.span,
                    );
                }
                None => {
                    self.scopes.declare(Symbol {
                        name: name.clone(),
                        ty: scrutinee.clone(),
                        mutable: false,
                    });
                }
            },
            PatternKind::Literal(literal) => {
                let literal_ty = self.literal_type(literal, pattern.span);
                if !self.assignable(scrutinee, &literal_ty)
                    && !self.assignable(&literal_ty, scrutinee)
                {
                    self.type_mismatch(scrutinee, &literal_ty, pattern.span);
                }
            }
            PatternKind::Variant { name, fields } => {
                let payload = match scrutinee {
                    Ty::Enum(enum_name) => {
                        self.user_types.get(enum_name).and_then(|user| match user {
                            UserType::Enum { variants } => variants
                                .iter()
                                .find(|variant| &variant.name == name)
                                .map(|variant| variant.payload.clone()),
                            UserType::Data { .. } => None,
                        })
                    }
                    _ => None,
                };
                match payload {
                    Some(payload) => {
                        if payload.len() != fields.len() {
                            self.error(
                                "E0302",
                                format!(
                                    "variant `{name}` has {} field(s) but {} were bound",
                                    payload.len(),
                                    fields.len()
                                ),
                                pattern.span,
                            );
                        }
                        for (index, field) in fields.iter().enumerate() {
                            let ty = payload.get(index).cloned().unwrap_or(Ty::Unknown);
                            self.check_pattern(field, &ty);
                        }
                    }
                    None => {
                        if !scrutinee.is_unknown() {
                            self.error(
                                "E0321",
                                format!("`{scrutinee}` has no variant `{name}`"),
                                pattern.span,
                            );
                        }
                    }
                }
            }
        }
    }

    // --- Compatibility ------------------------------------------------------

    /// Whether a value of type `found` may be used where `expected` is needed.
    ///
    /// `Unknown` is compatible in both directions: it means "a mistake was
    /// already reported", and propagating it silently prevents a single cause
    /// from producing a cascade of diagnostics.
    pub fn assignable(&self, expected: &Ty, found: &Ty) -> bool {
        if expected.is_unknown() || found.is_unknown() {
            return true;
        }
        if expected == found {
            return true;
        }
        match (expected, found) {
            // `Int` widens to `Float` implicitly in arithmetic contexts only;
            // elsewhere §5.3 requires an explicit `as`.
            (Ty::Optional(expected_inner), Ty::Optional(found_inner)) => {
                self.assignable(expected_inner, found_inner)
            }
            // A concrete value may be used where its optional is expected:
            // `let e: Str? = "x"` is a widening, not a mismatch (§5.3). This is
            // what makes `email: Str? = none` usable with a real address.
            (Ty::Optional(expected_inner), found) => self.assignable(expected_inner, found),
            (Ty::List(expected_inner), Ty::List(found_inner)) => {
                self.assignable(expected_inner, found_inner)
            }
            (Ty::Map(ek, ev), Ty::Map(fk, fv)) => {
                self.assignable(ek, fk) && self.assignable(ev, fv)
            }
            (Ty::Result(expected_inner), Ty::Result(found_inner)) => {
                self.assignable(expected_inner, found_inner)
            }
            (Ty::Tuple(expected), Ty::Tuple(found)) => {
                expected.len() == found.len()
                    && expected
                        .iter()
                        .zip(found.iter())
                        .all(|(e, f)| self.assignable(e, f))
            }
            (Ty::Data(expected), Ty::Data(found)) => expected == found,
            (Ty::Enum(expected), Ty::Enum(found)) => expected == found,
            // A `Result<T, Failure>` is accepted where `Result` is expected.
            _ => false,
        }
    }

    /// The join of two branch types, reporting when they disagree.
    fn unify(&mut self, left: &Ty, right: &Ty, span: orv_syntax::Span) -> Ty {
        if left.is_unknown() {
            return right.clone();
        }
        if right.is_unknown() {
            return left.clone();
        }
        if self.assignable(left, right) {
            return left.clone();
        }
        if self.assignable(right, left) {
            return right.clone();
        }
        // Mixed int/float arithmetic promotes; a branch join does too.
        if left.is_numeric() && right.is_numeric() {
            return Ty::Float;
        }
        self.type_mismatch(left, right, span);
        Ty::Unknown
    }

    /// Requires a condition to be `Bool` (§5.3: no truthiness).
    fn require_bool(&mut self, ty: &Ty, span: orv_syntax::Span) {
        if !ty.is_bool() && !ty.is_unknown() {
            self.error(
                "E0312",
                format!("condition must be `Bool`, found `{ty}`"),
                span,
            );
        }
    }

    /// Whether `ty` can be a `Map` key (§5.3: `K ∈ {Int, Str, Bool}`).
    ///
    /// `Unknown` is allowed so a single error does not cascade into a second,
    /// misleading map-key diagnostic.
    fn is_valid_map_key(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::Int | Ty::Str | Ty::Bool) || ty.is_unknown()
    }

    /// Resolves a user type name against the declarations.
    /// Resolves user type names against the declarations, **recursively**.
    ///
    /// `resolve_type` cannot know whether `Shape` is a `data` or an `enum`, so
    /// it returns `Ty::Data` and this pass corrects it. The recursion matters:
    /// in `fn(Float) -> Shape` or `List<Shape>` the name is nested, and leaving
    /// it as `Ty::Data` makes `assignable` reject a value the expression side
    /// types as `Ty::Enum` — the same type reported as "expected `Shape`, found
    /// `Shape`".
    fn resolve_user_type(&mut self, ty: Ty, span: orv_syntax::Span) -> Ty {
        match ty {
            Ty::Data(name) => match self.user_types.get(&name) {
                Some(UserType::Data { .. }) => Ty::Data(name),
                Some(UserType::Enum { .. }) => Ty::Enum(name),
                None => {
                    self.error("E0201", format!("undefined type `{name}`"), span);
                    Ty::Unknown
                }
            },
            Ty::List(element) => Ty::List(Box::new(self.resolve_user_type(*element, span))),
            Ty::Optional(inner) => Ty::Optional(Box::new(self.resolve_user_type(*inner, span))),
            Ty::Result(ok) => Ty::Result(Box::new(self.resolve_user_type(*ok, span))),
            Ty::Map(key, value) => Ty::Map(
                Box::new(self.resolve_user_type(*key, span)),
                Box::new(self.resolve_user_type(*value, span)),
            ),
            Ty::Tuple(elements) => Ty::Tuple(
                elements
                    .into_iter()
                    .map(|element| self.resolve_user_type(element, span))
                    .collect(),
            ),
            Ty::Fn { params, ret } => Ty::Fn {
                params: params
                    .into_iter()
                    .map(|param| self.resolve_user_type(param, span))
                    .collect(),
                ret: Box::new(self.resolve_user_type(*ret, span)),
            },
            other => other,
        }
    }

    // --- Diagnostics --------------------------------------------------------

    /// Records a declaration name, reporting `E0202` on a duplicate.
    fn declare_name(&mut self, name: &str, span: orv_syntax::Span) -> bool {
        if self
            .declarations
            .iter()
            .any(|(existing, _)| existing == name)
        {
            self.error("E0202", format!("`{name}` is defined more than once"), span);
            return false;
        }
        self.declarations.push((name.to_owned(), span));
        true
    }

    /// Reports a type mismatch.
    fn type_mismatch(&mut self, expected: &Ty, found: &Ty, span: orv_syntax::Span) {
        // The two types can render identically (`expected `Shape`, found
        // `Shape``) when the names match but the kinds do not, which hides the
        // cause. Disambiguate with the internal form in that case.
        let expected_text = expected.name();
        let found_text = found.name();
        let message = if expected_text == found_text && expected != found {
            format!("expected `{expected_text}`, found `{found_text}` ({expected:?} vs {found:?})")
        } else {
            format!("expected `{expected_text}`, found `{found_text}`")
        };
        self.error("E0301", message, span);
    }

    /// Pushes an error diagnostic.
    fn error(&mut self, code: &'static str, message: impl Into<String>, span: orv_syntax::Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, span));
    }
}

/// Convenience for callers that only have the AST.
pub fn check(program: &Program) -> CheckResult {
    Checker::new().check(program)
}

#[cfg(test)]
mod tests;
