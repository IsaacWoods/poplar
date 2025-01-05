use crate::{
    lex::{Lex, PeekingIter, Token, TokenType, TokenValue},
    object::{GinkgoObj, GinkgoString, ObjHeader},
    vm::{Chunk, Opcode, Value},
};
use std::{collections::BTreeMap, mem::replace, thread::current};

pub struct Parser<'s> {
    stream: PeekingIter<Lex<'s>>,

    prefix_parselets: BTreeMap<TokenType, PrefixParselet>,
    infix_parselets: BTreeMap<TokenType, InfixParselet>,
    precedence: BTreeMap<TokenType, u8>,

    chunk: Option<Chunk>,

    scope_depth: usize,
    locals: Vec<Local>,
}

pub struct Local {
    // TODO: it'd be nice for this to borrow out of the source string for the duration of the parse
    name: String,
    depth: usize,
}

impl<'s> Parser<'s> {
    pub fn new(source: &'s str) -> Parser<'s> {
        let lex = PeekingIter::new(Lex::new(source));
        let mut parser = Parser {
            stream: lex,
            prefix_parselets: BTreeMap::new(),
            infix_parselets: BTreeMap::new(),
            precedence: BTreeMap::new(),
            chunk: Some(Chunk::new()),
            scope_depth: 0,
            locals: Vec::new(),
        };
        parser.register_parselets();

        parser
    }

    pub fn parse(mut self) -> Result<Chunk, ()> {
        while self.stream.peek().is_some() {
            self.statement();
        }
        // TODO: where should we add the dubious return??
        self.emit(Opcode::Return);
        Ok(self.chunk.take().unwrap())
    }

    pub fn statement(&mut self) {
        if self.matches(TokenType::Let) {
            let name = self.identifier();

            if self.scope_depth > 0 {
                self.locals.push(Local { name, depth: self.scope_depth });
            } else {
                let name = GinkgoString::new(&name);
                let name_constant =
                    self.chunk.as_mut().unwrap().create_constant(Value::Obj(name as *const ObjHeader));
                self.emit2(Opcode::Constant, name_constant as u8);
            }

            // TODO: allow vars to not be initialized (initialize to unit maybe? Or a `nil` value?)
            self.consume(TokenType::Equals);
            self.expression(0);
            self.consume(TokenType::Semicolon);

            if self.scope_depth == 0 {
                self.emit(Opcode::DefineGlobal);
            }
        } else if self.matches(TokenType::LeftBrace) {
            self.begin_scope();
            while !self.matches(TokenType::RightBrace) {
                self.statement();
            }
            self.end_scope();
        } else if self.matches(TokenType::If) {
            self.expression(0);
            let then_jump = self.emit_jump(Opcode::JumpIfFalse);
            self.emit(Opcode::Pop); // Pop the condition on the then branch
            self.consume(TokenType::LeftBrace);
            self.begin_scope();
            while !self.matches(TokenType::RightBrace) {
                self.statement();
            }
            self.end_scope();
            let else_jump = self.emit_jump(Opcode::Jump);

            self.chunk.as_mut().unwrap().patch_jump(then_jump);
            self.emit(Opcode::Pop); // Pop the condition on the else branch

            if self.matches(TokenType::Else) {
                self.consume(TokenType::LeftBrace);
                self.begin_scope();
                while !self.matches(TokenType::RightBrace) {
                    self.statement();
                }
                self.end_scope();
            }
            self.chunk.as_mut().unwrap().patch_jump(else_jump);
        } else if self.matches(TokenType::While) {
            let loop_jump = self.chunk.as_ref().unwrap().current_offset();
            self.expression(0);
            let exit_jump = self.emit_jump(Opcode::JumpIfFalse);
            self.emit(Opcode::Pop);

            self.consume(TokenType::LeftBrace);
            self.begin_scope();
            while !self.matches(TokenType::RightBrace) {
                self.statement();
            }
            self.end_scope();
            self.emit_jump_to(Opcode::Jump, loop_jump);
            self.chunk.as_mut().unwrap().patch_jump(exit_jump);
        } else {
            /*
             * Default case - it's an expression statement.
             * Expressions in statement position may or may not be terminated with a semicolon, so we
             * handle both cases here.
             */
            self.expression(0);
            if self.matches(TokenType::Semicolon) {
                // Pop whatever the expression produces back off the stack
                self.emit(Opcode::Pop);
            }
        }

        // if self.matches(TokenType::Fn) {
        //     return self.function();
        // }

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

        // TODO: I think we want `if`s to be expressions too actually? (with optional `else` if the
        // then branch returns unit).
        // if self.matches(TokenType::If) {
        //     // let condition = self.expression(0);
        //     // self.consume(TokenType::LeftBrace);
        //     // let mut then_block = Vec::new();
        //     // while !self.matches(TokenType::RightBrace) {
        //     //     then_block.push(self.statement());
        //     // }
        //     // let then_block = Stmt::new_block(then_block);

        //     // let else_block = if self.matches(TokenType::Else) {
        //     //     self.consume(TokenType::LeftBrace);
        //     //     let mut statements = Vec::new();
        //     //     while !self.matches(TokenType::RightBrace) {
        //     //         statements.push(self.statement());
        //     //     }
        //     // Some(Stmt::new_block(statements))
        //     // } else {
        //     //     None
        //     // };

        //     // return Stmt::new_if(condition, then_block, else_block);
        //     todo!()
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

        // if self.matches(TokenType::Return) {
        //     let value = self.expression(0);
        //     self.consume(TokenType::Semicolon);
        //     return Stmt::new_return(value);
        // }
    }

    // pub fn function(&mut self) -> Stmt {
    //     let name = self.identifier();
    //     self.consume(TokenType::LeftParen);

    //     /*
    //      * Only the first parameter can be `self`, and it can only occur inside classes.
    //      * TODO: check here or in the resolver whether we're in the right place for a self?
    //      */
    //     let takes_self = if self.matches(TokenType::GinkgoSelf) {
    //         self.matches(TokenType::Comma);
    //         true
    //     } else {
    //         false
    //     };

    //     let mut params = Vec::new();
    //     while !self.matches(TokenType::RightParen) {
    //         // TODO: not sure if we want to parse expressions or something simpler (e.g. could
    //         // just be idents for now, but might want more complex (e.g. patterns) in the
    //         // future.
    //         let param = self.expression(0);
    //         if let ExprTyp::Identifier { name, .. } = param.typ {
    //             params.push(name);
    //         } else {
    //             panic!("Invalid param name");
    //         }
    //         self.matches(TokenType::Comma);
    //     }

    //     self.consume(TokenType::LeftBrace);

    //     let mut statements = Vec::new();
    //     while !self.matches(TokenType::RightBrace) {
    //         statements.push(self.statement());
    //     }
    //     Stmt::new_fn_def(name, takes_self, params, statements)
    // }

    // TODO: borrow out of the source string for the return value
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

    pub fn expression(&mut self, precedence: u8) {
        let token = self.stream.next().unwrap();

        /*
         * Start by parsing a prefix operator. Identifiers and literals both have prefix parselets,
         * so are parsed correctly if there is no 'real' prefix operator.
         */
        let Some(prefix) = self.prefix_for(token.typ) else {
            panic!("No prefix parselet for token: {:?}", token.typ);
        };
        (prefix)(self, token);

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
            (infix)(self, next);
        }
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.scope_depth -= 1;

        // Pop locals
        // TODO: can we do this from the back and then stop when we find a local to keep?
        let locals_to_pop = self.locals.iter().filter(|local| local.depth > self.scope_depth).count();
        // TODO: PopN instruction
        for _ in 0..locals_to_pop {
            self.emit(Opcode::Pop);
        }
        self.locals.truncate(locals_to_pop);
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
            let value = match parser.stream.inner.token_value(token) {
                Some(TokenValue::Identifier(value)) => value,
                _ => unreachable!(),
            };

            // See if the name resolves to a local
            let local_idx =
                parser
                    .locals
                    .iter()
                    .enumerate()
                    .rev()
                    .find_map(|(i, local)| if local.name == value { Some(i) } else { None });
            if let Some(local_idx) = local_idx {
                parser.emit(Opcode::GetLocal);
                parser.emit_raw(local_idx as u8);
            } else {
                let name = GinkgoString::new(&value.to_string());
                let name_constant =
                    parser.chunk.as_mut().unwrap().create_constant(Value::Obj(name as *const ObjHeader));
                parser.emit2(Opcode::Constant, name_constant as u8);
                parser.emit(Opcode::GetGlobal);
            }
        });
        self.register_prefix(TokenType::Integer, |parser, token| {
            let value = match parser.stream.inner.token_value(token) {
                Some(TokenValue::Integer(value)) => value,
                _ => unreachable!(),
            };
            let constant = parser.chunk.as_mut().unwrap().create_constant(Value::Integer(value as i64));
            parser.emit2(Opcode::Constant, constant as u8);
        });
        self.register_prefix(TokenType::String, |parser, token| {
            let value = match parser.stream.inner.token_value(token) {
                Some(TokenValue::String(value)) => value.to_string(),
                _ => unreachable!(),
            };
            let value = GinkgoString::new(&value.to_string());
            let constant = parser.chunk.as_mut().unwrap().create_constant(Value::Obj(value as *const ObjHeader));
            parser.emit2(Opcode::Constant, constant as u8);
        });
        let bool_literal: PrefixParselet = |parser, token| match token.typ {
            TokenType::True => parser.emit(Opcode::True),
            TokenType::False => parser.emit(Opcode::False),
            _ => unreachable!(),
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
            parser.expression(PRECEDENCE_PREFIX);
            parser.emit(Opcode::Negate);
        });
        self.register_prefix(TokenType::LeftParen, |parser, _token| {
            parser.expression(0);
            parser.consume(TokenType::RightParen);
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
                other => panic!("Unsupported binary op token: {:?}", other),
            };
            parser.expression(precedence);
            parser.emit(op);
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
            parser.expression(PRECEDENCE_LOGICAL_AND);
            parser.chunk.as_mut().unwrap().patch_jump(jump);
        });
        self.register_infix(TokenType::PipePipe, PRECEDENCE_LOGICAL_OR, |parser, _token| {
            /*
             * We do something similar for logical OR.
             */
            let jump = parser.emit_jump(Opcode::JumpIfTrue);
            parser.emit(Opcode::Pop);
            parser.expression(PRECEDENCE_LOGICAL_OR);
            parser.chunk.as_mut().unwrap().patch_jump(jump);
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
            let op_to_replace_with = match parser.chunk.as_mut().unwrap().pop_last_op() {
                Some(Opcode::GetGlobal) => Opcode::SetGlobal,
                Some(Opcode::GetLocal) => Opcode::SetLocal,
                // TODO: runtime error
                _ => panic!(),
            };

            parser.expression(PRECEDENCE_ASSIGNMENT - 1);
            parser.emit(op_to_replace_with);
        });

        /*
         * Function calls.
         */
        // self.register_infix(TokenType::LeftParen, PRECEDENCE_CALL, |parser, left, _token| {
        //     let mut params = Vec::new();
        //     while !parser.matches(TokenType::RightParen) {
        //         let param = parser.expression(0);
        //         parser.matches(TokenType::Comma);
        //         params.push(param);
        //     }

        //     Expr::new(ExprTyp::Call { left: Box::new(left), params })
        // });

        // self.register_infix(TokenType::Dot, PRECEDENCE_PROPERTY_ACCESS, |parser, left, _token| {
        //     let property = parser.identifier();
        //     Expr::new(ExprTyp::PropertyAccess { left: Box::new(left), property })
        // });
    }
}

/*
 * Parser utilities.
 */
type PrefixParselet = fn(&mut Parser, Token);
type InfixParselet = fn(&mut Parser, Token);

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

    fn emit(&mut self, op: Opcode) {
        self.emit_raw(op as u8);
    }

    fn emit_raw(&mut self, byte: u8) {
        self.chunk.as_mut().unwrap().push(byte);
    }

    fn emit2(&mut self, op: Opcode, operand: u8) {
        let chunk = self.chunk.as_mut().unwrap();
        chunk.push(op as u8);
        chunk.push(operand);
    }

    /// Emit a `jump`, returning an offset to patch later
    fn emit_jump(&mut self, jump: Opcode) -> usize {
        self.emit(jump);
        // Record the offset of the jump's operand to patch later
        let offset = self.chunk.as_ref().unwrap().current_offset();
        self.emit_raw(0);
        self.emit_raw(0);
        offset
    }

    /// Emit a `jump` to an already-known `target`
    fn emit_jump_to(&mut self, jump: Opcode, target: usize) {
        self.emit(jump);
        let current_offset = self.chunk.as_ref().unwrap().current_offset();
        // XXX: add an extra 2 to account for the `i16` operand
        let bytes = i16::try_from(target.checked_signed_diff(current_offset + 2).unwrap()).unwrap().to_le_bytes();
        self.emit_raw(bytes[0]);
        self.emit_raw(bytes[1]);
    }
}
