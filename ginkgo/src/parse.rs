use crate::{
    ast::{AstNode, BinaryOp, UnaryOp},
    lex::{Lex, PeekingIter, Token, TokenType, TokenValue},
};
use std::collections::BTreeMap;

const PRECEDENCE_ASSIGNMENT: u8 = 1;
const PRECEDENCE_CONDITIONAL: u8 = 2;
const PRECEDENCE_SUM: u8 = 3;
const PRECEDENCE_PRODUCT: u8 = 4;
const PRECEDENCE_EXPONENT: u8 = 5;
const PRECEDENCE_PREFIX: u8 = 6;
const PRECEDENCE_POSTFIX: u8 = 7;
const PRECEDENCE_CALL: u8 = 8;

type PrefixParselet = fn(&mut Parser, Token) -> AstNode;
type InfixParselet = fn(&mut Parser, AstNode, Token) -> AstNode;

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

        parser.register_prefix(TokenType::Identifier, |parser, token| {
            let value = match parser.stream.inner.token_value(token) {
                Some(TokenValue::Identifier(value)) => value,
                _ => unreachable!(),
            };
            AstNode::Identifier(value.to_string())
        });
        parser.register_prefix(TokenType::Integer, |parser, token| {
            let value = match parser.stream.inner.token_value(token) {
                Some(TokenValue::Integer(value)) => value,
                _ => unreachable!(),
            };
            AstNode::Integer(value)
        });
        parser.register_prefix(TokenType::Minus, |parser, _token| {
            let operand = parser.expression(PRECEDENCE_PREFIX);
            AstNode::UnaryOp { op: UnaryOp::Negate, operand: Box::new(operand) }
        });
        let binary_op: InfixParselet = |parser, left, token| {
            let (op, precedence) = match token.typ {
                TokenType::Plus => (BinaryOp::Add, PRECEDENCE_SUM),
                TokenType::Minus => (BinaryOp::Subtract, PRECEDENCE_SUM),
                TokenType::Asterix => (BinaryOp::Multiply, PRECEDENCE_PRODUCT),
                TokenType::Slash => (BinaryOp::Divide, PRECEDENCE_PRODUCT),
                other => panic!("Unsupported binary op token: {:?}", other),
            };
            let right = parser.expression(precedence);
            AstNode::BinaryOp { op, left: Box::new(left), right: Box::new(right) }
        };
        parser.register_infix(TokenType::Plus, PRECEDENCE_SUM, binary_op);
        parser.register_infix(TokenType::Minus, PRECEDENCE_SUM, binary_op);
        parser.register_infix(TokenType::Asterix, PRECEDENCE_PRODUCT, binary_op);
        parser.register_infix(TokenType::Slash, PRECEDENCE_PRODUCT, binary_op);

        parser
    }

    pub fn parse(mut self) -> Result<(), ()> {
        let expr = self.expression(0);
        println!("{}", expr);
        Ok(())
    }

    pub fn expression(&mut self, precedence: u8) -> AstNode {
        let token = self.stream.next().unwrap();

        let prefix = self.prefix_for(token.typ).unwrap();
        let mut left = (prefix)(self, token);

        while self.stream.peek().map_or(false, |next| precedence < self.precedence_for(next.typ).unwrap()) {
            let next = self.stream.next().unwrap();
            let infix = self.infix_for(next.typ).unwrap();
            left = (infix)(self, left, next);
        }

        left
    }
}

/*
 * Parser utilities.
 */
impl<'s> Parser<'s> {
    // TODO: not convinced about this name - other parsers call it match but that's a keyword in
    // Rust
    pub fn test(&mut self, typ: TokenType) -> bool {
        if let Some(token) = self.stream.peek() {
            if token.typ == typ {
                self.stream.next();
                return true;
            }
        }
        false
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
