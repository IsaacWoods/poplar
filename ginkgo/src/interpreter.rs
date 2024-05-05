use crate::ast::{BinaryOp, Expr, Stmt, UnaryOp};

#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Integer(isize),
    Bool(bool),
    String(String),
}

pub struct Interpreter {}

impl Interpreter {
    pub fn new() -> Interpreter {
        Interpreter {}
    }

    pub fn eval_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expression(expr) => {
                self.eval_expr(expr);
            }
        }
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Value {
        match expr {
            Expr::Literal(value) => value.clone(),
            Expr::Identifier(value) => todo!(),
            Expr::UnaryOp { op, operand } => {
                let operand = self.eval_expr(operand);
                match op {
                    UnaryOp::Plus => operand,
                    UnaryOp::Negate => {
                        if let Value::Integer(value) = operand {
                            Value::Integer(-value)
                        } else {
                            panic!()
                        }
                    }
                    UnaryOp::Not => todo!(),
                }
            }
            Expr::BinaryOp { op, left, right } => {
                let left = self.eval_expr(left);
                let right = self.eval_expr(right);
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
                }
            }
            Expr::Grouping { inner } => self.eval_expr(inner),
        }
    }
}
