use crate::interpreter::Value;
use std::fmt;

#[derive(Clone, PartialEq)]
pub enum Stmt {
    /// An expression in statement position that is not terminated with a semicolon. This may or
    /// may not be valid depending on position.
    Expression(Expr),
    /// An expression in statement position terminated with a semicolon.
    TerminatedExpression(Expr),
    Print {
        expression: Expr,
    },
    Let {
        name: String,
        expression: Expr,
    },
    Block(Vec<Stmt>),
}

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expression(expr) => writeln!(f, "({})", expr),
            Self::TerminatedExpression(expr) => writeln!(f, "({});", expr),
            Self::Print { expression } => writeln!(f, "(print {})", expression),
            Self::Let { name, expression } => writeln!(f, "(let {} = {})", name, expression),
            Self::Block(stmts) => {
                // TODO: this is not good
                writeln!(f, "{{")?;
                for stmt in stmts {
                    writeln!(f, "    {}", stmt)?;
                }
                writeln!(f, "}}")?;
                Ok(())
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    Identifier(String),
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// A logical operation. This is implemented separately to `BinaryOp` as it requires different
    /// semantics around short-circuiting.
    LogicalOp {
        op: LogicalOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Grouping {
        inner: Box<Expr>,
    },
    Assign {
        place: Box<Expr>,
        expr: Box<Expr>,
    },
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
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LogicalOp {
    LogicalAnd,
    LogicalOr,
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(value) => match value {
                Value::Integer(value) => write!(f, "{}", value),
                Value::Bool(value) => write!(f, "{}", value),
                Value::String(value) => write!(f, "{}", value),
            },
            Self::Identifier(name) => write!(f, "{}", name),
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
                BinaryOp::BitwiseAnd => write!(f, "(& {} {})", left, right),
                BinaryOp::BitwiseOr => write!(f, "(| {} {})", left, right),
                BinaryOp::BitwiseXor => write!(f, "(^ {} {})", left, right),
            },
            Self::LogicalOp { op, left, right } => match op {
                LogicalOp::LogicalAnd => write!(f, "(&& {} {})", left, right),
                LogicalOp::LogicalOr => write!(f, "(|| {} {})", left, right),
            },
            Self::Grouping { inner } => write!(f, "'(' {} ')'", inner),
            Self::Assign { place, expr } => write!(f, "(= {} {})", place, expr),
        }
    }
}
