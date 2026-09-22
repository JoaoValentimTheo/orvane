//! The tree-walking interpreter (SPEC §5.2, §5.3, §5.5).
//!
//! Design notes
//! ------------
//! * **Never panics.** Arithmetic uses checked operations and reports a
//!   [`Failure`] (SPEC §5.3: overflow and division by zero are failures, not
//!   wrap or panic). Indexing and map lookups are bounds-checked. A `match`
//!   with no matching arm is a failure, not an `unreachable!`.
//! * **Recursion is bounded** so deep recursion is a diagnostic rather than a
//!   stack overflow (ADR 0016).
//! * **Determinism.** Maps preserve insertion order ([`MapValue`]), so `print`
//!   of a map is stable.
//! * **Control flow.** `break`/`continue`/`return` must escape outward through
//!   expressions, which return `Result`. They travel as a [`FailureKind::Flow`]
//!   failure carrying the value; [`step`](Interpreter::step) and the loop
//!   handlers are the only places that interpret them, and they never reach the
//!   user.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use orv_sema::Ty;
use orv_syntax::{Expr, ExprKind, Literal, Span};

use crate::builtins;
use crate::env::Env;
use crate::failure::{Failure, FailureKind, Frame};
use crate::function::Closure;
use crate::value::{MapKey, MapValue, Value, display};

/// Deepest Orvane call nesting before the interpreter gives up.
///
/// The interpreter is a tree-walker, so one Orvane call costs many Rust frames
/// (arguments, callee, body, and every expression inside). The limit is set
/// where the guard reliably fires before a default thread stack overflows
/// (ADR 0016).
pub const MAX_CALL_DEPTH: usize = 48;

/// What evaluation produced.
pub type EvalResult = Result<Value, Box<Failure>>;

/// Control flow leaving a statement.
///
/// A `Loop` body may end normally, return from the enclosing function, or break
/// out of the loop; `continue` is folded into a normal completion by the loop
/// handler.
#[derive(Debug)]
pub enum Control {
    /// Normal completion with a value.
    Value(Value),
    /// `return expr` left the function.
    Return(Value),
    /// `break` left the loop.
    Break,
    /// `continue` skipped to the next iteration.
    Continue,
}

/// A statement-level result: a value, early control, or a runtime failure.
///
/// Early control travels as an `Err` carrying a [`FailureKind::Flow`] failure so
/// the evaluator stays on one result type; see the module docs.
pub type StepResult = Result<Control, Box<Failure>>;

/// The interpreter state.
pub struct Interpreter {
    /// The global scope: top-level functions bind here.
    pub global: Env,
    /// The scope evaluation currently runs in.
    env: Env,
    /// Call stack, innermost last, for failure traces.
    frames: Vec<Frame>,
    /// Current call depth, checked against [`MAX_CALL_DEPTH`].
    depth: usize,
    /// Output collected by `print`, so `orv run` and tests can assert on it.
    output: RefCell<String>,
    /// Top-level functions and builtins by name.
    functions: HashMap<String, Rc<Closure>>,
    /// `data` field names by type name, in declaration order.
    data_schemas: HashMap<String, Vec<String>>,
    /// Variants by variant name: `(enum name, payload arity)`.
    variant_schemas: HashMap<String, (String, usize)>,
    /// Control flow raised inside an expression, re-read by the statement that
    /// evaluated it (`break`/`continue`/`return` crossing a block boundary).
    pending: Option<Control>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    /// Creates an interpreter with the prelude installed.
    pub fn new() -> Self {
        let global = Env::new();
        let mut interpreter = Self {
            env: global.clone(),
            global,
            frames: Vec::new(),
            depth: 0,
            output: RefCell::new(String::new()),
            functions: HashMap::new(),
            data_schemas: HashMap::new(),
            variant_schemas: HashMap::new(),
            pending: None,
        };
        builtins::install(&mut interpreter);
        interpreter
    }

    // --- Registration -------------------------------------------------------

    /// Registers a builtin by name and arity.
    ///
    /// The implementation lives in [`builtins::lookup`]; the `Closure` is a
    /// placeholder so builtins share the namespace of user functions.
    pub fn define_builtin(&mut self, name: &str, arity: usize) {
        let closure = Rc::new(Closure::builtin(name, arity));
        self.functions.insert(name.to_owned(), closure.clone());
        self.global
            .define(Rc::from(name), Value::Function(closure), false);
    }

    /// Registers a top-level function.
    pub fn define_function(&mut self, name: &str, closure: Rc<Closure>) {
        self.functions.insert(name.to_owned(), closure.clone());
        self.global
            .define(Rc::from(name), Value::Function(closure), false);
    }

    /// Registers a `data` type's field names, in declaration order.
    pub fn define_data_schema(&mut self, name: &str, fields: Vec<String>) {
        self.data_schemas.insert(name.to_owned(), fields);
    }

    /// The field names of a `data` type.
    pub fn data_schema(&self, name: &str) -> Option<Vec<String>> {
        self.data_schemas.get(name).cloned()
    }

    /// Registers an enum variant's enum name and payload arity.
    pub fn define_variant_schema(&mut self, variant: &str, enum_name: &str, arity: usize) {
        self.variant_schemas
            .insert(variant.to_owned(), (enum_name.to_owned(), arity));
    }

    /// The enum name and payload arity of a variant.
    pub fn variant_schema(&self, name: &str) -> Option<(String, usize)> {
        self.variant_schemas.get(name).cloned()
    }

    /// Looks a function (or builtin) up by name.
    pub fn function(&self, name: &str) -> Option<Rc<Closure>> {
        self.functions.get(name).cloned()
    }

    // --- Output -------------------------------------------------------------

    /// Appends text to the collected output.
    pub fn write_output(&self, text: &str) {
        self.output.borrow_mut().push_str(text);
    }

    /// Takes everything written so far.
    pub fn take_output(&self) -> String {
        std::mem::take(&mut *self.output.borrow_mut())
    }

    /// The current output without consuming it.
    pub fn output(&self) -> String {
        self.output.borrow().clone()
    }

    // --- Expressions --------------------------------------------------------

    /// Evaluates an expression.
    pub fn eval(&mut self, expr: &Expr) -> EvalResult {
        match &expr.kind {
            ExprKind::Literal(literal) => Ok(literal_value(literal)),
            ExprKind::Ident(name) => self.eval_ident(name, expr.span),
            ExprKind::Paren(inner) => self.eval(inner),
            ExprKind::Block(block) => self.eval_block_as_expr(block),
            ExprKind::Field { receiver, name } => {
                let value = self.eval(receiver)?;
                self.field(&value, name, expr.span)
            }
            ExprKind::OptionalField { receiver, name } => {
                let value = self.eval(receiver)?;
                if matches!(value, Value::None) {
                    Ok(Value::None)
                } else {
                    self.field(&value, name, expr.span)
                }
            }
            ExprKind::Call { callee, args } => self.eval_call(callee, args, expr.span),
            ExprKind::Index { receiver, index } => {
                let container = self.eval(receiver)?;
                let index = self.eval(index)?;
                self.index(&container, &index, expr.span)
            }
            ExprKind::Unary { op, operand } => {
                let value = self.eval(operand)?;
                self.unary(*op, value, expr.span)
            }
            ExprKind::Binary { op, left, right } => self.eval_binary(*op, left, right, expr.span),
            ExprKind::Cast { expr: inner, ty } => {
                let value = self.eval(inner)?;
                self.cast(value, ty, expr.span)
            }
            ExprKind::Tuple(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    values.push(self.eval(element)?);
                }
                Ok(Value::Tuple(Rc::new(values)))
            }
            ExprKind::List(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    values.push(self.eval(element)?);
                }
                Ok(Value::list(values))
            }
            ExprKind::Map(entries) => self.eval_map(entries),
            ExprKind::If {
                condition,
                then_block,
                else_branch,
            } => {
                let condition = self.eval(condition)?;
                let Some(taken) = condition.as_bool() else {
                    return Err(self.unsupported("condition is not a `Bool`", expr.span));
                };
                if taken {
                    self.eval_block_as_expr(then_block)
                } else {
                    match else_branch {
                        Some(branch) => self.eval(branch),
                        None => Ok(Value::Unit),
                    }
                }
            }
            ExprKind::Match { scrutinee, arms } => {
                let value = self.eval(scrutinee)?;
                self.eval_match(&value, arms, expr.span)
            }
            ExprKind::Try(inner) => Ok(match self.eval(inner) {
                Ok(value) => Value::Result {
                    ok: true,
                    value: Box::new(value),
                    failure: None,
                },
                Err(failure) => Value::Result {
                    ok: false,
                    value: Box::new(Value::Unit),
                    failure: Some(failure),
                },
            }),
            ExprKind::Fail(inner) => {
                let value = self.eval(inner)?;
                Err(self.unsupported(display(&value), expr.span))
            }
            ExprKind::Lambda { params, body } => Ok(Value::Function(Rc::new(Closure::lambda(
                params.clone(),
                Rc::new(body.as_ref().clone()),
                self.env.snapshot(),
                expr.span,
            )))),
        }
    }

    /// Evaluates an identifier: a local binding, a function, or a bare
    /// unit-variant such as `Active`.
    ///
    /// The unit-variant case is the runtime mirror of
    /// [`Checker::ident_type`](orv_sema::Checker): when the checker resolves a
    /// name to an enum variant it returns `Ty::Enum(name)`, so the runtime must
    /// be able to build that value here. Without this the front end accepts a
    /// program the interpreter then refuses with R0010 — sema and runtime
    /// disagreeing about what is valid.
    ///
    /// Only arity-0 variants are built here; a variant *with* a payload is a
    /// constructor and is handled by [`Self::eval_call`].
    fn eval_ident(&mut self, name: &str, span: Span) -> EvalResult {
        if let Some(value) = self.env.get(name) {
            return Ok(value);
        }
        if let Some(function) = self.functions.get(name) {
            return Ok(Value::Function(function.clone()));
        }
        if let Some((enum_name, 0)) = self.variant_schema(name) {
            return Ok(Value::Variant {
                enum_name: Rc::from(enum_name.as_str()),
                variant: Rc::from(name),
                payload: Rc::new(Vec::new()),
            });
        }
        if let Some((enum_name, arity)) = self.variant_schema(name) {
            // A payload-carrying variant is a first-class constructor value of
            // type `fn(payload) -> Enum` (ADR 0017), so the checker's `Ty::Fn`
            // has a runtime counterpart.
            return Ok(Value::Function(Rc::new(Closure::constructor(
                &enum_name, name, arity,
            ))));
        }
        Err(self.unsupported(format!("undefined name `{name}`"), span))
    }

    /// Evaluates `#{k: v, ...}`.
    fn eval_map(&mut self, entries: &[(Expr, Expr)]) -> EvalResult {
        let mut map = MapValue::new();
        for (key, value) in entries {
            let key_value = self.eval(key)?;
            let Some(key) = MapKey::from_value(&key_value) else {
                return Err(self.unsupported(
                    format!(
                        "map keys must be Int, Str or Bool, found `{}`",
                        key_value.ty()
                    ),
                    key.span,
                ));
            };
            let value = self.eval(value)?;
            map.insert(key, value);
        }
        Ok(Value::map(map))
    }

    // --- Blocks and statements ---------------------------------------------

    /// Evaluates a block as an expression, propagating early control.
    ///
    /// A `break` inside an `if` arm must reach the enclosing loop rather than
    /// become the arm's value, so the control is parked in [`Self::pending`]
    /// and re-raised by the statement that evaluates this expression.
    fn eval_block_as_expr(&mut self, block: &orv_syntax::Block) -> EvalResult {
        self.env.push();
        let outcome = self.run_body(&block.statements);
        self.env.pop();
        match outcome? {
            Control::Value(value) => Ok(value),
            other => {
                self.pending = Some(other);
                Ok(Value::Unit)
            }
        }
    }

    /// Evaluates a block as a statement, discarding its value.
    ///
    /// Used where a block is a statement (`while`/`for` bodies, `if` arms in
    /// statement position); the value is not observable there.
    pub fn eval_block(&mut self, block: &orv_syntax::Block) -> StepResult {
        self.env.push();
        let outcome = self.run_body(&block.statements);
        self.env.pop();
        outcome
    }

    /// Runs statements, preserving early control flow.
    fn run_body(&mut self, statements: &[orv_syntax::Stmt]) -> StepResult {
        let mut last = Control::Value(Value::Unit);
        for statement in statements {
            match self.step(statement)? {
                Control::Value(value) => last = Control::Value(value),
                other => return Ok(other),
            }
        }
        Ok(last)
    }

    /// Executes one statement.
    pub fn step(&mut self, statement: &orv_syntax::Stmt) -> StepResult {
        use orv_syntax::StmtKind;
        match &statement.kind {
            StmtKind::Let {
                pattern,
                mutable,
                value,
                ..
            } => {
                let value = self.eval(value)?;
                if let orv_syntax::PatternKind::Bind(name) = &pattern.kind {
                    self.env.define(Rc::from(name.as_str()), value, *mutable);
                }
                Ok(Control::Value(Value::Unit))
            }
            StmtKind::Assign { target, op, value } => {
                let value = self.eval(value)?;
                self.assign(target, *op, value, statement.span)?;
                Ok(Control::Value(Value::Unit))
            }
            StmtKind::While { condition, body } => self.run_while(condition, body, statement.span),
            StmtKind::For {
                pattern,
                iterable,
                body,
            } => self.run_for(pattern, iterable, body, statement.span),
            StmtKind::Return(value) => {
                let value = match value {
                    Some(value) => self.eval(value)?,
                    None => Value::Unit,
                };
                Ok(Control::Return(value))
            }
            StmtKind::Break => Ok(Control::Break),
            StmtKind::Continue => Ok(Control::Continue),
            StmtKind::Fail(value) => {
                let value = self.eval(value)?;
                Err(self.unsupported(display(&value), statement.span))
            }
            StmtKind::Expr(expr) => {
                let value = self.eval(expr)?;
                // A block inside the expression may have raised control flow.
                match self.pending.take() {
                    Some(control) => Ok(control),
                    None => Ok(Control::Value(value)),
                }
            }
        }
    }

    /// Runs a `while` loop.
    fn run_while(&mut self, condition: &Expr, body: &orv_syntax::Block, span: Span) -> StepResult {
        loop {
            let condition = self.eval(condition)?;
            if let Some(control) = self.pending.take() {
                return Ok(control);
            }
            match condition.as_bool() {
                Some(true) => {}
                Some(false) => return Ok(Control::Value(Value::Unit)),
                None => return Err(self.unsupported("condition is not a `Bool`", span)),
            }
            match self.eval_block(body)? {
                Control::Break => return Ok(Control::Value(Value::Unit)),
                Control::Continue | Control::Value(_) => {}
                Control::Return(value) => return Ok(Control::Return(value)),
            }
        }
    }

    /// Runs a `for` loop over a list, map or string.
    fn run_for(
        &mut self,
        pattern: &orv_syntax::Pattern,
        iterable: &Expr,
        body: &orv_syntax::Block,
        span: Span,
    ) -> StepResult {
        let iterable = self.eval(iterable)?;
        let elements = self.iterable(&iterable, span)?;
        for element in elements {
            self.env.push();
            if let orv_syntax::PatternKind::Bind(name) = &pattern.kind {
                self.env.define(Rc::from(name.as_str()), element, false);
            }
            let outcome = self.run_body(&body.statements);
            self.env.pop();
            match outcome? {
                Control::Break => return Ok(Control::Value(Value::Unit)),
                Control::Continue | Control::Value(_) => continue,
                Control::Return(value) => return Ok(Control::Return(value)),
            }
        }
        Ok(Control::Value(Value::Unit))
    }

    // --- Assignment ---------------------------------------------------------

    /// Assigns to a variable, list element or map entry.
    fn assign(
        &mut self,
        target: &Expr,
        op: orv_syntax::AssignOp,
        value: Value,
        span: Span,
    ) -> Result<(), Box<Failure>> {
        let value = match op {
            orv_syntax::AssignOp::Assign => value,
            compound => {
                let current = self.eval(target)?;
                let op = match compound {
                    orv_syntax::AssignOp::Add => orv_syntax::BinaryOp::Add,
                    orv_syntax::AssignOp::Sub => orv_syntax::BinaryOp::Sub,
                    orv_syntax::AssignOp::Mul => orv_syntax::BinaryOp::Mul,
                    orv_syntax::AssignOp::Div => orv_syntax::BinaryOp::Div,
                    // `Assign` is handled above.
                    orv_syntax::AssignOp::Assign => orv_syntax::BinaryOp::Add,
                };
                self.binary_op(op, current, value, span)?
            }
        };

        match &target.kind {
            ExprKind::Ident(name) => match self.env.assign(name, value) {
                Ok(true) => Ok(()),
                Ok(false) => {
                    Err(self.unsupported(format!("cannot assign to immutable `{name}`"), span))
                }
                Err(()) => Err(self.unsupported(format!("undefined name `{name}`"), span)),
            },
            ExprKind::Index { receiver, index } => {
                let container = self.eval(receiver)?;
                let index = self.eval(index)?;
                self.assign_index(&container, &index, value, span)
            }
            // A `data` value is immutable behind its `Rc`, and assigning to a
            // field would need the binding that holds it. The alpha cannot
            // express that, so it is reported rather than silently dropped.
            ExprKind::Field { receiver, name } => {
                let container = self.eval(receiver)?;
                Err(self.unsupported(
                    format!(
                        "assigning to field `{name}` of a `{}` is not supported in 0.1.0-alpha",
                        container.ty()
                    ),
                    span,
                ))
            }
            _ => Err(self.unsupported("invalid assignment target", span)),
        }
    }

    /// Assigns to a list element or map entry.
    fn assign_index(
        &mut self,
        container: &Value,
        index: &Value,
        value: Value,
        span: Span,
    ) -> Result<(), Box<Failure>> {
        match container {
            Value::List(elements) => {
                let Some(position) = index.as_int() else {
                    return Err(self.unsupported("list index must be an `Int`", span));
                };
                let mut elements = elements.borrow_mut();
                let Some(position) = normalize_index(position, elements.len()) else {
                    return Err(self.index_error(position, elements.len(), span));
                };
                elements[position] = value;
                Ok(())
            }
            Value::Map(entries) => {
                let Some(key) = MapKey::from_value(index) else {
                    return Err(self.unsupported("map keys must be Int, Str or Bool", span));
                };
                entries.borrow_mut().insert(key, value);
                Ok(())
            }
            other => Err(self.unsupported(
                format!("`{}` cannot be indexed for assignment", other.ty()),
                span,
            )),
        }
    }

    // --- Field, index, iteration -------------------------------------------

    /// Looks a field up on a value.
    fn field(&mut self, value: &Value, name: &str, span: Span) -> EvalResult {
        match value {
            Value::Data {
                name: type_name,
                fields,
            } => fields
                .iter()
                .find(|(field, _)| field.as_ref() == name)
                .map(|(_, value)| value.clone())
                .ok_or_else(|| {
                    self.unsupported(format!("`{type_name}` has no field `{name}`"), span)
                }),
            other => Err(self.unsupported(format!("`{}` has no field `{name}`", other.ty()), span)),
        }
    }

    /// Indexes a list, map or string.
    fn index(&mut self, container: &Value, index: &Value, span: Span) -> EvalResult {
        match container {
            Value::List(elements) => {
                let Some(position) = index.as_int() else {
                    return Err(self.unsupported("list index must be an `Int`", span));
                };
                let elements = elements.borrow();
                let Some(position) = normalize_index(position, elements.len()) else {
                    return Err(self.index_error(position, elements.len(), span));
                };
                Ok(elements[position].clone())
            }
            Value::Map(entries) => {
                let Some(key) = MapKey::from_value(index) else {
                    return Err(self.unsupported("map keys must be Int, Str or Bool", span));
                };
                entries
                    .borrow()
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| self.missing_key(key, span))
            }
            Value::Str(text) => {
                let Some(position) = index.as_int() else {
                    return Err(self.unsupported("string index must be an `Int`", span));
                };
                let characters: Vec<char> = text.chars().collect();
                let Some(position) = normalize_index(position, characters.len()) else {
                    return Err(self.index_error(position, characters.len(), span));
                };
                Ok(Value::str(characters[position].to_string()))
            }
            other => Err(self.unsupported(format!("`{}` cannot be indexed", other.ty()), span)),
        }
    }

    /// The values a `for` loop iterates.
    fn iterable(&mut self, value: &Value, span: Span) -> Result<Vec<Value>, Box<Failure>> {
        match value {
            Value::List(elements) => Ok(elements.borrow().clone()),
            Value::Map(entries) => Ok(entries
                .borrow()
                .keys()
                .iter()
                .map(MapKey::to_value)
                .collect()),
            Value::Str(text) => Ok(text.chars().map(|c| Value::str(c.to_string())).collect()),
            other => Err(self.unsupported(format!("`{}` is not iterable", other.ty()), span)),
        }
    }

    // --- Calls --------------------------------------------------------------

    /// Evaluates a call.
    fn eval_call(&mut self, callee: &Expr, args: &[orv_syntax::Arg], span: Span) -> EvalResult {
        if let ExprKind::Ident(name) = &callee.kind {
            // A `data` constructor, then an enum variant constructor.
            if let Some(value) = self.try_construct(name, args, span)? {
                return Ok(value);
            }
            if let Some(value) = self.try_variant(name, args, span)? {
                return Ok(value);
            }
        }

        let callee_value = self.eval(callee)?;
        let Value::Function(function) = callee_value else {
            return Err(self.unsupported(format!("`{}` is not callable", callee_value.ty()), span));
        };

        let mut evaluated = Vec::with_capacity(args.len());
        for arg in args {
            evaluated.push(self.eval(&arg.value)?);
        }
        self.call_function(&function, evaluated, span)
    }

    /// Calls a function value with already-evaluated arguments.
    pub fn call_function(
        &mut self,
        function: &Rc<Closure>,
        args: Vec<Value>,
        span: Span,
    ) -> EvalResult {
        if args.len() != function.arity {
            return Err(self.unsupported(
                format!(
                    "`{}` takes {} argument(s) but {} were given",
                    function.name,
                    function.arity,
                    args.len()
                ),
                span,
            ));
        }
        if let Some(builtin) = builtins::lookup(&function.name) {
            return builtin(self, &args, span);
        }
        if let Some((enum_name, variant, _arity)) = function.as_constructor() {
            return Ok(Value::Variant {
                enum_name: Rc::from(enum_name),
                variant: Rc::from(variant),
                payload: Rc::new(args),
            });
        }
        if self.depth >= MAX_CALL_DEPTH {
            return Err(Box::new(Failure::new(
                FailureKind::StackOverflow,
                format!("call depth exceeded {MAX_CALL_DEPTH} (possible infinite recursion)"),
                span,
            )));
        }

        let names = function.param_names();
        let call_env = self.call_env(function);
        let previous_env = std::mem::replace(&mut self.env, call_env);
        // Control flow is confined to the call: a `break`/`continue` parked by
        // the callee's body must never be read by the caller's loop (ADR 0021).
        // The caller's own pending control is saved and restored untouched.
        let caller_pending = self.pending.take();
        self.env.push();
        for (name, value) in names.iter().zip(args) {
            self.env.define(Rc::from(name.as_str()), value, false);
        }
        self.frames.push(Frame {
            function: function.name.clone(),
            span,
        });
        self.depth += 1;

        let outcome = match function.block() {
            Some(block) => self.run_body(&block.statements),
            None => match function.lambda_body() {
                Some(body) => {
                    let body = body.clone();
                    self.eval(&body).map(Control::Value)
                }
                None => Err(self.unsupported(format!("`{}` has no body", function.name), span)),
            },
        };

        // Anything the callee parked is the callee's own control flow; it has
        // nowhere to go at a call boundary.
        let leaked = self.pending.take();
        self.pending = caller_pending;

        self.depth = self.depth.saturating_sub(1);
        self.frames.pop();
        self.env.pop();
        self.env = previous_env;

        match outcome {
            // A `return` inside the body is the function's value.
            Ok(_) if leaked.is_some() => Err(self.unsupported(
                "`break` or `continue` cannot leave the function that contains it",
                span,
            )),
            Ok(Control::Return(value) | Control::Value(value)) => Ok(value),
            // `break`/`continue` outside a loop cannot leave a function; the
            // checker rejects it, and the runtime reports instead of panicking.
            Ok(Control::Break | Control::Continue) => {
                Err(self.unsupported("`break` or `continue` outside a loop", span))
            }
            Err(failure) => Err(failure),
        }
    }

    /// The environment a call starts from.
    ///
    /// A named function or builtin is top-level and sees the global scope; a
    /// lambda sees the scope it captured when the value was created. The
    /// captured chain is snapshotted per call so pushing the call's own frame
    /// (parameters) cannot leak into the closure value or another call.
    fn call_env(&self, function: &Rc<Closure>) -> Env {
        match &function.callable {
            crate::function::Callable::Lambda { captured, .. } => captured.snapshot(),
            crate::function::Callable::Named(_)
            | crate::function::Callable::Builtin
            | crate::function::Callable::Constructor { .. } => self.global.clone(),
        }
    }

    /// Builds a `data` value when `name` is a declared type.
    fn try_construct(
        &mut self,
        name: &str,
        args: &[orv_syntax::Arg],
        span: Span,
    ) -> Result<Option<Value>, Box<Failure>> {
        let Some(schema) = self.data_schema(name) else {
            return Ok(None);
        };
        let mut fields: Vec<(Rc<str>, Value)> = schema
            .iter()
            .map(|field| (Rc::from(field.as_str()), Value::None))
            .collect();

        for (index, arg) in args.iter().enumerate() {
            let value = self.eval(&arg.value)?;
            let position = match &arg.name {
                Some(field) => schema.iter().position(|name| name == field),
                None => Some(index),
            };
            match position {
                Some(position) if position < fields.len() => {
                    fields[position].1 = value;
                }
                _ => {
                    return Err(
                        self.unsupported(format!("`{name}` has no field for this argument"), span)
                    );
                }
            }
        }
        Ok(Some(Value::Data {
            name: Rc::from(name),
            fields: Rc::new(fields),
        }))
    }

    /// Builds an `enum` variant when `name` is one.
    fn try_variant(
        &mut self,
        name: &str,
        args: &[orv_syntax::Arg],
        span: Span,
    ) -> Result<Option<Value>, Box<Failure>> {
        let Some((enum_name, arity)) = self.variant_schema(name) else {
            return Ok(None);
        };
        let mut payload = Vec::with_capacity(args.len());
        for arg in args {
            payload.push(self.eval(&arg.value)?);
        }
        if payload.len() != arity {
            return Err(self.unsupported(
                format!(
                    "variant `{name}` takes {arity} field(s) but {} were given",
                    payload.len()
                ),
                span,
            ));
        }
        Ok(Some(Value::Variant {
            enum_name: Rc::from(enum_name.as_str()),
            variant: Rc::from(name),
            payload: Rc::new(payload),
        }))
    }

    // --- Operators ----------------------------------------------------------

    /// Applies a prefix operator.
    fn unary(&mut self, op: orv_syntax::UnaryOp, value: Value, span: Span) -> EvalResult {
        match op {
            orv_syntax::UnaryOp::Not => match value.as_bool() {
                Some(value) => Ok(Value::Bool(!value)),
                None => Err(self.unsupported(
                    format!("`not` expects `Bool`, found `{}`", value.ty()),
                    span,
                )),
            },
            orv_syntax::UnaryOp::Neg => match value {
                Value::Int(value) => value.checked_neg().map(Value::Int).ok_or_else(|| {
                    self.overflow("integer overflow negating the minimum `Int`", span)
                }),
                Value::Float(value) => Ok(Value::Float(-value)),
                other => Err(self.unsupported(format!("cannot negate `{}`", other.ty()), span)),
            },
        }
    }

    /// Evaluates a binary operation, short-circuiting `and`/`or`.
    fn eval_binary(
        &mut self,
        op: orv_syntax::BinaryOp,
        left: &Expr,
        right: &Expr,
        span: Span,
    ) -> EvalResult {
        use orv_syntax::BinaryOp;
        match op {
            // Short-circuit: the right side is only evaluated when it matters.
            BinaryOp::And => {
                let left = self.eval(left)?;
                match left.as_bool() {
                    Some(false) => Ok(Value::Bool(false)),
                    Some(true) => match self.eval(right)?.as_bool() {
                        Some(value) => Ok(Value::Bool(value)),
                        None => Err(self.unsupported("`and` expects `Bool`", span)),
                    },
                    None => Err(self
                        .unsupported(format!("`and` expects `Bool`, found `{}`", left.ty()), span)),
                }
            }
            BinaryOp::Or => {
                let left = self.eval(left)?;
                match left.as_bool() {
                    Some(true) => Ok(Value::Bool(true)),
                    Some(false) => match self.eval(right)?.as_bool() {
                        Some(value) => Ok(Value::Bool(value)),
                        None => Err(self.unsupported("`or` expects `Bool`", span)),
                    },
                    None => Err(self
                        .unsupported(format!("`or` expects `Bool`, found `{}`", left.ty()), span)),
                }
            }
            // `x ?? default` only evaluates the default when `x` is `none`.
            BinaryOp::Coalesce => {
                let left = self.eval(left)?;
                if matches!(left, Value::None) {
                    self.eval(right)
                } else {
                    Ok(left)
                }
            }
            _ => {
                let left = self.eval(left)?;
                let right = self.eval(right)?;
                self.binary_op(op, left, right, span)
            }
        }
    }

    /// Applies a binary operator to two values.
    fn binary_op(
        &mut self,
        op: orv_syntax::BinaryOp,
        left: Value,
        right: Value,
        span: Span,
    ) -> EvalResult {
        use orv_syntax::BinaryOp;
        match op {
            BinaryOp::Eq => Ok(Value::Bool(left.equals(&right))),
            BinaryOp::NotEq => Ok(Value::Bool(!left.equals(&right))),
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let ordering = self.compare(&left, &right, span)?;
                Ok(Value::Bool(match op {
                    BinaryOp::Lt => ordering.is_lt(),
                    BinaryOp::Le => ordering.is_le(),
                    BinaryOp::Gt => ordering.is_gt(),
                    _ => ordering.is_ge(),
                }))
            }
            BinaryOp::Range | BinaryOp::RangeInclusive => {
                let (Some(start), Some(end)) = (left.as_int(), right.as_int()) else {
                    return Err(self.unsupported("range bounds must be `Int`", span));
                };
                Ok(Value::list(int_range(
                    start,
                    end,
                    matches!(op, BinaryOp::RangeInclusive),
                )))
            }
            BinaryOp::Add => match (&left, &right) {
                // String concatenation and list concatenation (§5.3).
                (Value::Str(a), Value::Str(b)) => {
                    let mut text = String::with_capacity(a.len() + b.len());
                    text.push_str(a);
                    text.push_str(b);
                    Ok(Value::str(text))
                }
                (Value::List(a), Value::List(b)) => {
                    let mut combined = a.borrow().clone();
                    combined.extend(b.borrow().iter().cloned());
                    Ok(Value::list(combined))
                }
                _ => self.arithmetic(op, left, right, span),
            },
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                self.arithmetic(op, left, right, span)
            }
            // Handled by `eval_binary` before reaching here.
            BinaryOp::And | BinaryOp::Or | BinaryOp::Coalesce => Ok(Value::Bool(false)),
        }
    }

    /// Arithmetic with checked operations (§5.3: no wrap, no panic).
    fn arithmetic(
        &mut self,
        op: orv_syntax::BinaryOp,
        left: Value,
        right: Value,
        span: Span,
    ) -> EvalResult {
        use orv_syntax::BinaryOp;

        // Float arithmetic when either side is a float.
        if matches!(left, Value::Float(_)) || matches!(right, Value::Float(_)) {
            let (Some(a), Some(b)) = (as_float(&left), as_float(&right)) else {
                return Err(self.unsupported(
                    format!(
                        "cannot apply `{}` to `{}` and `{}`",
                        op.as_str(),
                        left.ty(),
                        right.ty()
                    ),
                    span,
                ));
            };
            if matches!(op, BinaryOp::Div | BinaryOp::Rem) && b == 0.0 {
                return Err(self.division_by_zero(span));
            }
            return Ok(Value::Float(match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a - b,
                BinaryOp::Mul => a * b,
                BinaryOp::Div => a / b,
                _ => a % b,
            }));
        }

        let (Some(a), Some(b)) = (left.as_int(), right.as_int()) else {
            return Err(self.unsupported(
                format!(
                    "cannot apply `{}` to `{}` and `{}`",
                    op.as_str(),
                    left.ty(),
                    right.ty()
                ),
                span,
            ));
        };

        if matches!(op, BinaryOp::Div | BinaryOp::Rem) && b == 0 {
            return Err(self.division_by_zero(span));
        }
        let result = match op {
            BinaryOp::Add => a.checked_add(b),
            BinaryOp::Sub => a.checked_sub(b),
            BinaryOp::Mul => a.checked_mul(b),
            BinaryOp::Div => a.checked_div(b),
            BinaryOp::Rem => a.checked_rem(b),
            _ => None,
        };
        result
            .map(Value::Int)
            .ok_or_else(|| self.overflow(format!("integer overflow in `{}`", op.as_str()), span))
    }

    /// Orders two comparable values.
    fn compare(
        &mut self,
        left: &Value,
        right: &Value,
        span: Span,
    ) -> Result<std::cmp::Ordering, Box<Failure>> {
        let ordering = match (left, right) {
            (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
            (Value::Str(a), Value::Str(b)) => Some(a.cmp(b)),
            _ => {
                return Err(self.unsupported(
                    format!("cannot compare `{}` and `{}`", left.ty(), right.ty()),
                    span,
                ));
            }
        };
        ordering.ok_or_else(|| self.unsupported("cannot order `NaN`", span))
    }

    /// Applies `expr as Type` (SPEC §5.3).
    fn cast(&mut self, value: Value, ty: &orv_syntax::Type, span: Span) -> EvalResult {
        let Ok(target) = orv_sema::resolve_type(ty) else {
            return Err(self.unsupported("invalid cast target", span));
        };
        match (&value, &target) {
            (Value::Int(value), Ty::Float) => Ok(Value::Float(*value as f64)),
            _ if value.ty() == target => Ok(value),
            _ => Err(self.unsupported(format!("cannot cast `{}` to `{target}`", value.ty()), span)),
        }
    }

    // --- Match --------------------------------------------------------------

    /// Evaluates a `match`; the first arm that matches wins (§5.2).
    fn eval_match(
        &mut self,
        value: &Value,
        arms: &[orv_syntax::MatchArm],
        span: Span,
    ) -> EvalResult {
        for arm in arms {
            self.env.push();
            let matched = self.match_pattern(&arm.pattern, value);
            if matched {
                let guard_ok = match &arm.guard {
                    Some(guard) => self.eval(guard)?.as_bool().unwrap_or(false),
                    None => true,
                };
                if guard_ok {
                    let outcome = self.eval(&arm.body);
                    self.env.pop();
                    return outcome;
                }
            }
            self.env.pop();
        }
        // Exhaustiveness is checked in M5; the interpreter still must not
        // panic, so an unmatched value is a failure.
        Err(self.unsupported(format!("no `match` arm matched `{}`", display(value)), span))
    }

    /// Whether `pattern` matches `value`, binding names in the current scope.
    fn match_pattern(&mut self, pattern: &orv_syntax::Pattern, value: &Value) -> bool {
        use orv_syntax::PatternKind;
        match &pattern.kind {
            PatternKind::Wildcard => true,
            // A bare name that is a declared variant is a variant pattern
            // (ADR 0015); otherwise it binds the value.
            PatternKind::Bind(name) => match self.variant_schema(name) {
                Some((_, arity)) => match value {
                    Value::Variant {
                        variant, payload, ..
                    } => variant.as_ref() == name && payload.len() == arity,
                    _ => false,
                },
                None => {
                    self.env
                        .define(Rc::from(name.as_str()), value.clone(), false);
                    true
                }
            },
            PatternKind::Literal(literal) => literal_value(literal).equals(value),
            PatternKind::Variant { name, fields } => match value {
                Value::Variant {
                    variant, payload, ..
                } if variant.as_ref() == name => {
                    payload.len() == fields.len()
                        && fields
                            .iter()
                            .zip(payload.iter())
                            .all(|(pattern, value)| self.match_pattern(pattern, value))
                }
                _ => false,
            },
        }
    }

    // --- Failure helpers ----------------------------------------------------

    /// A failure for an unsupported or invalid operation.
    fn unsupported(&self, message: impl Into<String>, span: Span) -> Box<Failure> {
        Box::new(Failure::new(FailureKind::Unsupported, message.into(), span))
    }

    /// An `R0001` division by zero.
    fn division_by_zero(&self, span: Span) -> Box<Failure> {
        Box::new(Failure::new(
            FailureKind::DivisionByZero,
            "division by zero",
            span,
        ))
    }

    /// An `R0002` overflow.
    fn overflow(&self, message: impl Into<String>, span: Span) -> Box<Failure> {
        Box::new(Failure::new(FailureKind::Overflow, message.into(), span))
    }

    /// An `R0003` index error.
    fn index_error(&self, index: i64, length: usize, span: Span) -> Box<Failure> {
        Box::new(Failure::new(
            FailureKind::IndexOutOfBounds,
            format!("index {index} is out of bounds for a collection of length {length}"),
            span,
        ))
    }

    /// A missing map key.
    fn missing_key(&self, key: MapKey, span: Span) -> Box<Failure> {
        Box::new(Failure::new(
            FailureKind::MissingKey,
            format!("key `{key}` not found"),
            span,
        ))
    }
}

/// The value of a literal.
///
/// Interpolation parts are not evaluated in the alpha (ADR 0012): [`StrPart::Lit`]
/// text is concatenated and `{...}` parts are kept verbatim, so the program's
/// output still shows what was written.
///
/// [`StrPart::Lit`]: orv_syntax::StrPart::Lit
fn literal_value(literal: &Literal) -> Value {
    match literal {
        Literal::Int(value) => Value::Int(*value),
        Literal::Float(value) => Value::Float(*value),
        Literal::Bool(value) => Value::Bool(*value),
        Literal::Str(parts) => {
            let mut text = String::new();
            for part in parts {
                match part {
                    orv_syntax::StrPart::Lit(lit) => text.push_str(lit),
                    orv_syntax::StrPart::Expr { src, .. } => {
                        text.push('{');
                        text.push_str(src);
                        text.push('}');
                    }
                }
            }
            Value::str(text)
        }
        Literal::None => Value::None,
    }
}

/// A float view of a numeric value.
fn as_float(value: &Value) -> Option<f64> {
    match value {
        Value::Int(value) => Some(*value as f64),
        Value::Float(value) => Some(*value),
        _ => None,
    }
}

/// Normalizes a possibly negative index against a length.
///
/// Negative indices count from the end, matching the usual convention.
fn normalize_index(index: i64, len: usize) -> Option<usize> {
    let len = len as i64;
    let resolved = if index < 0 { len + index } else { index };
    if resolved < 0 || resolved >= len {
        None
    } else {
        usize::try_from(resolved).ok()
    }
}

/// The integers in a range.
///
/// The span is capped so a runaway literal cannot exhaust memory silently.
fn int_range(start: i64, end: i64, inclusive: bool) -> Vec<Value> {
    const MAX_RANGE: usize = 10_000_000;
    let mut values = Vec::new();
    let mut current = start;
    let in_range = |value: i64| if inclusive { value <= end } else { value < end };
    while in_range(current) && values.len() < MAX_RANGE {
        values.push(Value::Int(current));
        match current.checked_add(1) {
            Some(next) => current = next,
            // An inclusive range ending at `i64::MAX` stops instead of wrapping.
            None => break,
        }
    }
    values
}
