use crate::{
    ast::{BinaryOp, Expr, ExprTyp, LogicalOp, Resolution, Stmt, StmtTyp, UnaryOp},
    interpreter::Value,
    lex::{Lex, PeekingIter, Token, TokenType, TokenValue},
};
use std::collections::BTreeMap;

pub struct Parser<'s> {
    stream: PeekingIter<Lex<'s>>,

    prefix_parselets: BTreeMap<TokenType, PrefixParselet>,
    infix_parselets: BTreeMap<TokenType, InfixParselet>,
    precedence: BTreeMap<TokenType, u8>,
}

impl<'s> Parser<'s> {
    pub fn new(source: &'s str) -> Parser<'s> {
        let lex = PeekingIter::new(Lex::new(source));
        let mut parser = Parser {
            stream: lex,
            prefix_parselets: BTreeMap::new(),
            infix_parselets: BTreeMap::new(),
            precedence: BTreeMap::new(),
        };
        parser.register_parselets();

        parser
    }

    pub fn parse(mut self) -> Result<Vec<Stmt>, ()> {
        let mut statements = Vec::new();
        while self.stream.peek().is_some() {
            statements.push(self.statement());
        }
        Ok(statements)
    }

    pub fn statement(&mut self) -> Stmt {
        if self.matches(TokenType::Let) {
            let name = self.identifier();
            self.consume(TokenType::Equals);
            let expression = self.expression(0);
            self.consume(TokenType::Semicolon);
            return Stmt::new_let(name, expression);
        }

        if self.matches(TokenType::Fn) {
            return self.function();
        }

        if self.matches(TokenType::Class) {
            let name = self.identifier();
            self.consume(TokenType::LeftBrace);

            let mut defs = Vec::new();

            while self.matches(TokenType::Fn) {
                defs.push(self.function());
            }

            self.consume(TokenType::RightBrace);
            return Stmt { typ: StmtTyp::ClassDef { name } };
        }

        // TODO: in the future, we want expressions to be able to do this too (so it can probs move
        // into there)
        if self.matches(TokenType::LeftBrace) {
            let mut statements = Vec::new();
            while !self.matches(TokenType::RightBrace) {
                statements.push(self.statement());
            }
            return Stmt::new_block(statements);
        }

        // TODO: I think we want `if`s to be expressions too actually? (with optional `else` if the
        // then branch returns unit).
        if self.matches(TokenType::If) {
            let condition = self.expression(0);
            self.consume(TokenType::LeftBrace);
            let mut then_block = Vec::new();
            while !self.matches(TokenType::RightBrace) {
                then_block.push(self.statement());
            }
            let then_block = Stmt::new_block(then_block);

            let else_block = if self.matches(TokenType::Else) {
                self.consume(TokenType::LeftBrace);
                let mut statements = Vec::new();
                while !self.matches(TokenType::RightBrace) {
                    statements.push(self.statement());
                }
                Some(Stmt::new_block(statements))
            } else {
                None
            };

            return Stmt::new_if(condition, then_block, else_block);
        }

        if self.matches(TokenType::While) {
            let condition = self.expression(0);
            self.consume(TokenType::LeftBrace);
            let mut body = Vec::new();
            while !self.matches(TokenType::RightBrace) {
                body.push(self.statement());
            }

            return Stmt::new_while(condition, Stmt::new_block(body));
        }

        /*
         * Default case - it's an expression statement.
         * Expressions in statement position may or may not be terminated with a semicolon, so we
         * handle both cases here.
         */
        let expression = self.expression(0);
        if self.matches(TokenType::Semicolon) {
            Stmt::new_terminated_expr(expression)
        } else {
            Stmt::new_expr(expression)
        }
    }

    pub fn function(&mut self) -> Stmt {
        let name = self.identifier();
        self.consume(TokenType::LeftParen);

        /*
         * Only the first parameter can be `self`, and it can only occur inside classes.
         * TODO: check here or in the resolver whether we're in the right place for a self?
         */
        let takes_self = if self.matches(TokenType::GinkgoSelf) {
            self.matches(TokenType::Comma);
            true
        } else {
            false
        };

        let mut params = Vec::new();
        while !self.matches(TokenType::RightParen) {
            // TODO: not sure if we want to parse expressions or something simpler (e.g. could
            // just be idents for now, but might want more complex (e.g. patterns) in the
            // future.
            let param = self.expression(0);
            if let ExprTyp::Identifier { name, .. } = param.typ {
                params.push(name);
            } else {
                panic!("Invalid param name");
            }
            self.matches(TokenType::Comma);
        }

        self.consume(TokenType::LeftBrace);

        let mut statements = Vec::new();
        while !self.matches(TokenType::RightBrace) {
            statements.push(self.statement());
        }
        Stmt::new_fn_def(name, takes_self, params, statements)
    }

    pub fn identifier(&mut self) -> String {
        let token = self.consume(TokenType::Identifier).unwrap();
        if let Some(TokenValue::Identifier(name)) = self.stream.inner.token_value(token) {
            name.to_string()
        } else {
            // TODO: report error properly - this shouldn't even be reachable so is probs
            // acc an ICE??
            panic!();
        }
    }

    pub fn expression(&mut self, precedence: u8) -> Expr {
        let token = self.stream.next().unwrap();

        /*
         * Start by parsing a prefix operator. Identifiers and literals both have prefix parselets,
         * so are parsed correctly if there is no 'real' prefix operator.
         */
        let Some(prefix) = self.prefix_for(token.typ) else {
            panic!("No prefix parselet for token: {:?}", token.typ);
        };
        let mut left = (prefix)(self, token);

        /*
         * Check if the next token, if it exists, represents a valid infix operator that we can
         * parse at the current precedence level. If not, or if it has higher precedence than we're
         * currently allowed to parse, just return the current expression.
         */
        while {
            self.stream.peek().map_or(false, |next| {
                self.precedence_for(next.typ).map_or(false, |next_precedence| precedence < next_precedence)
            })
        } {
            let next = self.stream.next().unwrap();
            let Some(infix) = self.infix_for(next.typ) else {
                panic!("No infix parselet for token: {:?}", token.typ);
            };
            left = (infix)(self, left, next);
        }

        left
    }
}

/*
 * Expression parselets.
 */
impl<'s> Parser<'s> {
    fn register_parselets(&mut self) {
        const PRECEDENCE_ASSIGNMENT: u8 = 1;
        const PRECEDENCE_LOGICAL_OR: u8 = 2;
        const PRECEDENCE_LOGICAL_AND: u8 = 3;
        const PRECEDENCE_CONDITIONAL: u8 = 4;
        const PRECEDENCE_BITWISE_OR: u8 = 5;
        const PRECEDENCE_BITWISE_XOR: u8 = 6;
        const PRECEDENCE_BITWISE_AND: u8 = 7;
        const PRECEDENCE_SUM: u8 = 8;
        const PRECEDENCE_PRODUCT: u8 = 9;
        const PRECEDENCE_EXPONENT: u8 = 10;
        const PRECEDENCE_PREFIX: u8 = 11;
        const PRECEDENCE_POSTFIX: u8 = 12;
        const PRECEDENCE_CALL: u8 = 13;

        /*
         * Literals and identifiers and consumed as prefix operations.
         */
        self.register_prefix(TokenType::Identifier, |parser, token| {
            let value = match parser.stream.inner.token_value(token) {
                Some(TokenValue::Identifier(value)) => value,
                _ => unreachable!(),
            };
            Expr::new(ExprTyp::Identifier { name: value.to_string(), resolution: Resolution::Unresolved })
        });
        self.register_prefix(TokenType::Integer, |parser, token| {
            let value = match parser.stream.inner.token_value(token) {
                Some(TokenValue::Integer(value)) => value,
                _ => unreachable!(),
            };
            Expr::new(ExprTyp::Literal(Value::Integer(value)))
        });
        self.register_prefix(TokenType::String, |parser, token| {
            let value = match parser.stream.inner.token_value(token) {
                Some(TokenValue::String(value)) => value.to_string(),
                _ => unreachable!(),
            };
            Expr::new(ExprTyp::Literal(Value::String(value)))
        });
        let bool_literal: PrefixParselet = |_parser, token| {
            Expr::new(ExprTyp::Literal(match token.typ {
                TokenType::True => Value::Bool(true),
                TokenType::False => Value::Bool(false),
                _ => unreachable!(),
            }))
        };
        self.register_prefix(TokenType::True, bool_literal);
        self.register_prefix(TokenType::False, bool_literal);

        /*
         * 'Real' prefix operations.
         */
        self.register_prefix(TokenType::Minus, |parser, _token| {
            let operand = parser.expression(PRECEDENCE_PREFIX);
            Expr::new(ExprTyp::UnaryOp { op: UnaryOp::Negate, operand: Box::new(operand) })
        });
        self.register_prefix(TokenType::LeftParen, |parser, _token| {
            let inner = parser.expression(0);
            parser.consume(TokenType::RightParen);
            Expr::new(ExprTyp::Grouping { inner: Box::new(inner) })
        });

        /*
         * Function definitions are also prefix operations.
         */
        self.register_prefix(TokenType::Fn, |parser, _token| {
            parser.consume(TokenType::LeftParen);

            /*
             * Only the first parameter can be `self`, and it can only occur inside classes.
             */
            let takes_self = if parser.matches(TokenType::GinkgoSelf) {
                parser.matches(TokenType::Comma);
                true
            } else {
                false
            };

            let mut params = Vec::new();
            while !parser.matches(TokenType::RightParen) {
                // TODO: not sure if we want to parse expressions or something simpler (e.g. could
                // just be idents for now, but might want more complex (e.g. patterns) in the
                // future.
                let param = parser.expression(0);
                if let ExprTyp::Identifier { name, .. } = param.typ {
                    params.push(name);
                } else {
                    panic!("Invalid param name");
                }
                parser.matches(TokenType::Comma);
            }

            parser.consume(TokenType::LeftBrace);

            let mut statements = Vec::new();
            while !parser.matches(TokenType::RightBrace) {
                statements.push(parser.statement());
            }

            Expr::new(ExprTyp::Function { takes_self, body: statements, params })
        });

        /*
         * Infix operations.
         */
        let binary_op: InfixParselet = |parser, left, token| {
            let (op, precedence) = match token.typ {
                TokenType::Plus => (BinaryOp::Add, PRECEDENCE_SUM),
                TokenType::Minus => (BinaryOp::Subtract, PRECEDENCE_SUM),
                TokenType::Asterix => (BinaryOp::Multiply, PRECEDENCE_PRODUCT),
                TokenType::Slash => (BinaryOp::Divide, PRECEDENCE_PRODUCT),
                TokenType::Ampersand => (BinaryOp::BitwiseAnd, PRECEDENCE_BITWISE_AND),
                TokenType::Pipe => (BinaryOp::BitwiseOr, PRECEDENCE_BITWISE_OR),
                TokenType::Caret => (BinaryOp::BitwiseXor, PRECEDENCE_BITWISE_XOR),
                TokenType::EqualEquals => (BinaryOp::Equal, PRECEDENCE_CONDITIONAL),
                TokenType::BangEquals => (BinaryOp::NotEqual, PRECEDENCE_CONDITIONAL),
                TokenType::GreaterThan => (BinaryOp::GreaterThan, PRECEDENCE_CONDITIONAL),
                TokenType::GreaterEqual => (BinaryOp::GreaterEqual, PRECEDENCE_CONDITIONAL),
                TokenType::LessThan => (BinaryOp::LessThan, PRECEDENCE_CONDITIONAL),
                TokenType::LessEqual => (BinaryOp::LessEqual, PRECEDENCE_CONDITIONAL),
                other => panic!("Unsupported binary op token: {:?}", other),
            };
            let right = parser.expression(precedence);
            Expr::new(ExprTyp::BinaryOp { op, left: Box::new(left), right: Box::new(right) })
        };
        self.register_infix(TokenType::Plus, PRECEDENCE_SUM, binary_op);
        self.register_infix(TokenType::Minus, PRECEDENCE_SUM, binary_op);
        self.register_infix(TokenType::Asterix, PRECEDENCE_PRODUCT, binary_op);
        self.register_infix(TokenType::Slash, PRECEDENCE_PRODUCT, binary_op);
        self.register_infix(TokenType::Ampersand, PRECEDENCE_BITWISE_AND, binary_op);
        self.register_infix(TokenType::Pipe, PRECEDENCE_BITWISE_OR, binary_op);
        self.register_infix(TokenType::Caret, PRECEDENCE_BITWISE_XOR, binary_op);
        self.register_infix(TokenType::EqualEquals, PRECEDENCE_CONDITIONAL, binary_op);
        self.register_infix(TokenType::BangEquals, PRECEDENCE_CONDITIONAL, binary_op);
        self.register_infix(TokenType::GreaterThan, PRECEDENCE_CONDITIONAL, binary_op);
        self.register_infix(TokenType::GreaterEqual, PRECEDENCE_CONDITIONAL, binary_op);
        self.register_infix(TokenType::LessThan, PRECEDENCE_CONDITIONAL, binary_op);
        self.register_infix(TokenType::LessEqual, PRECEDENCE_CONDITIONAL, binary_op);

        let logical_op: InfixParselet = |parser, left, token| {
            let (op, precedence) = match token.typ {
                TokenType::AmpersandAmpersand => (LogicalOp::LogicalAnd, PRECEDENCE_LOGICAL_AND),
                TokenType::PipePipe => (LogicalOp::LogicalOr, PRECEDENCE_LOGICAL_OR),
                other => panic!("Unsupported logical op token: {:?}", other),
            };
            let right = parser.expression(precedence);
            Expr::new(ExprTyp::LogicalOp { op, left: Box::new(left), right: Box::new(right) })
        };
        self.register_infix(TokenType::AmpersandAmpersand, PRECEDENCE_LOGICAL_AND, logical_op);
        self.register_infix(TokenType::PipePipe, PRECEDENCE_LOGICAL_OR, logical_op);

        /*
         * Assignment.
         */
        self.register_infix(TokenType::Equals, PRECEDENCE_ASSIGNMENT, |parser, left, _token| {
            let expr = parser.expression(PRECEDENCE_ASSIGNMENT - 1);
            Expr::new(ExprTyp::Assign { place: Box::new(left), expr: Box::new(expr) })
        });

        /*
         * Function calls.
         */
        self.register_infix(TokenType::LeftParen, PRECEDENCE_CALL, |parser, left, _token| {
            let mut params = Vec::new();
            while !parser.matches(TokenType::RightParen) {
                let param = parser.expression(0);
                parser.matches(TokenType::Comma);
                params.push(param);
            }

            Expr::new(ExprTyp::Call { left: Box::new(left), params })
        });
    }
}

/*
 * Parser utilities.
 */
type PrefixParselet = fn(&mut Parser, Token) -> Expr;
type InfixParselet = fn(&mut Parser, Expr, Token) -> Expr;

impl<'s> Parser<'s> {
    pub fn matches(&mut self, typ: TokenType) -> bool {
        if let Some(token) = self.stream.peek() {
            if token.typ == typ {
                self.stream.next();
                return true;
            }
        }
        false
    }

    /// Expect a token of the given type, issuing a parse error if the next token is not of the
    /// expected type.
    pub fn consume(&mut self, typ: TokenType) -> Option<Token> {
        let token = self.stream.next();
        if token.is_none() || token.unwrap().typ != typ {
            // TODO: real error
            // TODO: for possible recovery, should we consume the token or not??
            // println!("Parse error: expected token of type {:?}", typ);
            panic!("Parse error: expected token of type {:?}", typ);
        }
        token
    }

    pub fn register_prefix(&mut self, token: TokenType, parselet: PrefixParselet) {
        self.prefix_parselets.insert(token, parselet);
    }

    pub fn register_infix(&mut self, token: TokenType, precedence: u8, parselet: InfixParselet) {
        self.infix_parselets.insert(token, parselet);
        self.precedence.insert(token, precedence);
    }

    pub fn prefix_for(&self, token: TokenType) -> Option<PrefixParselet> {
        self.prefix_parselets.get(&token).map(|x| *x)
    }

    pub fn precedence_for(&self, token: TokenType) -> Option<u8> {
        self.precedence.get(&token).map(|x| *x)
    }

    pub fn infix_for(&self, token: TokenType) -> Option<InfixParselet> {
        self.infix_parselets.get(&token).map(|x| *x)
    }
}
