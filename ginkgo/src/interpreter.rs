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
        expect_receiver: bool,
        /// The object that 'receives' the method call. This is only relevant for methods that take a `self` parameter, not functions, where this should be `None`.
        receiver: Option<Box<Value>>,
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    NativeFunction(usize),
    Class(usize),
    Instance {
        class: usize,
        instance: usize,
    },
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Integer(value) => write!(f, "{}", value),
            Value::Bool(value) => write!(f, "{}", value),
            Value::String(value) => write!(f, "\"{}\"", value),
            Value::Function { .. } => write!(f, "[function]"),
            Value::NativeFunction(index) => write!(f, "[native function({})]", index),
            Value::Class(index) => write!(f, "[class({})]", index),
            Value::Instance { .. } => write!(f, "[instance]"),
        }
    }
}

pub struct Class {
    name: String,
    defs: BTreeMap<String, Value>,
}

pub struct Instance {
    properties: BTreeMap<String, Value>,
}

pub struct Interpreter<'a> {
    globals: Arc<RefCell<Environment>>,
    environment: Arc<RefCell<Environment>>,
    /*
     * We keep track of various things as indexed lists. We don't support removing elements of any
     * of these, as that would invalidate indices.
     * TODO: that doesn't really work long term for instances does it. Probs need a nicer way or at
     * least a generational arena for them.
     */
    native_fns: Vec<Box<dyn Fn(Vec<Value>) -> Value + 'a>>,
    classes: Vec<Class>,
    instances: Vec<Instance>,
}

// TODO: this is probably bad (it isn't true by default bc RefCell)
unsafe impl Send for Interpreter<'_> {}
unsafe impl Sync for Interpreter<'_> {}

#[derive(Clone, PartialEq, Debug)]
pub enum ControlFlow {
    None,
    Yield(Value),
    Return(Value),
}

impl core::ops::Try for ControlFlow {
    type Output = Value;
    type Residual = ControlFlow;

    fn from_output(output: Self::Output) -> Self {
        todo!()
    }

    fn branch(self) -> std::ops::ControlFlow<Self::Residual, Self::Output> {
        match self {
            ControlFlow::None => std::ops::ControlFlow::Continue(Value::Unit),
            ControlFlow::Yield(value) => std::ops::ControlFlow::Continue(value),
            ControlFlow::Return(value) => std::ops::ControlFlow::Break(ControlFlow::Return(value)),
        }
    }
}

impl core::ops::FromResidual for ControlFlow {
    fn from_residual(residual: <Self as std::ops::Try>::Residual) -> Self {
        residual
    }
}

impl<'a> Interpreter<'a> {
    pub fn new() -> Interpreter<'a> {
        let globals = Environment::new();
        Interpreter {
            globals: globals.clone(),
            environment: globals,
            native_fns: Vec::new(),
            classes: Vec::new(),
            instances: Vec::new(),
        }
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

    pub fn eval_block(&mut self, statements: Vec<Stmt>, environment: Arc<RefCell<Environment>>) -> ControlFlow {
        let previous_environment = mem::replace(&mut self.environment, environment);

        let mut statements = statements.into_iter();
        let mut result = ControlFlow::Yield(Value::Unit);
        while let Some(next) = statements.next() {
            match self.eval_stmt(next) {
                ControlFlow::None => (),
                ControlFlow::Yield(value) => {
                    /*
                     * Only the last statement is allowed to return a value. If there are more
                     * statements after this one, issue an error.
                     */
                    if !statements.next().is_none() {
                        // TODO: runtime error instead of panic
                        panic!("Non-terminated statement is not last. Value = {:#?}", value);
                    }
                    result = ControlFlow::Yield(value);
                    break;
                }
                ControlFlow::Return(value) => {
                    result = ControlFlow::Return(value);
                    break;
                }
            }
        }

        self.environment = previous_environment;
        result
    }

    pub fn eval_stmt(&mut self, stmt: Stmt) -> ControlFlow {
        match stmt.typ {
            StmtTyp::Expression(expr) => {
                let result = self.eval_expr(expr)?;
                ControlFlow::Yield(result)
            }
            StmtTyp::TerminatedExpression(expr) => {
                self.eval_expr(expr)?;
                ControlFlow::None
            }
            StmtTyp::Let { name, expression } => {
                let value = self.eval_expr(expression)?;
                self.environment.borrow_mut().define(name, value);
                ControlFlow::None
            }
            StmtTyp::FnDef { name, takes_self, params, body } => {
                self.environment
                    .borrow_mut()
                    .define(name, Value::Function { expect_receiver: takes_self, receiver: None, params, body });
                ControlFlow::None
            }
            StmtTyp::ClassDef { name, defs } => {
                let defs = defs
                    .into_iter()
                    .filter_map(|def| match def.typ {
                        StmtTyp::FnDef { name, takes_self, params, body } => Some((
                            name,
                            Value::Function { expect_receiver: takes_self, receiver: None, params, body },
                        )),
                        _ => {
                            panic!("Unexpected statement in class definition");
                            None
                        }
                    })
                    .collect();

                let index = self.classes.len();
                self.classes.push(Class { name: name.clone(), defs });
                self.environment.borrow_mut().define(name, Value::Class(index));
                ControlFlow::None
            }
            StmtTyp::Block(statements) => {
                self.eval_block(statements, Environment::new_with_parent(self.environment.clone()))
            }
            StmtTyp::If { condition, then_block, else_block } => {
                if let Value::Bool(truthy) = self.eval_expr(condition)? {
                    if truthy {
                        self.eval_stmt(*then_block)
                    } else if let Some(else_block) = else_block {
                        self.eval_stmt(*else_block)
                    } else {
                        ControlFlow::None
                    }
                } else {
                    panic!("Condition of `if` must be a bool");
                }
            }
            StmtTyp::While { condition, body } => {
                let body = *body;
                while let Value::Bool(truthy) = self.eval_expr(condition.clone())?
                    && truthy
                {
                    self.eval_stmt(body.clone());
                }
                ControlFlow::None
            }
            StmtTyp::Return { value } => ControlFlow::Return(self.eval_expr(value)?),
        }
    }

    pub fn eval_expr(&mut self, expr: Expr) -> ControlFlow {
        match expr.typ {
            ExprTyp::Literal(value) => ControlFlow::Yield(value.clone()),
            ExprTyp::Identifier { name, resolution } => {
                if let Some(value) = self.resolve_binding(&name, resolution) {
                    ControlFlow::Yield(value.clone())
                } else {
                    panic!("Failed to get value for binding called '{}'. Either it does not exist (or dies before a function referencing it, bc we don't do any borrow checking :))", name);
                }
            }
            ExprTyp::GinkgoSelf { resolution } => {
                if let Some(value) = self.resolve_binding("self", resolution) {
                    ControlFlow::Yield(value.clone())
                } else {
                    panic!("Failed to get `self`");
                }
            }
            ExprTyp::UnaryOp { op, operand } => {
                let operand = self.eval_expr(*operand)?;
                match op {
                    UnaryOp::Plus => ControlFlow::Yield(operand),
                    UnaryOp::Negate => {
                        if let Value::Integer(value) = operand {
                            ControlFlow::Yield(Value::Integer(-value))
                        } else {
                            panic!()
                        }
                    }
                    UnaryOp::Not => {
                        if let Value::Bool(value) = operand {
                            ControlFlow::Yield(Value::Bool(!value))
                        } else {
                            panic!()
                        }
                    }
                }
            }
            ExprTyp::BinaryOp { op, left, right } => {
                let left = self.eval_expr(*left)?;
                let right = self.eval_expr(*right)?;
                match op {
                    BinaryOp::Add => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Integer(left + right))
                        } else {
                            panic!();
                        }
                    }
                    BinaryOp::Subtract => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Integer(left - right))
                        } else {
                            panic!();
                        }
                    }
                    BinaryOp::Multiply => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Integer(left * right))
                        } else {
                            panic!();
                        }
                    }
                    BinaryOp::Divide => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Integer(left / right))
                        } else {
                            panic!();
                        }
                    }
                    BinaryOp::BitwiseAnd => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Integer(left & right))
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::BitwiseOr => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Integer(left | right))
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::BitwiseXor => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Integer(left ^ right))
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::Equal => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Bool(left == right))
                        } else if let Value::String(left) = left
                            && let Value::String(right) = right
                        {
                            ControlFlow::Yield(Value::Bool(left == right))
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::NotEqual => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Bool(left != right))
                        } else if let Value::String(left) = left
                            && let Value::String(right) = right
                        {
                            ControlFlow::Yield(Value::Bool(left != right))
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::GreaterThan => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Bool(left > right))
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::GreaterEqual => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Bool(left >= right))
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::LessThan => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Bool(left < right))
                        } else {
                            panic!()
                        }
                    }
                    BinaryOp::LessEqual => {
                        if let Value::Integer(left) = left
                            && let Value::Integer(right) = right
                        {
                            ControlFlow::Yield(Value::Bool(left <= right))
                        } else {
                            panic!()
                        }
                    }
                }
            }
            ExprTyp::LogicalOp { op, left, right } => {
                let Value::Bool(left) = self.eval_expr(*left)? else { panic!() };

                match op {
                    LogicalOp::LogicalAnd => {
                        if !left {
                            ControlFlow::Yield(Value::Bool(false))
                        } else {
                            let Value::Bool(right) = self.eval_expr(*right)? else { panic!() };
                            ControlFlow::Yield(Value::Bool(left && right))
                        }
                    }
                    LogicalOp::LogicalOr => {
                        if left {
                            ControlFlow::Yield(Value::Bool(true))
                        } else {
                            let Value::Bool(right) = self.eval_expr(*right)? else { panic!() };
                            ControlFlow::Yield(Value::Bool(left || right))
                        }
                    }
                }
            }
            ExprTyp::Grouping { inner } => self.eval_expr(*inner),
            ExprTyp::Assign { place, expr } => {
                let value = self.eval_expr(*expr)?;
                match place.typ {
                    ExprTyp::Identifier { name, resolution } => match resolution {
                        Resolution::Unresolved => self.globals.borrow_mut().assign(name, 0, value.clone()),
                        Resolution::Local { depth } => {
                            self.environment.borrow_mut().assign(name, depth, value.clone())
                        }
                    },
                    ExprTyp::PropertyAccess { left, property } => {
                        if let Value::Instance { class, instance } = self.eval_expr(*left)? {
                            self.instances.get_mut(instance).unwrap().properties.insert(property, value.clone());
                        } else {
                            panic!("Tried to set property on value that is not an object!");
                        }
                    }
                    _ => panic!("Invalid place: {:?}", place.typ),
                }
                ControlFlow::Yield(value)
            }
            ExprTyp::Function { takes_self, body, params } => {
                ControlFlow::Yield(Value::Function { expect_receiver: takes_self, receiver: None, params, body })
            }
            ExprTyp::Call { left, params } => {
                match self.eval_expr(*left)? {
                    Value::Function { expect_receiver, receiver, params: param_defs, body } => {
                        let environment = Environment::new_with_parent(self.environment.clone());

                        /*
                         * If this is a method, introduce a `self` binding to the instance the method is invoked on.
                         */
                        if expect_receiver {
                            environment.borrow_mut().define(
                                "self".to_string(),
                                *receiver.expect("No method receiver despite expecting one!"),
                            );
                        }

                        /*
                         * For each parameter expected, take the next param supplied and bind it to the
                         * correct name.
                         */
                        assert_eq!(param_defs.len(), params.len());
                        let mut params = params.into_iter();
                        for param_def in param_defs {
                            environment.borrow_mut().define(param_def, self.eval_expr(params.next().unwrap())?);
                        }

                        /*
                         * When we call a function, we want to terminate the propagation of its return at the call-site.
                         */
                        match self.eval_block(body, environment) {
                            ControlFlow::None => ControlFlow::Yield(Value::Unit),
                            ControlFlow::Yield(value) => ControlFlow::Yield(value),
                            ControlFlow::Return(value) => ControlFlow::Yield(value),
                        }
                    }
                    Value::NativeFunction(index) => {
                        /*
                         * Native functions do not define how many parameters they wish to accept. This
                         * allows them to accept varying numbers of parameters, which is not a feature
                         * supported in Ginkgo itself yet, but is useful.
                         */
                        let mut evaluated_params = Vec::new();
                        for param in params {
                            evaluated_params.push(self.eval_expr(param)?);
                        }
                        let function = &self.native_fns[index];
                        ControlFlow::Yield(function(evaluated_params))
                    }
                    Value::Class(index) => {
                        /*
                         * "Calling" a class instantiates an instance of it.
                         */
                        let instance_index = self.instances.len();
                        self.instances.push(Instance { properties: BTreeMap::new() });
                        ControlFlow::Yield(Value::Instance { class: index, instance: instance_index })
                    }
                    other => {
                        panic!("Tried to call non-callable value: {:?}", other);
                    }
                }
            }
            ExprTyp::PropertyAccess { left, property } => {
                if let Value::Instance { class, instance } = self.eval_expr(*left)? {
                    /*
                     * When accessing a property, check for fields on the instance, and then definitions on the class. This means fields on the instance can shadow class definitions.
                     */
                    if let Some(property) = self
                        .instances
                        .get(instance)
                        .unwrap()
                        .properties
                        .get(&property)
                        .or_else(|| self.classes.get(class).unwrap().defs.get(&property))
                    {
                        /*
                         * If the accessed property is a method that expects a receiver, return a version of the function with the correct receiver already bound.
                         */
                        if let Value::Function { expect_receiver: true, receiver, params, body } = property {
                            assert!(receiver.is_none());
                            ControlFlow::Yield(Value::Function {
                                expect_receiver: true,
                                receiver: Some(Box::new(Value::Instance { class, instance })),
                                params: params.clone(),
                                body: body.clone(),
                            })
                        } else {
                            ControlFlow::Yield(property.clone())
                        }
                    } else {
                        panic!("Tried to access property '{}' on object but it does not exist!", property);
                    }
                } else {
                    panic!("Tried to access property on value that is not an object!");
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

#[derive(Debug)]
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
