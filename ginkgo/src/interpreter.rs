use crate::ast::{BinaryOp, Expr, ExprTyp, LogicalOp, Resolution, Stmt, StmtTyp, UnaryOp};
use core::fmt;
use std::{cell::RefCell, collections::BTreeMap, mem, sync::Arc};

#[derive(Clone, PartialEq, Default, Debug)]
pub enum Value {
    #[default]
    Unit,
    Integer(isize),
    Bool(bool),
    String(String),
    Function {
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    NativeFunction(usize),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Integer(value) => write!(f, "{}", value),
            Value::Bool(value) => write!(f, "{}", value),
            Value::String(value) => write!(f, "\"{}\"", value),
            Value::Function { .. } => write!(f, "[function]"),
            Value::NativeFunction(_) => write!(f, "[native function]"),
        }
    }
}

pub struct Interpreter<'a> {
    globals: Arc<RefCell<Environment>>,
    environment: Arc<RefCell<Environment>>,
    /// XXX: We don't support removing native functions once they're added as that would invalide
    /// indices pointing towards them.
    native_fns: Vec<Box<dyn Fn(Vec<Value>) -> Value + 'a>>,
}

// TODO: this is probably bad (it isn't true by default bc RefCell)
unsafe impl Send for Interpreter<'_> {}
unsafe impl Sync for Interpreter<'_> {}

impl<'a> Interpreter<'a> {
    pub fn new() -> Interpreter<'a> {
        let globals = Environment::new();
        Interpreter { globals: globals.clone(), environment: globals, native_fns: Vec::new() }
    }

    /// Define a native function with the given name as a global.
    pub fn define_native_function<'b, F>(&mut self, name: &str, function: F)
    where
        'b: 'a,
        F: (Fn(Vec<Value>) -> Value) + 'b,
    {
        let index = self.native_fns.len();
        self.native_fns.push(Box::new(function));
        self.globals.borrow_mut().define(name.to_string(), Value::NativeFunction(index));
    }

    pub fn define_global(&mut self, name: &str, value: Value) {
        self.globals.borrow_mut().define(name.to_string(), value);
    }

    pub fn eval_block(&mut self, statements: Vec<Stmt>, environment: Arc<RefCell<Environment>>) -> Option<Value> {
        let previous_environment = mem::replace(&mut self.environment, environment);

        let mut statements = statements.into_iter();
        let mut result = None;
        while let Some(next) = statements.next() {
            if let Some(value) = self.eval_stmt(next) {
                /*
                 * Only the last statement is allowed to return a value. If there are more
                 * statements after this one, issue an error.
                 */
                if !statements.next().is_none() {
                    // TODO: runtime error instead of panic
                    panic!("Non-terminated statement is not last. Value = {:#?}", value);
                }

                result = Some(value);
                break;
            }
        }

        self.environment = previous_environment;
        result
    }

    pub fn eval_stmt(&mut self, stmt: Stmt) -> Option<Value> {
        match stmt.typ {
            StmtTyp::Expression(expr) => {
                let result = self.eval_expr(expr);
                Some(result)
            }
            StmtTyp::TerminatedExpression(expr) => {
                self.eval_expr(expr);
                None
            }
            StmtTyp::Let { name, expression } => {
                let value = self.eval_expr(expression);
                self.environment.borrow_mut().define(name, value);
                None
            }
            StmtTyp::FnDef { name, params, body } => {
                self.environment.borrow_mut().define(name, Value::Function { params, body });
                None
            }
            StmtTyp::Block(statements) => {
                self.eval_block(statements, Environment::new_with_parent(self.environment.clone()))
            }
            StmtTyp::If { condition, then_block, else_block } => {
                if let Value::Bool(truthy) = self.eval_expr(condition) {
                    if truthy {
                        self.eval_stmt(*then_block)
                    } else if let Some(else_block) = else_block {
                        self.eval_stmt(*else_block)
                    } else {
                        None
                    }
                } else {
                    panic!("Condition of `if` must be a bool");
                }
            }
            StmtTyp::While { condition, body } => {
                let body = *body;
                while let Value::Bool(truthy) = self.eval_expr(condition.clone())
                    && truthy
                {
                    self.eval_stmt(body.clone());
                }
                None
            }
        }
    }

    pub fn eval_expr(&mut self, expr: Expr) -> Value {
        match expr.typ {
            ExprTyp::Literal(value) => value.clone(),
            ExprTyp::Identifier { name, resolution } => {
                if let Some(value) = self.resolve_binding(&name, resolution) {
                    value.clone()
                } else {
                    panic!("Failed to get value for binding called '{}'. Either it does not exist (or dies before a function referencing it, bc we don't do any borrow checking :))", name);
                }
            }
            ExprTyp::UnaryOp { op, operand } => {
                let operand = self.eval_expr(*operand);
                match op {
                    UnaryOp::Plus => operand,
                    UnaryOp::Negate => {
                        if let Value::Integer(value) = operand {
                            Value::Integer(-value)
                        } else {
                            panic!()
                        }
                    }
                    UnaryOp::Not => {
                        if let Value::Bool(value) = operand {
                            Value::Bool(!value)
                        } else {
                            panic!()
                        }
                    }
                }
            }
            ExprTyp::BinaryOp { op, left, right } => {
                let left = self.eval_expr(*left);
                let right = self.eval_expr(*right);
                match op {
                    BinaryOp::Add => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Integer(left + right)
                        } else {
                            panic!();
                        }
                    }
                    BinaryOp::Subtract => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Integer(left - right)
                        } else {
                            panic!();
                        }
                    }
                    BinaryOp::Multiply => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Integer(left * right)
                        } else {
                            panic!();
                        }
                    }
                    BinaryOp::Divide => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Integer(left / right)
                        } else {
                            panic!();
                        }
                    }
                    BinaryOp::BitwiseAnd => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Integer(left & right)
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::BitwiseOr => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Integer(left | right)
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::BitwiseXor => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Integer(left ^ right)
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::Equal => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Bool(left == right)
                        } else if let Value::String(left) = left
                            && let Value::String(right) = right
                        {
                            Value::Bool(left == right)
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::NotEqual => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Bool(left != right)
                        } else if let Value::String(left) = left
                            && let Value::String(right) = right
                        {
                            Value::Bool(left != right)
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::GreaterThan => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Bool(left > right)
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::GreaterEqual => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Bool(left >= right)
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::LessThan => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Bool(left < right)
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::LessEqual => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            Value::Bool(left <= right)
                        } else {
                            panic!()
                        }
                    }
                }
            }
            ExprTyp::LogicalOp { op, left, right } => {
                let Value::Bool(left) = self.eval_expr(*left) else { panic!() };

                match op {
                    LogicalOp::LogicalAnd => {
                        if !left {
                            Value::Bool(false)
                        } else {
                            let Value::Bool(right) = self.eval_expr(*right) else { panic!() };
                            Value::Bool(left && right)
                        }
                    }
                    LogicalOp::LogicalOr => {
                        if left {
                            Value::Bool(true)
                        } else {
                            let Value::Bool(right) = self.eval_expr(*right) else { panic!() };
                            Value::Bool(left || right)
                        }
                    }
                }
            }
            ExprTyp::Grouping { inner } => self.eval_expr(*inner),
            ExprTyp::Assign { place, expr } => {
                let value = self.eval_expr(*expr);
                match place.typ {
                    ExprTyp::Identifier { name, resolution } => match resolution {
                        Resolution::Unresolved => self.globals.borrow_mut().assign(name, 0, value.clone()),
                        Resolution::Local { depth } => {
                            self.environment.borrow_mut().assign(name, depth, value.clone())
                        }
                    },
                    _ => panic!("Invalid place"),
                }
                value
            }
            ExprTyp::Function { body, params } => Value::Function { params, body },
            ExprTyp::Call { left, params } => {
                let function = self.eval_expr(*left);

                if let Value::Function { params: param_defs, body } = function {
                    let environment = Environment::new_with_parent(self.environment.clone());

                    /*
                     * For each parameter expected, take the next param supplied and bind it to the
                     * correct name.
                     */
                    assert_eq!(param_defs.len(), params.len());
                    let mut params = params.into_iter();
                    for param_def in param_defs {
                        environment.borrow_mut().define(param_def, self.eval_expr(params.next().unwrap()));
                    }

                    self.eval_block(body, environment).unwrap_or(Value::Unit)
                } else if let Value::NativeFunction(index) = function {
                    /*
                     * Native functions do not define how many parameters they wish to accept. This
                     * allows them to accept varying numbers of parameters, which is not a feature
                     * supported in Ginkgo itself yet, but is useful.
                     */
                    let params = params.into_iter().map(|param| self.eval_expr(param)).collect();
                    let function = &self.native_fns[index];
                    function(params)
                } else {
                    panic!("Tried to call value that isn't a function: {:?}", function);
                }
            }
        }
    }

    pub fn resolve_binding(&self, name: &str, resolution: Resolution) -> Option<Value> {
        match resolution {
            Resolution::Unresolved => {
                /*
                 * The binding has not been resolved. If we've got to interpretation, that should
                 * mean it's global.
                 */
                self.globals.borrow().get(name, 0)
            }
            Resolution::Local { depth } => self.environment.borrow().get(name, depth),
        }
    }
}

pub struct Environment {
    parent: Option<Arc<RefCell<Environment>>>,
    bindings: BTreeMap<String, Value>,
}

impl Environment {
    pub fn new() -> Arc<RefCell<Environment>> {
        Arc::new(RefCell::new(Environment { parent: None, bindings: BTreeMap::new() }))
    }

    pub fn new_with_parent(parent: Arc<RefCell<Environment>>) -> Arc<RefCell<Environment>> {
        Arc::new(RefCell::new(Environment { parent: Some(parent), bindings: BTreeMap::new() }))
    }

    pub fn define(&mut self, name: String, value: Value) {
        self.bindings.insert(name, value);
    }

    pub fn assign(&mut self, name: String, depth: u8, value: Value) {
        if depth == 0 {
            if self.bindings.contains_key(&name) {
                self.bindings.insert(name, value);
            } else {
                panic!("Tried to assign to undefined variable!");
            }
        } else {
            if let Some(parent) = &self.parent {
                parent.borrow_mut().assign(name, depth - 1, value);
            } else {
                panic!("Tried to assign to undefined variable!");
            }
        }
    }

    pub fn get(&self, name: &str, depth: u8) -> Option<Value> {
        if depth == 0 {
            if let Some(value) = self.bindings.get(name) {
                Some(value.clone())
            } else {
                None
            }
        } else {
            if let Some(parent) = &self.parent {
                parent.borrow().get(name, depth - 1)
            } else {
                None
            }
        }
    }
}
