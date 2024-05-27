use crate::interpreter::Value;
use std::fmt;

#[derive(Clone, PartialEq, Debug)]
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
    If {
        condition: Expr,
        then_block: Box<Stmt>,
        else_block: Option<Box<Stmt>>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
}

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expression(expr) => writeln!(f, "({})", expr),
            Self::TerminatedExpression(expr) => writeln!(f, "({});", expr),
            Self::Print { expression } => writeln!(f, "(print {})", expression),
            Self::Let { name, expression } => writeln!(f, "(let {} = {})", name, expression),
            Self::Block(stmts) => {
                // TODO: this won't handle nesting well
                writeln!(f, "{{")?;
                for stmt in stmts {
                    writeln!(f, "    {}", stmt)?;
                }
                writeln!(f, "}}")?;
                Ok(())
            }
            Self::If { condition, then_block, else_block } => {
                writeln!(f, "(if {}", condition)?;
                write!(f, "{}", then_block)?;
                if let Some(else_block) = else_block {
                    write!(f, "{}", else_block)?;
                }
                writeln!(f, ")")?;
                Ok(())
            }
            Self::While { condition, body } => {
                writeln!(f, "(while {}", condition)?;
                write!(f, "{}", body)?;
                writeln!(f, ")")?;
                Ok(())
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
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
    Function {
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    Call {
        left: Box<Expr>,
        params: Vec<Expr>,
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
    Equal,
    NotEqual,
    GreaterThan,
    GreaterEqual,
    LessThan,
    LessEqual,
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
                Value::Function { .. } => write!(f, "[function]"),
                Value::Unit => write!(f, "[unit]"),
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
                BinaryOp::Equal => write!(f, "(== {} {})", left, right),
                BinaryOp::NotEqual => write!(f, "(!= {} {})", left, right),
                BinaryOp::GreaterThan => write!(f, "(> {} {})", left, right),
                BinaryOp::GreaterEqual => write!(f, "(>= {} {})", left, right),
                BinaryOp::LessThan => write!(f, "(< {} {})", left, right),
                BinaryOp::LessEqual => write!(f, "(<= {} {})", left, right),
            },
            Self::LogicalOp { op, left, right } => match op {
                LogicalOp::LogicalAnd => write!(f, "(&& {} {})", left, right),
                LogicalOp::LogicalOr => write!(f, "(|| {} {})", left, right),
            },
            Self::Grouping { inner } => write!(f, "'(' {} ')'", inner),
            Self::Assign { place, expr } => write!(f, "(= {} {})", place, expr),
            // TODO: maybe print body
            Self::Function { .. } => write!(f, "(fn() [body])"),
            // TODO: print params
            Self::Call { left, .. } => write!(f, "(call {} ([params]))", left),
        }
    }
}
