use std::str::Chars;

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
    Print,
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
    Integer(isize),
    Decimal(f64),
}

pub struct Lex<'s> {
    source: &'s str,
    // TODO: I wonder if we, in a UTF8-aware world, want to iterate over grapheme clusters instead
    // (this may well be a fair bit slower??)
    stream: PeekingIter<Chars<'s>>,
    offset: usize,

    // Tracks info for the token that is currently being lexed
    start_offset: usize,
    current_length: usize,
}

impl<'s> Lex<'s> {
    pub fn new(source: &'s str) -> Lex<'s> {
        Lex { source, stream: PeekingIter::new(source.chars()), offset: 0, start_offset: 0, current_length: 0 }
    }

    /// Get the value of a token, if it has one. This can be called at any time, including after
    /// all tokens have been consumed, and does not change the state of the lexer.
    pub fn token_value(&self, token: Token) -> Option<TokenValue> {
        let value = &self.source[token.offset..(token.offset + token.length)];
        match token.typ {
            TokenType::Identifier => Some(TokenValue::Identifier(value)),
            TokenType::String => Some(TokenValue::String(value)),
            TokenType::Integer => Some(TokenValue::Integer(str::parse(value).unwrap())),
            TokenType::Decimal => Some(TokenValue::Decimal(str::parse(value).unwrap())),
            _ => None,
        }
    }

    pub fn advance(&mut self) -> Option<char> {
        let c = self.stream.next()?;
        self.offset += 1;
        self.current_length += 1;
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

                // TODO: parse comments, both line and block here
                '/' => return Some(self.produce(TokenType::Slash)),

                /*
                 * Skip whitespace.
                 */
                ' ' | '\t' | '\n' => continue,

                /*
                 * Number literals.
                 */
                c if c.is_digit(10) => {
                    // TODO: parse hex
                    // TODO: support octal?

                    while self.stream.peek().map_or(false, |c| c.is_digit(10)) {
                        self.advance();
                    }

                    /*
                     * Handle floating-point numbers. We want to consume the decimal point, but
                     * only if it is followed by another digit.
                     */
                    if self.stream.peek().map_or(false, |c| c == '.')
                        && self.stream.peek_next().map_or(false, |c| c.is_digit(10))
                    {
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
                c if c.is_alphanumeric() => {
                    /*
                     * Do a maximal munch to make sure identifiers that start with reserved
                     * keywords are not mistaken for those keywords.
                     */
                    while self.stream.peek().map_or(false, char::is_alphanumeric) {
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
                        "print" => return Some(self.produce(TokenType::Print)),
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
