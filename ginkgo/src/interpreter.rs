use crate::ast::{BinaryOp, Expr, LogicalOp, Stmt, UnaryOp};
use std::{cell::RefCell, collections::BTreeMap, mem, sync::Arc};

#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Integer(isize),
    Bool(bool),
    String(String),
}

pub struct Interpreter {
    globals: Arc<RefCell<Environment>>,
    environment: Arc<RefCell<Environment>>,
}

impl Interpreter {
    pub fn new() -> Interpreter {
        let globals = Environment::new();
        Interpreter { globals: globals.clone(), environment: globals }
    }

    pub fn eval_stmt(&mut self, stmt: Stmt) -> Option<Value> {
        match stmt {
            Stmt::Expression(expr) => {
                let result = self.eval_expr(expr);
                Some(result)
            }
            Stmt::TerminatedExpression(expr) => {
                self.eval_expr(expr);
                None
            }
            Stmt::Print { expression } => {
                let result = self.eval_expr(expression);
                println!("PRINT: {:?}", result);
                None
            }
            Stmt::Let { name, expression } => {
                let value = self.eval_expr(expression);
                self.environment.borrow_mut().define(name, value);
                None
            }
            Stmt::Block(statements) => {
                let environment = Environment::new_with_parent(self.environment.clone());
                let previous_environment = mem::replace(&mut self.environment, environment);

                let mut statements = statements.into_iter();
                let mut result = None;
                while let Some(next) = statements.next() {
                    if let Some(value) = self.eval_stmt(next) {
                        /*
                         * Only the last statement is allowed to return a value. If there are more
                         * statements after this one, issue an error.
                         */
                        // TODO: runtime error instead of panic
                        assert!(statements.next().is_none());
                        result = Some(value);
                        break;
                    }
                }

                self.environment = previous_environment;
                result
            }
        }
    }

    pub fn eval_expr(&mut self, expr: Expr) -> Value {
        match expr {
            Expr::Literal(value) => value.clone(),
            Expr::Identifier(name) => self.environment.borrow().get(&name).unwrap().clone(),
            Expr::UnaryOp { op, operand } => {
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
            Expr::BinaryOp { op, left, right } => {
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
                }
            }
            Expr::LogicalOp { op, left, right } => {
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
            Expr::Grouping { inner } => self.eval_expr(*inner),
            Expr::Assign { place, expr } => {
                let value = self.eval_expr(*expr);
                match *place {
                    Expr::Identifier(name) => {
                        self.environment.borrow_mut().assign(name, value.clone());
                    }
                    _ => panic!("Invalid place"),
                }
                value
            }
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

    pub fn assign(&mut self, name: String, value: Value) {
        if self.bindings.contains_key(&name) {
            self.bindings.insert(name, value);
        } else if let Some(parent) = &self.parent {
            parent.borrow_mut().assign(name, value);
        } else {
            // TODO: error here properly
            panic!("Tried to assign to undefined variable!");
        }
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.bindings.get(name) {
            Some(value.clone())
        } else if let Some(parent) = &self.parent {
            parent.borrow().get(name)
        } else {
            None
        }
    }
}
