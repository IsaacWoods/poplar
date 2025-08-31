use std::str::Chars;
use unicode_xid::UnicodeXID;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum TokenType {
    /*
     * Symbols.
     */
    Plus,
    Minus,
    Dot,
    Equals,
    EqualEquals,
    Asterix,
    Slash,
    Colon,
    Semicolon,
    Comma,
    Bang,
    BangEquals,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
    QuestionMark,
    Caret,
    Ampersand,
    AmpersandAmpersand,
    Pipe,
    PipePipe,

    /*
     * Literals.
     */
    Identifier,
    String,
    Integer,
    Decimal,

    /*
     * Keywords.
     */
    Let,
    If,
    Else,
    For,
    Loop,
    While,
    True,
    False,
    Return,
    Fn,
    Class,
    // XXX: stupid name because `Self` is a keyword in Rust too
    GinkgoSelf,
}

#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub typ: TokenType,
    pub offset: usize,
    pub length: usize,
}

#[derive(Clone, PartialEq, Debug)]
pub enum TokenValue<'s> {
    Identifier(&'s str),
    String(&'s str),
    Integer(usize),
    Decimal(f64),
}

pub struct Lex<'s> {
    source: &'s str,
    stream: PeekingIter<Chars<'s>>,
    offset: usize,

    // Tracks info for the token that is currently being lexed
    start_offset: usize,
    current_length: usize,
}

fn token_to_integer_literal(value: &str) -> usize {
    let mut c = value.chars();

    if c.next().unwrap() == '0' {
        match c.next().unwrap() {
            'b' => usize::from_str_radix(&value[2..], 2).unwrap(),
            'x' => usize::from_str_radix(&value[2..], 16).unwrap(),
            'o' => usize::from_str_radix(&value[2..], 8).unwrap(),
            // Also decimal
            _ => str::parse(value).unwrap(),
        }
    } else {
        // Decimal
        str::parse(value).unwrap()
    }
}

impl<'s> Lex<'s> {
    pub fn new(source: &'s str) -> Lex<'s> {
        Lex { source, stream: PeekingIter::new(source.chars()), offset: 0, start_offset: 0, current_length: 0 }
    }

    /// Get the value of a token, if it has one. This can be called at any time, including after
    /// all tokens have been consumed, and does not change the state of the lexer.
    pub fn token_value(&self, token: Token) -> Option<TokenValue<'_>> {
        let value = &self.source[token.offset..(token.offset + token.length)];
        match token.typ {
            TokenType::Identifier => Some(TokenValue::Identifier(value)),
            TokenType::String => Some(TokenValue::String(value)),
            TokenType::Integer => Some(TokenValue::Integer(token_to_integer_literal(value))),
            TokenType::Decimal => Some(TokenValue::Decimal(str::parse(value).unwrap())),
            _ => None,
        }
    }

    pub fn advance(&mut self) -> Option<char> {
        let c = self.stream.next()?;
        self.offset += c.len_utf8();
        self.current_length += c.len_utf8();
        Some(c)
    }

    pub fn produce(&mut self, typ: TokenType) -> Token {
        Token { typ, offset: self.start_offset, length: self.current_length }
    }
}

impl<'s> Iterator for Lex<'s> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            /*
             * Each loop iteration starts a new token. This ensures skipped whitespace etc. is not
             * counted in tokens.
             */
            self.start_offset = self.offset;
            self.current_length = 0;
            let c = self.advance()?;

            match c {
                '+' => return Some(self.produce(TokenType::Plus)),
                '-' => return Some(self.produce(TokenType::Minus)),
                '.' => return Some(self.produce(TokenType::Dot)),
                '=' => match self.stream.peek() {
                    Some('=') => {
                        self.advance();
                        return Some(self.produce(TokenType::EqualEquals));
                    }
                    _ => return Some(self.produce(TokenType::Equals)),
                },
                '*' => return Some(self.produce(TokenType::Asterix)),
                ':' => return Some(self.produce(TokenType::Colon)),
                ';' => return Some(self.produce(TokenType::Semicolon)),
                ',' => return Some(self.produce(TokenType::Comma)),
                '!' => match self.stream.peek() {
                    Some('=') => {
                        self.advance();
                        return Some(self.produce(TokenType::BangEquals));
                    }
                    _ => return Some(self.produce(TokenType::Bang)),
                },
                '(' => return Some(self.produce(TokenType::LeftParen)),
                ')' => return Some(self.produce(TokenType::RightParen)),
                '{' => return Some(self.produce(TokenType::LeftBrace)),
                '}' => return Some(self.produce(TokenType::RightBrace)),
                '<' => match self.stream.peek() {
                    Some('=') => {
                        self.advance();
                        return Some(self.produce(TokenType::LessEqual));
                    }
                    _ => return Some(self.produce(TokenType::LessThan)),
                },
                '>' => match self.stream.peek() {
                    Some('=') => {
                        self.advance();
                        return Some(self.produce(TokenType::GreaterEqual));
                    }
                    _ => return Some(self.produce(TokenType::GreaterThan)),
                },
                '?' => return Some(self.produce(TokenType::QuestionMark)),
                '^' => return Some(self.produce(TokenType::Caret)),
                '&' => match self.stream.peek() {
                    Some('&') => {
                        self.advance();
                        return Some(self.produce(TokenType::AmpersandAmpersand));
                    }
                    _ => return Some(self.produce(TokenType::Ampersand)),
                },
                '|' => match self.stream.peek() {
                    Some('|') => {
                        self.advance();
                        return Some(self.produce(TokenType::PipePipe));
                    }
                    _ => return Some(self.produce(TokenType::Pipe)),
                },

                // TODO: parse comments, both line and block here, including handling of nested comments
                '/' => return Some(self.produce(TokenType::Slash)),

                /*
                 * Skip whitespace.
                 */
                ' ' | '\t' | '\n' | '\r' => continue,

                /*
                 * Number literals.
                 */
                c if c.is_digit(10) => {
                    // TODO: scientfic notation
                    // TODO: separation with underscore between digits

                    let base = if c == '0'
                        && let Some('x') = self.stream.peek()
                    {
                        self.advance();
                        16
                    } else if let Some('b') = self.stream.peek() {
                        self.advance();
                        2
                    } else if let Some('o') = self.stream.peek() {
                        self.advance();
                        8
                    } else {
                        10
                    };

                    while self.stream.peek().map_or(false, |c| c.is_digit(base)) {
                        self.advance();
                    }

                    /*
                     * Handle floating-point numbers. We want to consume the decimal point, but
                     * only if it is followed by another digit.
                     */
                    if self.stream.peek().map_or(false, |c| c == '.')
                        && self.stream.peek_next().map_or(false, |c| c.is_digit(10))
                    {
                        if base != 10 {
                            // TODO: produce a nice diagnostic here
                            panic!("No floating point in your hex in this language!");
                        }

                        self.advance();
                        while self.stream.peek().map_or(false, |c| c.is_digit(10)) {
                            self.advance();
                        }
                        return Some(self.produce(TokenType::Decimal));
                    } else {
                        return Some(self.produce(TokenType::Integer));
                    }
                }

                '"' => {
                    while self.stream.peek().map_or(false, |c| c != '"') {
                        self.advance()?;
                    }

                    self.advance()?;

                    /*
                     * Manually trim the opening and closing `"`s off.
                     */
                    self.start_offset += 1;
                    self.current_length -= 2;
                    return Some(self.produce(TokenType::String));
                }

                /*
                 * Parse keywords and identifiers.
                 */
                // TODO: identifiers should be able to start with underscores, but an underscore on its own should not be lexed as an identifier
                c if c.is_xid_start() || c == '_' => {
                    /*
                     * Do a maximal munch to make sure identifiers that start with reserved
                     * keywords are not mistaken for those keywords.
                     */
                    while self.stream.peek().map_or(false, |c| c.is_xid_continue()) {
                        self.advance()?;
                    }

                    match &self.source[self.start_offset..(self.start_offset + self.current_length)] {
                        "let" => return Some(self.produce(TokenType::Let)),
                        "if" => return Some(self.produce(TokenType::If)),
                        "else" => return Some(self.produce(TokenType::Else)),
                        "for" => return Some(self.produce(TokenType::For)),
                        "loop" => return Some(self.produce(TokenType::Loop)),
                        "while" => return Some(self.produce(TokenType::While)),
                        "true" => return Some(self.produce(TokenType::True)),
                        "false" => return Some(self.produce(TokenType::False)),
                        "return" => return Some(self.produce(TokenType::Return)),
                        "fn" => return Some(self.produce(TokenType::Fn)),
                        "class" => return Some(self.produce(TokenType::Class)),
                        "self" => return Some(self.produce(TokenType::GinkgoSelf)),
                        _ => return Some(self.produce(TokenType::Identifier)),
                    }
                }

                // TODO: issue a lex error rather than panicking
                other => panic!("Unknown char in lex: {:?}", other),
            }
        }
    }
}

/// Wraps an `Iterator` and provides exactly two items of lookahead. This is enough to implement a
/// wide variety of lexers and parsers.
pub struct PeekingIter<I>
where
    I: Iterator,
    I::Item: Clone,
{
    pub inner: I,
    peek: Option<I::Item>,
    peek_next: Option<I::Item>,
}

impl<I> PeekingIter<I>
where
    I: Iterator,
    I::Item: Clone,
{
    pub fn new(inner: I) -> PeekingIter<I> {
        PeekingIter { inner, peek: None, peek_next: None }
    }

    pub fn peek(&mut self) -> Option<I::Item> {
        if let Some(peek) = &self.peek {
            Some(peek.clone())
        } else {
            let next = self.inner.next()?;
            self.peek = Some(next.clone());
            Some(next)
        }
    }

    pub fn peek_next(&mut self) -> Option<I::Item> {
        assert!(self.peek.is_some());

        if let Some(peek) = &self.peek_next {
            Some(peek.clone())
        } else {
            let next = self.inner.next()?;
            self.peek_next = Some(next.clone());
            Some(next)
        }
    }
}

impl<I> Iterator for PeekingIter<I>
where
    I: Iterator,
    I::Item: Clone,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've already peeked forward, use that
        if let Some(peeked) = self.peek.take() {
            // Bump the 2nd lookahead item up a space
            self.peek = self.peek_next.take();

            Some(peeked)
        } else {
            self.inner.next()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tokens(source: &str, tokens: &[TokenType]) {
        let mut lex = Lex::new(source);

        for token_to_match in tokens.into_iter() {
            match lex.next() {
                Some(token) if token.typ == *token_to_match => (),
                Some(other) => panic!("Got wrong type of token: {:?} => {:?}", other, lex.token_value(other)),
                None => panic!(),
            }
        }
    }

    #[test]
    fn keywords() {
        test_tokens(
            "let if else for loop while true false return fn class self",
            &[
                TokenType::Let,
                TokenType::If,
                TokenType::Else,
                TokenType::For,
                TokenType::Loop,
                TokenType::While,
                TokenType::True,
                TokenType::False,
                TokenType::Return,
                TokenType::Fn,
                TokenType::Class,
                TokenType::GinkgoSelf,
            ],
        );
    }

    #[test]
    fn identifiers() {
        fn test_identifier(ident: &str) {
            let mut lex = Lex::new(ident);
            let token = lex.next().expect("Failed to lex identifier correctly!");
            assert!(lex.next().is_none());

            match lex.token_value(token) {
                Some(TokenValue::Identifier(lexed_ident)) => assert_eq!(ident, lexed_ident),
                _ => panic!("Failed to lex identifier correctly!"),
            }
        }

        test_identifier("foo");
        test_identifier("bar73");
        test_identifier("with_some_underscores");
        test_identifier("do_n0t_nam3_th1ngs_l1k3_thi5");
        test_identifier("Москва");
        test_identifier("東京");
    }

    #[test]
    fn numbers() {
        fn test_number(source: &str, typ: TokenType, literal: usize) {
            let mut lex = Lex::new(source);
            let number = lex.next().unwrap();
            assert!(lex.next().is_none());

            assert_eq!(number.typ, typ);
            assert_eq!(lex.token_value(number).unwrap(), TokenValue::Integer(literal));
        }

        test_number("14", TokenType::Integer, 14);
        test_number("0b00110101010101010", TokenType::Integer, 0b00110101010101010);
        test_number("0xbeef", TokenType::Integer, 0xbeef);
        test_number("0o777777777", TokenType::Integer, 0o777_777_777);
    }

    // TODO: test numbers - hex literals, octal literals, binary literals, scientific notation, underscore separation
    // TODO: test strings
    // TODO: test operators
    // TODO: test comments
}
