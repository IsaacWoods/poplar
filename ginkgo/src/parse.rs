use crate::{
    diagnostic::{Diagnostic, Result},
    lex::{Lex, Token, TokenType, TokenValue},
    object::{GinkgoFunction, GinkgoString},
    vm::{Chunk, Opcode, Value},
};
use std::{collections::BTreeMap, mem};
use thiserror::Error;

pub struct Parser<'s> {
    stream: Lex<'s>,

    prefix_parselets: BTreeMap<TokenType, PrefixParselet>,
    infix_parselets: BTreeMap<TokenType, InfixParselet>,
    precedence: BTreeMap<TokenType, u8>,

    current_function: Function,
    func_stack: Vec<Function>,
}

pub struct Function {
    chunk: Chunk,
    scope_depth: usize,
    locals: Vec<Local>,
}

impl Function {
    pub fn new() -> Function {
        Function { chunk: Chunk::new(), scope_depth: 0, locals: Vec::new() }
    }
}

pub struct Local {
    // TODO: it'd be nice for this to borrow out of the source string for the duration of the parse
    name: String,
    depth: usize,
}

impl<'s> Parser<'s> {
    pub fn new(source: &'s str) -> Parser<'s> {
        // We treat the top-level as a special function
        let script_func = Function { chunk: Chunk::new(), scope_depth: 0, locals: Vec::new() };
        let mut parser = Parser {
            stream: Lex::new(source),
            prefix_parselets: BTreeMap::new(),
            infix_parselets: BTreeMap::new(),
            precedence: BTreeMap::new(),
            current_function: script_func,
            func_stack: Vec::new(),
        };
        parser.register_parselets();

        parser
    }

    pub fn parse(mut self) -> Result<Chunk> {
        while self.stream.peek()?.is_some() {
            self.statement()?;
        }
        // TODO: where should we add the dubious return??
        self.emit(Opcode::Return);

        let script_func = mem::replace(&mut self.current_function, Function::new());
        Ok(script_func.chunk)
    }

    pub fn statement(&mut self) -> Result<()> {
        if self.matches(TokenType::Let)? {
            let name = self.identifier()?;

            if self.current_function.scope_depth > 0 {
                self.current_function
                    .locals
                    .push(Local { name: name.clone(), depth: self.current_function.scope_depth });
            }

            // TODO: allow vars to not be initialized (initialize to unit maybe? Or a `nil` value?)
            self.consume(TokenType::Equals)?;
            self.expression(0)?;
            self.consume(TokenType::Semicolon)?;

            if self.current_function.scope_depth == 0 {
                let name = GinkgoString::new(&name);
                let name_constant = self.current_function.chunk.create_constant(Value::Obj(name.erase()));
                self.emit2(Opcode::Constant, name_constant as u8);
                self.emit(Opcode::DefineGlobal);
            }
        } else if self.matches(TokenType::LeftBrace)? {
            self.begin_scope();
            while !self.matches(TokenType::RightBrace)? {
                self.statement()?;
            }
            self.end_scope();
        } else if self.matches(TokenType::If)? {
            self.expression(0)?;
            let then_jump = self.emit_jump(Opcode::JumpIfFalse);
            self.emit(Opcode::Pop); // Pop the condition on the then branch
            self.consume(TokenType::LeftBrace)?;
            self.begin_scope();
            while !self.matches(TokenType::RightBrace)? {
                self.statement()?;
            }
            self.end_scope();
            let else_jump = self.emit_jump(Opcode::Jump);

            self.current_function.chunk.patch_jump(then_jump);
            self.emit(Opcode::Pop); // Pop the condition on the else branch

            if self.matches(TokenType::Else)? {
                self.consume(TokenType::LeftBrace)?;
                self.begin_scope();
                while !self.matches(TokenType::RightBrace)? {
                    self.statement()?;
                }
                self.end_scope();
            }
            self.current_function.chunk.patch_jump(else_jump);
        } else if self.matches(TokenType::While)? {
            let loop_jump = self.current_function.chunk.current_offset();
            self.expression(0)?;
            let exit_jump = self.emit_jump(Opcode::JumpIfFalse);
            self.emit(Opcode::Pop);

            self.consume(TokenType::LeftBrace)?;
            self.begin_scope();
            while !self.matches(TokenType::RightBrace)? {
                self.statement()?;
            }
            self.end_scope();
            self.emit_jump_to(Opcode::Jump, loop_jump);
            self.current_function.chunk.patch_jump(exit_jump);
        } else if self.matches(TokenType::Fn)? {
            self.function_decl()?;
        } else if self.matches(TokenType::Return)? {
            if self.matches(TokenType::Semicolon)? {
                self.emit(Opcode::Unit);
                self.emit(Opcode::Return);
            } else {
                self.expression(0)?;
                self.consume(TokenType::Semicolon)?;
                self.emit(Opcode::Return);
            }
        } else {
            /*
             * Default case - it's an expression statement.
             * Expressions in statement position may or may not be terminated with a semicolon, so we
             * handle both cases here.
             */
            self.expression(0)?;
            if self.matches(TokenType::Semicolon)? {
                // Pop whatever the expression produces back off the stack
                self.emit(Opcode::Pop);
            }
        }
        // if self.matches(TokenType::Class) {
        //     let name = self.identifier();
        //     self.consume(TokenType::LeftBrace);

        //     let mut defs = Vec::new();

        //     while self.matches(TokenType::Fn) {
        //         defs.push(self.function());
        //     }

        //     self.consume(TokenType::RightBrace);
        // return Stmt { typ: StmtTyp::ClassDef { name, defs } };
        // }

        // if self.matches(TokenType::While) {
        //     let condition = self.expression(0);
        //     self.consume(TokenType::LeftBrace);
        //     let mut body = Vec::new();
        //     while !self.matches(TokenType::RightBrace) {
        //         body.push(self.statement());
        //     }

        //     return Stmt::new_while(condition, Stmt::new_block(body));
        // }

        Ok(())
    }

    fn function_decl(&mut self) -> Result<()> {
        let name = self.identifier()?;

        self.func_stack.push(mem::replace(&mut self.current_function, Function::new()));
        self.begin_scope();

        // The first 'local' is the function being called, so put that in here
        self.current_function.locals.push(Local { name: name.clone(), depth: self.current_function.scope_depth });

        self.consume(TokenType::LeftParen)?;
        let mut arity = 0;
        if !self.matches(TokenType::RightParen)? {
            loop {
                let param_name = self.identifier()?;
                arity += 1;

                self.current_function
                    .locals
                    .push(Local { name: param_name, depth: self.current_function.scope_depth });

                if !self.matches(TokenType::Comma)? {
                    break;
                }
            }
            self.consume(TokenType::RightParen)?;
        }

        self.consume(TokenType::LeftBrace)?;
        while !self.matches(TokenType::RightBrace)? {
            self.statement()?;
        }
        self.end_scope();

        let function = mem::replace(&mut self.current_function, self.func_stack.pop().unwrap());
        let function = GinkgoFunction::new(name.clone(), arity, function.chunk);
        let constant = self.current_function.chunk.create_constant(Value::Obj(function.erase())) as u8;
        self.emit2(Opcode::Constant, constant);

        if self.current_function.scope_depth == 0 {
            let name = GinkgoString::new(&name);
            let name_constant = self.current_function.chunk.create_constant(Value::Obj(name.erase()));
            self.emit2(Opcode::Constant, name_constant as u8);
            self.emit(Opcode::DefineGlobal);
        }

        Ok(())
    }

    // TODO: borrow out of the source string for the return value
    pub fn identifier(&mut self) -> Result<String> {
        let token = self.consume(TokenType::Identifier)?.unwrap();
        if let Some(TokenValue::Identifier(name)) = self.stream.token_value(token) {
            Ok(name.to_string())
        } else {
            // TODO: report error properly - this shouldn't even be reachable so is probs
            // acc an ICE??
            panic!();
        }
    }

    pub fn expression(&mut self, precedence: u8) -> Result<()> {
        let token = self.stream.next().unwrap()?;

        /*
         * Start by parsing a prefix operator. Identifiers and literals both have prefix parselets,
         * so are parsed correctly if there is no 'real' prefix operator.
         */
        let Some(prefix) = self.prefix_for(token.typ) else { Err(UnrecognisedPrefixOperator { op: token.typ })? };
        (prefix)(self, token)?;

        /*
         * Check if the next token, if it exists, represents a valid infix operator that we can
         * parse at the current precedence level. If not, or if it has higher precedence than we're
         * currently allowed to parse, just return the current expression.
         */
        while {
            let next = self.stream.peek()?;
            next.map_or(false, |next| {
                self.precedence_for(next.typ).map_or(false, |next_precedence| precedence < next_precedence)
            })
        } {
            let next = self.stream.next().unwrap()?;
            let Some(infix) = self.infix_for(next.typ) else { Err(UnrecognisedInfixOperator { op: next.typ })? };
            (infix)(self, next)?;
        }

        Ok(())
    }

    fn begin_scope(&mut self) {
        self.current_function.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.current_function.scope_depth -= 1;

        // Pop locals
        // TODO: can we do this from the back and then stop when we find a local to keep?
        let locals_to_pop = self
            .current_function
            .locals
            .iter()
            .filter(|local| local.depth > self.current_function.scope_depth)
            .count();
        // TODO: PopN instruction
        for _ in 0..locals_to_pop {
            self.emit(Opcode::Pop);
        }
        self.current_function.locals.truncate(locals_to_pop);
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
        const PRECEDENCE_PROPERTY_ACCESS: u8 = 13;
        const PRECEDENCE_CALL: u8 = 14;

        /*
         * Literals and identifiers are consumed as prefix operations.
         */
        self.register_prefix(TokenType::Identifier, |parser, token| {
            let value = match parser.stream.token_value(token) {
                Some(TokenValue::Identifier(value)) => value,
                _ => unreachable!(),
            };

            // See if the name resolves to a local
            let local_idx = parser
                .current_function
                .locals
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, local)| if local.name == value { Some(i) } else { None });
            if let Some(local_idx) = local_idx {
                parser.emit2(Opcode::GetLocal, local_idx as u8);
            } else {
                let name = GinkgoString::new(&value.to_string());
                let name_constant = parser.current_function.chunk.create_constant(Value::Obj(name.erase()));
                parser.emit2(Opcode::GetGlobal, name_constant as u8);
            }
            Ok(())
        });
        self.register_prefix(TokenType::Integer, |parser, token| {
            let value = match parser.stream.token_value(token) {
                Some(TokenValue::Integer(value)) => value,
                _ => unreachable!(),
            };
            let constant = parser.current_function.chunk.create_constant(Value::Integer(value as i64));
            parser.emit2(Opcode::Constant, constant as u8);
            Ok(())
        });
        self.register_prefix(TokenType::String, |parser, token| {
            let value = match parser.stream.token_value(token) {
                Some(TokenValue::String(value)) => value.to_string(),
                _ => unreachable!(),
            };
            let value = GinkgoString::new(&value.to_string());
            let constant = parser.current_function.chunk.create_constant(Value::Obj(value.erase()));
            parser.emit2(Opcode::Constant, constant as u8);
            Ok(())
        });
        let bool_literal: PrefixParselet = |parser, token| {
            match token.typ {
                TokenType::True => parser.emit(Opcode::True),
                TokenType::False => parser.emit(Opcode::False),
                _ => unreachable!(),
            }
            Ok(())
        };
        self.register_prefix(TokenType::True, bool_literal);
        self.register_prefix(TokenType::False, bool_literal);
        // self.register_prefix(TokenType::GinkgoSelf, |_parser, _token| {
        //     Expr::new(ExprTyp::GinkgoSelf { resolution: Resolution::Unresolved })
        // });

        /*
         * 'Real' prefix operations.
         */
        self.register_prefix(TokenType::Minus, |parser, _token| {
            parser.expression(PRECEDENCE_PREFIX)?;
            parser.emit(Opcode::Negate);
            Ok(())
        });
        self.register_prefix(TokenType::LeftParen, |parser, _token| {
            parser.expression(0)?;
            parser.consume(TokenType::RightParen)?;
            Ok(())
        });

        /*
         * Function definitions are also prefix operations.
         */
        // self.register_prefix(TokenType::Fn, |parser, _token| {
        //     parser.consume(TokenType::LeftParen);

        //     /*
        //      * Only the first parameter can be `self`, and it can only occur inside classes.
        //      */
        //     let takes_self = if parser.matches(TokenType::GinkgoSelf) {
        //         parser.matches(TokenType::Comma);
        //         true
        //     } else {
        //         false
        //     };

        //     let mut params = Vec::new();
        //     while !parser.matches(TokenType::RightParen) {
        //         // TODO: not sure if we want to parse expressions or something simpler (e.g. could
        //         // just be idents for now, but might want more complex (e.g. patterns) in the
        //         // future.
        //         let param = parser.expression(0);
        //         if let ExprTyp::Identifier { name, .. } = param.typ {
        //             params.push(name);
        //         } else {
        //             panic!("Invalid param name");
        //         }
        //         parser.matches(TokenType::Comma);
        //     }

        //     parser.consume(TokenType::LeftBrace);

        //     let mut statements = Vec::new();
        //     while !parser.matches(TokenType::RightBrace) {
        //         statements.push(parser.statement());
        //     }

        //     Expr::new(ExprTyp::Function { takes_self, body: statements, params })
        // });

        /*
         * Infix operations.
         */
        let binary_op: InfixParselet = |parser, token| {
            let (op, precedence) = match token.typ {
                TokenType::Plus => (Opcode::Add, PRECEDENCE_SUM),
                TokenType::Minus => (Opcode::Subtract, PRECEDENCE_SUM),
                TokenType::Asterix => (Opcode::Multiply, PRECEDENCE_PRODUCT),
                TokenType::Slash => (Opcode::Divide, PRECEDENCE_PRODUCT),
                TokenType::Ampersand => (Opcode::BitwiseAnd, PRECEDENCE_BITWISE_AND),
                TokenType::Pipe => (Opcode::BitwiseOr, PRECEDENCE_BITWISE_OR),
                TokenType::Caret => (Opcode::BitwiseXor, PRECEDENCE_BITWISE_XOR),
                TokenType::EqualEquals => (Opcode::Equal, PRECEDENCE_CONDITIONAL),
                TokenType::BangEquals => (Opcode::NotEqual, PRECEDENCE_CONDITIONAL),
                TokenType::GreaterThan => (Opcode::GreaterThan, PRECEDENCE_CONDITIONAL),
                TokenType::GreaterEqual => (Opcode::GreaterEqual, PRECEDENCE_CONDITIONAL),
                TokenType::LessThan => (Opcode::LessThan, PRECEDENCE_CONDITIONAL),
                TokenType::LessEqual => (Opcode::LessEqual, PRECEDENCE_CONDITIONAL),
                _ => unreachable!(),
            };
            parser.expression(precedence)?;
            parser.emit(op);
            Ok(())
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

        self.register_infix(TokenType::AmpersandAmpersand, PRECEDENCE_LOGICAL_AND, |parser, _token| {
            /*
             * We utilise the fact that jump operations leave their condition on the stack to implement
             * short-circuiting here. If the left side is false, jump over the right side, leaving the
             * `false` as the overall result.
             */
            let jump = parser.emit_jump(Opcode::JumpIfFalse);
            parser.emit(Opcode::Pop);
            parser.expression(PRECEDENCE_LOGICAL_AND)?;
            parser.current_function.chunk.patch_jump(jump);
            Ok(())
        });
        self.register_infix(TokenType::PipePipe, PRECEDENCE_LOGICAL_OR, |parser, _token| {
            /*
             * We do something similar for logical OR.
             */
            let jump = parser.emit_jump(Opcode::JumpIfTrue);
            parser.emit(Opcode::Pop);
            parser.expression(PRECEDENCE_LOGICAL_OR)?;
            parser.current_function.chunk.patch_jump(jump);
            Ok(())
        });

        /*
         * Assignment.
         */
        self.register_infix(TokenType::Equals, PRECEDENCE_ASSIGNMENT, |parser, _token| {
            /*
             * This is a slightly strange approach, but works for our needs. We have parsed the left-hand
             * side of the assignment, which should produce a place. We need to remove the emitted bytecode
             * and emit our own to do the assignment. We also use this to confirm the LHS will produce
             * a valid place to assign to.
             */
            // TODO: this won't work with GetLocal followed by the index :( - can we fix through somehow?? Maybe push the slot to the actual stack idk??
            let hopefully_an_operand = parser.current_function.chunk.pop_last().unwrap();
            let op_to_replace_with = match parser.current_function.chunk.pop_last_op() {
                Some(Opcode::GetGlobal) => Opcode::SetGlobal,
                Some(Opcode::GetLocal) => Opcode::SetLocal,
                // TODO: runtime error
                _ => panic!(),
            };

            parser.expression(PRECEDENCE_ASSIGNMENT - 1)?;
            parser.emit2(op_to_replace_with, hopefully_an_operand);
            Ok(())
        });

        /*
         * Function calls.
         */
        self.register_infix(TokenType::LeftParen, PRECEDENCE_CALL, |parser, _token| {
            let mut arg_count = 0;
            while !parser.matches(TokenType::RightParen)? {
                parser.expression(0)?;
                parser.matches(TokenType::Comma)?;
                arg_count += 1;
            }

            parser.emit2(Opcode::Call, arg_count);
            Ok(())
        });

        // self.register_infix(TokenType::Dot, PRECEDENCE_PROPERTY_ACCESS, |parser, left, _token| {
        //     let property = parser.identifier();
        //     Expr::new(ExprTyp::PropertyAccess { left: Box::new(left), property })
        // });
    }
}

/*
 * Parser utilities.
 */
type PrefixParselet = fn(&mut Parser, Token) -> Result<()>;
type InfixParselet = fn(&mut Parser, Token) -> Result<()>;

impl<'s> Parser<'s> {
    pub fn matches(&mut self, typ: TokenType) -> Result<bool> {
        match self.stream.peek() {
            Ok(Some(token)) if token.typ == typ => {
                self.stream.next();
                Ok(true)
            }
            Err(err) => Err(err),
            _ => Ok(false),
        }
    }

    /// Expect a token of the given type, issuing a parse error if the next token is not of the
    /// expected type.
    pub fn consume(&mut self, typ: TokenType) -> Result<Option<Token>> {
        match self.stream.next() {
            Some(Ok(token)) if token.typ == typ => Ok(Some(token)),
            Some(Err(err)) => Err(err),
            Some(Ok(other)) => {
                // TODO: should we consume the token or not for best chances of recovery?
                Err(ExpectedTokenButGot { expected: typ, got: other.typ })?
            }
            None => Err(UnexpectedEndOfTokenStream { expected: typ })?,
        }
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

    fn emit(&mut self, op: Opcode) {
        self.emit_raw(op as u8);
    }

    fn emit_raw(&mut self, byte: u8) {
        self.current_function.chunk.push(byte);
    }

    fn emit2(&mut self, op: Opcode, operand: u8) {
        self.current_function.chunk.push(op as u8);
        self.current_function.chunk.push(operand);
    }

    /// Emit a `jump`, returning an offset to patch later
    fn emit_jump(&mut self, jump: Opcode) -> usize {
        self.emit(jump);
        // Record the offset of the jump's operand to patch later
        let offset = self.current_function.chunk.current_offset();
        self.emit_raw(0);
        self.emit_raw(0);
        offset
    }

    /// Emit a `jump` to an already-known `target`
    fn emit_jump_to(&mut self, jump: Opcode, target: usize) {
        self.emit(jump);
        let current_offset = self.current_function.chunk.current_offset();
        // XXX: add an extra 2 to account for the `i16` operand
        let bytes = i16::try_from(target.checked_signed_diff(current_offset + 2).unwrap()).unwrap().to_le_bytes();
        self.emit_raw(bytes[0]);
        self.emit_raw(bytes[1]);
    }
}

#[derive(Clone, Debug, Error)]
#[error("unexpected end of token stream, expected a {expected:?}")]
pub struct UnexpectedEndOfTokenStream {
    pub expected: TokenType,
}
impl Diagnostic for UnexpectedEndOfTokenStream {}

#[derive(Clone, Debug, Error)]
#[error("expected {expected:?} but got {got:?}")]
pub struct ExpectedTokenButGot {
    pub expected: TokenType,
    pub got: TokenType,
}
impl Diagnostic for ExpectedTokenButGot {}

#[derive(Clone, Debug, Error)]
#[error("unrecognised prefix operator {op:?}")]
pub struct UnrecognisedPrefixOperator {
    pub op: TokenType,
}
impl Diagnostic for UnrecognisedPrefixOperator {}

#[derive(Clone, Debug, Error)]
#[error("unrecognised infix operator {op:?}")]
pub struct UnrecognisedInfixOperator {
    pub op: TokenType,
}
impl Diagnostic for UnrecognisedInfixOperator {}
