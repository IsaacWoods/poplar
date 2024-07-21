use crate::interpreter::Value;
use std::collections::BTreeSet;

#[derive(Clone, PartialEq, Debug)]
pub struct Stmt {
    pub typ: StmtTyp,
}

impl Stmt {
    pub fn new_expr(expr: Expr) -> Stmt {
        Stmt { typ: StmtTyp::Expression(expr) }
    }

    pub fn new_terminated_expr(expr: Expr) -> Stmt {
        Stmt { typ: StmtTyp::TerminatedExpression(expr) }
    }

    pub fn new_let(name: String, expr: Expr) -> Stmt {
        Stmt { typ: StmtTyp::Let { name, expression: expr } }
    }

    pub fn new_fn_def(name: String, params: Vec<String>, body: Vec<Stmt>) -> Stmt {
        Stmt { typ: StmtTyp::FnDef { name, params, body } }
    }

    pub fn new_block(stmts: Vec<Stmt>) -> Stmt {
        Stmt { typ: StmtTyp::Block(stmts) }
    }

    pub fn new_if(condition: Expr, then_block: Stmt, else_block: Option<Stmt>) -> Stmt {
        Stmt {
            typ: StmtTyp::If {
                condition,
                then_block: Box::new(then_block),
                else_block: else_block.map(|stmt| Box::new(stmt)),
            },
        }
    }

    pub fn new_while(condition: Expr, body: Stmt) -> Stmt {
        Stmt { typ: StmtTyp::While { condition, body: Box::new(body) } }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum StmtTyp {
    /// An expression in statement position that is not terminated with a semicolon. This may or
    /// may not be valid depending on position.
    Expression(Expr),
    /// An expression in statement position terminated with a semicolon.
    TerminatedExpression(Expr),
    Let {
        name: String,
        expression: Expr,
    },
    FnDef {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    ClassDef {
        name: String,
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

// impl fmt::Display for StmtTyp {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Self::Expression(expr) => writeln!(f, "({})", expr),
//             Self::TerminatedExpression(expr) => writeln!(f, "({});", expr),
//             Self::Let { name, expression } => writeln!(f, "(let {} = {})", name, expression),
//             Self::Block(stmts) => {
//                 // TODO: this won't handle nesting well
//                 writeln!(f, "{{")?;
//                 for stmt in stmts {
//                     writeln!(f, "    {}", stmt)?;
//                 }
//                 writeln!(f, "}}")?;
//                 Ok(())
//             }
//             Self::If { condition, then_block, else_block } => {
//                 writeln!(f, "(if {}", condition)?;
//                 write!(f, "    {}", then_block)?;
//                 if let Some(else_block) = else_block {
//                     write!(f, "    {}", else_block)?;
//                 }
//                 writeln!(f, ")")?;
//                 Ok(())
//             }
//             Self::While { condition, body } => {
//                 writeln!(f, "(while {}", condition)?;
//                 write!(f, "    {}", body)?;
//                 writeln!(f, ")")?;
//                 Ok(())
//             }
//         }
//     }
// }

#[derive(Clone, PartialEq, Debug)]
pub struct Expr {
    pub typ: ExprTyp,
}

impl Expr {
    pub fn new(typ: ExprTyp) -> Expr {
        Expr { typ }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum ExprTyp {
    Literal(Value),
    Identifier {
        name: String,
        resolution: Resolution,
    },
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

// impl fmt::Display for Expr {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Self::Literal(value) => match value {
//                 Value::Unit => write!(f, "[unit]"),
//                 Value::Integer(value) => write!(f, "{}", value),
//                 Value::Bool(value) => write!(f, "{}", value),
//                 Value::String(value) => write!(f, "{}", value),
//                 Value::Function { .. } => write!(f, "[function]"),
//                 Value::NativeFunction(_) => write!(f, "[native function]"),
//             },
//             Self::Identifier(name) => write!(f, "{}", name),
//             Self::UnaryOp { op, operand } => match op {
//                 UnaryOp::Negate => write!(f, "(- {})", operand),
//                 UnaryOp::Plus => write!(f, "(+ {})", operand),
//                 UnaryOp::Not => write!(f, "(! {})", operand),
//             },
//             Self::BinaryOp { op, left, right } => match op {
//                 BinaryOp::Add => write!(f, "(+ {} {})", left, right),
//                 BinaryOp::Subtract => write!(f, "(- {} {})", left, right),
//                 BinaryOp::Multiply => write!(f, "(* {} {})", left, right),
//                 BinaryOp::Divide => write!(f, "(/ {} {})", left, right),
//                 BinaryOp::BitwiseAnd => write!(f, "(& {} {})", left, right),
//                 BinaryOp::BitwiseOr => write!(f, "(| {} {})", left, right),
//                 BinaryOp::BitwiseXor => write!(f, "(^ {} {})", left, right),
//                 BinaryOp::Equal => write!(f, "(== {} {})", left, right),
//                 BinaryOp::NotEqual => write!(f, "(!= {} {})", left, right),
//                 BinaryOp::GreaterThan => write!(f, "(> {} {})", left, right),
//                 BinaryOp::GreaterEqual => write!(f, "(>= {} {})", left, right),
//                 BinaryOp::LessThan => write!(f, "(< {} {})", left, right),
//                 BinaryOp::LessEqual => write!(f, "(<= {} {})", left, right),
//             },
//             Self::LogicalOp { op, left, right } => match op {
//                 LogicalOp::LogicalAnd => write!(f, "(&& {} {})", left, right),
//                 LogicalOp::LogicalOr => write!(f, "(|| {} {})", left, right),
//             },
//             Self::Grouping { inner } => write!(f, "'(' {} ')'", inner),
//             Self::Assign { place, expr } => write!(f, "(= {} {})", place, expr),
//             // TODO: maybe print body
//             Self::Function { .. } => write!(f, "(fn() [body])"),
//             // TODO: print params
//             Self::Call { left, .. } => write!(f, "(call {} ([params]))", left),
//         }
//     }
// }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Resolution {
    /// A binding is unresolved in a few instances: when binding resolution has not yet occured, or
    /// has happened and this binding is not a local (so may be global) or could not be resolved
    /// (an error will also be produced by the resolver in this case).
    Unresolved,
    Local {
        /// The De-Bruijn index of the binding - how many scopes are there between the use of a
        /// binding and its definition.
        depth: u8,
    },
}

pub struct BindingResolver {
    scopes: Vec<BTreeSet<String>>,
}

impl BindingResolver {
    pub fn new() -> BindingResolver {
        BindingResolver { scopes: Vec::new() }
    }

    pub fn resolve_bindings(&mut self, stmt: &mut Stmt) {
        match stmt.typ {
            StmtTyp::Expression(ref mut expr) => self.resolve_bindings_expr(expr),
            StmtTyp::TerminatedExpression(ref mut expr) => self.resolve_bindings_expr(expr),
            StmtTyp::Let { ref name, ref mut expression } => {
                // Resolve the expression that the binding will be initialised with.
                self.resolve_bindings_expr(expression);

                // Define the new binding
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(name.clone());
                }
            }
            StmtTyp::FnDef { ref params, ref mut body, .. } => {
                self.begin_scope();
                for param in params {
                    self.scopes.last_mut().unwrap().insert(param.clone());
                }
                for stmt in body {
                    self.resolve_bindings(stmt);
                }
                self.end_scope();
            }
            StmtTyp::ClassDef { ref name } => {
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(name.clone());
                }
            }
            StmtTyp::Block(ref mut stmts) => {
                self.begin_scope();
                for stmt in stmts {
                    self.resolve_bindings(stmt);
                }
                self.end_scope();
            }
            StmtTyp::If { ref mut condition, ref mut then_block, ref mut else_block } => {
                self.resolve_bindings_expr(condition);
                self.resolve_bindings(then_block);
                if let Some(else_block) = else_block {
                    self.resolve_bindings(else_block);
                }
            }
            StmtTyp::While { ref mut condition, ref mut body } => {
                self.resolve_bindings_expr(condition);
                self.resolve_bindings(body);
            }
        }
    }

    fn resolve_bindings_expr(&mut self, expr: &mut Expr) {
        match expr.typ {
            ExprTyp::Literal(_) => {}
            ExprTyp::Identifier { ref name, ref mut resolution } => {
                for (i, scope) in self.scopes.iter().enumerate() {
                    if scope.contains(name) {
                        *resolution = Resolution::Local { depth: (self.scopes.len() - i - 1) as u8 };
                        break;
                    }
                }

                // TODO: how to handle globals? Do they even need to be special-cased or just add
                // them as an extra scope at the top? Lox just leaves them unresolved and assumes
                // they're global for interpretation...
            }
            ExprTyp::UnaryOp { ref mut operand, .. } => self.resolve_bindings_expr(operand),
            ExprTyp::BinaryOp { ref mut left, ref mut right, .. } => {
                self.resolve_bindings_expr(left);
                self.resolve_bindings_expr(right)
            }
            ExprTyp::LogicalOp { ref mut left, ref mut right, .. } => {
                self.resolve_bindings_expr(left);
                self.resolve_bindings_expr(right)
            }
            ExprTyp::Grouping { ref mut inner } => self.resolve_bindings_expr(inner),
            ExprTyp::Assign { ref mut place, ref mut expr } => {
                self.resolve_bindings_expr(place);
                self.resolve_bindings_expr(expr);
            }
            ExprTyp::Function { ref mut params, ref mut body } => {
                self.begin_scope();
                for param in params {
                    self.scopes.last_mut().unwrap().insert(param.clone());
                }
                for stmt in body {
                    self.resolve_bindings(stmt);
                }
                self.end_scope();
            }
            ExprTyp::Call { ref mut left, ref mut params } => {
                self.resolve_bindings_expr(left);
                for param in params {
                    self.resolve_bindings_expr(param);
                }
            }
        }
    }

    fn begin_scope(&mut self) {
        self.scopes.push(BTreeSet::new());
    }

    fn end_scope(&mut self) {
        self.scopes.pop();
    }
}
