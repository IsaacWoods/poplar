use core::fmt;

#[derive(Clone, PartialEq)]
pub enum AstNode {
    Integer(isize),
    Bool(bool),
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
            AstNode::Bool(value) => write!(f, "{}", value),
            AstNode::Integer(value) => write!(f, "{}", value),
            AstNode::Identifier(value) => write!(f, "{}", value),
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
            left: Box::new(AstNode::UnaryOp { op: UnaryOp::Negate, operand: Box::new(AstNode::Integer(4)) }),
            right: Box::new(AstNode::Integer(19)),
        };
        println!("{}", ast);
        panic!();
    }
}
