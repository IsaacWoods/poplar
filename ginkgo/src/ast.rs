use core::fmt;

#[derive(Clone, PartialEq)]
pub enum AstNode {
    Integer(isize),
    UnaryOp { op: UnaryOp, right: Box<AstNode> },
    BinaryOp { op: BinaryOp, left: Box<AstNode>, right: Box<AstNode> },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum UnaryOp {
    Negate,
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
            Self::UnaryOp { op, right } => match op {
                UnaryOp::Negate => write!(f, "(- {})", right),
                UnaryOp::Not => write!(f, "(! {})", right),
            },
            Self::BinaryOp { op, left, right } => match op {
                BinaryOp::Add => write!(f, "(+ {} {})", left, right),
                BinaryOp::Subtract => write!(f, "(- {} {})", left, right),
                BinaryOp::Multiply => write!(f, "(* {} {})", left, right),
                BinaryOp::Divide => write!(f, "(/ {} {})", left, right),
            },
            AstNode::Integer(value) => write!(f, "{}", value),
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
            left: Box::new(AstNode::UnaryOp { op: UnaryOp::Negate, right: Box::new(AstNode::Integer(4)) }),
            right: Box::new(AstNode::Integer(19)),
        };
        println!("{}", ast);
        panic!();
    }
}
