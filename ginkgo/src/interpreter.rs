use crate::ast::{AstNode, BinaryOp, UnaryOp};

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

    pub fn eval(&mut self, expr: &AstNode) -> Value {
        match expr {
            AstNode::Literal(value) => value.clone(),
            AstNode::Identifier(value) => todo!(),
            AstNode::UnaryOp { op, operand } => {
                let operand = self.eval(operand);
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
            AstNode::BinaryOp { op, left, right } => {
                let left = self.eval(left);
                let right = self.eval(right);
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
            AstNode::Grouping { inner } => self.eval(inner),
        }
    }
}
