use crate::interpreter::Value;
use std::fmt;

#[derive(Clone, PartialEq)]
pub enum AstNode {
    Literal(Value),
    Identifier(String),
    UnaryOp { op: UnaryOp, operand: Box<AstNode> },
    BinaryOp { op: BinaryOp, left: Box<AstNode>, right: Box<AstNode> },
    Grouping { inner: Box<AstNode> },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum UnaryOp {
    Negate,
    Plus,
    Not,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl fmt::Display for AstNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AstNode::Literal(value) => match value {
                Value::Integer(value) => write!(f, "{}", value),
                Value::Bool(value) => write!(f, "{}", value),
                Value::String(value) => write!(f, "{}", value),
            },
            AstNode::Identifier(name) => write!(f, "{}", name),
            Self::UnaryOp { op, operand } => match op {
                UnaryOp::Negate => write!(f, "(- {})", operand),
                UnaryOp::Plus => write!(f, "(+ {})", operand),
                UnaryOp::Not => write!(f, "(! {})", operand),
            },
            Self::BinaryOp { op, left, right } => match op {
                BinaryOp::Add => write!(f, "(+ {} {})", left, right),
                BinaryOp::Subtract => write!(f, "(- {} {})", left, right),
                BinaryOp::Multiply => write!(f, "(* {} {})", left, right),
                BinaryOp::Divide => write!(f, "(/ {} {})", left, right),
            },
            Self::Grouping { inner } => write!(f, "'(' {} ')'", inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foo() {
        let ast = AstNode::BinaryOp {
            op: BinaryOp::Add,
            left: Box::new(AstNode::UnaryOp {
                op: UnaryOp::Negate,
                operand: Box::new(AstNode::Literal(Value::Integer(4))),
            }),
            right: Box::new(AstNode::Literal(Value::Integer(19))),
        };
        println!("{}", ast);
        panic!();
    }
}
